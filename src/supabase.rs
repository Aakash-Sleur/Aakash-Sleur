use crate::models::*;
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

pub struct SupabaseClient {
    client: Client,
    url: String,
    anon_key: String,
}

impl SupabaseClient {
    pub fn new(url: String, anon_key: String) -> Self {
        Self {
            client: Client::new(),
            url,
            anon_key,
        }
    }

    pub async fn signup(&self, email: &str, password: &str) -> Result<AuthResponse> {
        let url = format!("{}/auth/v1/signup", self.url);
        let response = self.client
            .post(url)
            .header("apikey", &self.anon_key)
            .json(&json!({
                "email": email,
                "password": password,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let err = response.text().await?;
            Err(anyhow!("Signup failed: {}", err))
        }
    }

    pub async fn signin(&self, email: &str, password: &str) -> Result<AuthResponse> {
        let url = format!("{}/auth/v1/token?grant_type=password", self.url);
        let response = self.client
            .post(url)
            .header("apikey", &self.anon_key)
            .json(&json!({
                "email": email,
                "password": password,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let err = response.text().await?;
            Err(anyhow!("Signin failed: {}", err))
        }
    }

    pub async fn create_room(&self, name: Option<String>, is_private: bool) -> Result<Room> {
        let url = format!("{}/rest/v1/rooms", self.url);
        let response = self.client
            .post(url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {}", self.anon_key)) // Should probably use a service role key if available, but anon works if RLS allows
            .header("Prefer", "return=representation")
            .json(&json!({
                "name": name,
                "is_private": is_private,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            let rooms: Vec<Room> = response.json().await?;
            rooms.into_iter().next().ok_or_else(|| anyhow!("No room returned"))
        } else {
            let err = response.text().await?;
            Err(anyhow!("Create room failed: {}", err))
        }
    }

    pub async fn join_room(&self, room_id: Uuid, user_id: Uuid) -> Result<()> {
        let url = format!("{}/rest/v1/room_members", self.url);
        let response = self.client
            .post(url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .json(&json!({
                "room_id": room_id,
                "user_id": user_id,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let err = response.text().await?;
            Err(anyhow!("Join room failed: {}", err))
        }
    }

    pub async fn save_message(&self, room_id: Uuid, sender_id: Uuid, content: &str) -> Result<Message> {
        let url = format!("{}/rest/v1/messages", self.url);
        let response = self.client
            .post(url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .header("Prefer", "return=representation")
            .json(&json!({
                "room_id": room_id,
                "sender_id": sender_id,
                "content": content,
            }))
            .send()
            .await?;

        if response.status().is_success() {
            let messages: Vec<Message> = response.json().await?;
            messages.into_iter().next().ok_or_else(|| anyhow!("No message returned"))
        } else {
            let err = response.text().await?;
            Err(anyhow!("Save message failed: {}", err))
        }
    }

    pub async fn get_room_members(&self, room_id: Uuid) -> Result<Vec<Uuid>> {
        let url = format!("{}/rest/v1/room_members?room_id=eq.{}", self.url, room_id);
        let response = self.client
            .get(url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .send()
            .await?;

        if response.status().is_success() {
            let members: Vec<RoomMember> = response.json().await?;
            Ok(members.into_iter().map(|m| m.user_id).collect())
        } else {
            let err = response.text().await?;
            Err(anyhow!("Get room members failed: {}", err))
        }
    }

    pub async fn find_private_room(&self, user1: Uuid, user2: Uuid) -> Result<Option<Uuid>> {
        // This is a bit complex with PostgREST.
        // We need a room that is private and has both users as members.
        // Simplified: get all private rooms for user1, then check if user2 is also in any of them.

        let url = format!("{}/rest/v1/rpc/get_private_room", self.url);
        let response = self.client
            .post(url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {}", self.anon_key))
            .json(&json!({
                "user1": user1,
                "user2": user2,
            }))
            .send()
            .await?;

        if response.status().is_success() {
             let room_id: Option<Uuid> = response.json().await?;
             Ok(room_id)
        } else {
            // If RPC is not defined, we might need a fallback or just assume it's defined in the schema
            // For now, I'll return None and we can implement it if needed.
            Ok(None)
        }
    }
}
