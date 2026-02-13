use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Room {
    pub id: Uuid,
    pub name: Option<String>,
    pub is_private: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoomMember {
    pub room_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub user: User,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    CreateRoom { name: String },
    JoinRoom { room_id: Uuid },
    LeaveRoom { room_id: Uuid },
    SendMessage { room_id: Uuid, content: String },
    PrivateMessage { recipient_id: Uuid, content: String },
    PriceUpdate { symbol: String, price: f64 },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    MessageReceived {
        id: Uuid,
        room_id: Uuid,
        sender_id: Uuid,
        content: String,
        created_at: DateTime<Utc>
    },
    RoomCreated { room_id: Uuid, name: String },
    UserJoined { room_id: Uuid, user_id: Uuid },
    UserLeft { room_id: Uuid, user_id: Uuid },
    Notification { message: String },
    PriceChanged { symbol: String, price: f64 },
    Error { message: String },
}
