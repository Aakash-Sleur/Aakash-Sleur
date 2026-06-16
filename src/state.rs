use std::collections::HashSet;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::mpsc;
use uuid::Uuid;
use axum::extract::ws::Message;

pub type Tx = mpsc::UnboundedSender<Message>;

pub struct AppState {
    // Maps User ID to their active connection's sender
    pub user_connections: DashMap<Uuid, Tx>,
    // Maps Room ID to a set of User IDs currently in that room
    pub room_members: DashMap<Uuid, HashSet<Uuid>>,
    // Supabase client
    pub supabase: crate::supabase::SupabaseClient,
}

impl AppState {
    pub fn new(supabase: crate::supabase::SupabaseClient) -> Self {
        Self {
            user_connections: DashMap::new(),
            room_members: DashMap::new(),
            supabase,
        }
    }

    pub async fn broadcast_to_room(&self, room_id: Uuid, msg: Message) {
        if let Some(members) = self.room_members.get(&room_id) {
            for user_id in members.iter() {
                if let Some(tx) = self.user_connections.get(user_id) {
                    let _ = tx.send(msg.clone());
                }
            }
        }
    }

    pub fn send_to_user(&self, user_id: &Uuid, msg: Message) {
        if let Some(tx) = self.user_connections.get(user_id) {
            let _ = tx.send(msg);
        }
    }
}

pub type SharedState = Arc<AppState>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supabase::SupabaseClient;

    #[tokio::test]
    async fn test_state_management() {
        let supabase = SupabaseClient::new("http://localhost".into(), "key".into());
        let state = AppState::new(supabase);
        let user_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Test connection management
        state.user_connections.insert(user_id, tx);
        assert!(state.user_connections.contains_key(&user_id));

        // Test room membership
        state.room_members.entry(room_id).or_default().insert(user_id);
        assert!(state.room_members.get(&room_id).unwrap().contains(&user_id));

        // Test send to user
        let msg = Message::Text("hello".into());
        state.send_to_user(&user_id, msg);
        let received = rx.recv().await.unwrap();
        assert_eq!(received.into_text().unwrap(), "hello");
    }
}
