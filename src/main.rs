mod models;
mod supabase;
mod state;

use std::sync::Arc;
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use state::{AppState, SharedState};
use supabase::SupabaseClient;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use dotenvy::dotenv;
use std::env;
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;
use crate::models::{WsMessage, WsEvent};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // user_id
    exp: usize,
}

#[derive(Deserialize)]
struct AuthRequest {
    email: String,
    password: String,
}

async fn register(
    State(state): State<SharedState>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    match state.supabase.signup(&payload.email, &payload.password).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn login(
    State(state): State<SharedState>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    match state.supabase.signin(&payload.email, &payload.password).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => (StatusCode::UNAUTHORIZED, e.to_string()).into_response(),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let token = params.get("token").cloned();

    if let Some(token) = token {
        let jwt_secret = env::var("SUPABASE_JWT_SECRET").expect("SUPABASE_JWT_SECRET must be set");
        let validation = Validation::new(Algorithm::HS256);
        let key = DecodingKey::from_secret(jwt_secret.as_bytes());

        match decode::<Claims>(&token, &key, &validation) {
            Ok(token_data) => {
                let user_id = Uuid::parse_str(&token_data.claims.sub).unwrap_or_else(|_| Uuid::new_v4());
                ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
            }
            Err(_) => (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
        }
    } else {
        (StatusCode::UNAUTHORIZED, "Missing token").into_response()
    }
}

async fn handle_socket(socket: WebSocket, state: SharedState, user_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    state.user_connections.insert(user_id, tx);

    tracing::info!("User {} connected", user_id);

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    process_message(user_id, ws_msg, &state_clone).await;
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    state.user_connections.remove(&user_id);
    // Remove user from all rooms
    for mut entry in state.room_members.iter_mut() {
        entry.value_mut().remove(&user_id);
    }
    tracing::info!("User {} disconnected", user_id);
}

async fn process_message(user_id: Uuid, msg: WsMessage, state: &SharedState) {
    match msg {
        WsMessage::CreateRoom { name } => {
            match state.supabase.create_room(Some(name.clone()), false).await {
                Ok(room) => {
                    let _ = state.supabase.join_room(room.id, user_id).await;
                    state.room_members.entry(room.id).or_default().insert(user_id);

                    let event = WsEvent::RoomCreated { room_id: room.id, name };
                    if let Ok(text) = serde_json::to_string(&event) {
                        state.send_to_user(&user_id, Message::Text(text));
                    }
                }
                Err(e) => {
                    let event = WsEvent::Error { message: e.to_string() };
                    if let Ok(text) = serde_json::to_string(&event) {
                        state.send_to_user(&user_id, Message::Text(text));
                    }
                }
            }
        }
        WsMessage::JoinRoom { room_id } => {
            state.room_members.entry(room_id).or_default().insert(user_id);
            let _ = state.supabase.join_room(room_id, user_id).await;

            let event = WsEvent::UserJoined { room_id, user_id };
            if let Ok(text) = serde_json::to_string(&event) {
                state.broadcast_to_room(room_id, Message::Text(text)).await;
            }
        }
        WsMessage::LeaveRoom { room_id } => {
            if let Some(mut members) = state.room_members.get_mut(&room_id) {
                members.remove(&user_id);
            }
            let event = WsEvent::UserLeft { room_id, user_id };
            if let Ok(text) = serde_json::to_string(&event) {
                state.broadcast_to_room(room_id, Message::Text(text)).await;
            }
        }
        WsMessage::SendMessage { room_id, content } => {
            match state.supabase.save_message(room_id, user_id, &content).await {
                Ok(saved_msg) => {
                    let event = WsEvent::MessageReceived {
                        id: saved_msg.id,
                        room_id: saved_msg.room_id,
                        sender_id: saved_msg.sender_id,
                        content: saved_msg.content,
                        created_at: saved_msg.created_at,
                    };
                    if let Ok(text) = serde_json::to_string(&event) {
                        state.broadcast_to_room(room_id, Message::Text(text)).await;
                    }
                }
                Err(e) => {
                    let event = WsEvent::Error { message: e.to_string() };
                    if let Ok(text) = serde_json::to_string(&event) {
                        state.send_to_user(&user_id, Message::Text(text));
                    }
                }
            }
        }
        WsMessage::PrivateMessage { recipient_id, content } => {
            // Find or create private room
            let room_id = match state.supabase.find_private_room(user_id, recipient_id).await {
                Ok(Some(id)) => id,
                _ => {
                    match state.supabase.create_room(None, true).await {
                        Ok(room) => {
                            let _ = state.supabase.join_room(room.id, user_id).await;
                            let _ = state.supabase.join_room(room.id, recipient_id).await;
                            room.id
                        }
                        Err(e) => {
                            let event = WsEvent::Error { message: e.to_string() };
                            if let Ok(text) = serde_json::to_string(&event) {
                                state.send_to_user(&user_id, Message::Text(text));
                            }
                            return;
                        }
                    }
                }
            };

            // Save and send message
            match state.supabase.save_message(room_id, user_id, &content).await {
                Ok(saved_msg) => {
                    let event = WsEvent::MessageReceived {
                        id: saved_msg.id,
                        room_id: saved_msg.room_id,
                        sender_id: saved_msg.sender_id,
                        content: saved_msg.content,
                        created_at: saved_msg.created_at,
                    };
                    if let Ok(text) = serde_json::to_string(&event) {
                        state.send_to_user(&user_id, Message::Text(text.clone()));
                        state.send_to_user(&recipient_id, Message::Text(text));
                    }
                }
                Err(e) => {
                    let event = WsEvent::Error { message: e.to_string() };
                    if let Ok(text) = serde_json::to_string(&event) {
                        state.send_to_user(&user_id, Message::Text(text));
                    }
                }
            }
        }
        WsMessage::PriceUpdate { symbol, price } => {
            let event = WsEvent::PriceChanged { symbol, price };
            if let Ok(text) = serde_json::to_string(&event) {
                // Broadcast price update to everyone for demo
                for entry in state.user_connections.iter() {
                    let _ = entry.value().send(Message::Text(text.clone()));
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "rust_websocket_service=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let supabase_url = env::var("SUPABASE_URL").expect("SUPABASE_URL must be set");
    let supabase_key = env::var("SUPABASE_ANON_KEY").expect("SUPABASE_ANON_KEY must be set");

    let supabase = SupabaseClient::new(supabase_url, supabase_key);
    let state = Arc::new(AppState::new(supabase));

    let app = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/ws", get(ws_handler))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
