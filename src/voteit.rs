use std::env;

use bitflags::{bitflags, bitflags_match};
use serde_json::{Value, json};

pub type VoteItToken = String;

pub struct VoteItRequest {
    pub token: VoteItToken,
    pub email: String,
    pub perms: Permissions,
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub struct Permissions: u8 {
        const PARTICIPANT = 0b00001;
        const DISCUSSER   = 0b00010;
        const PROPOSER    = 0b00100;
        const VOTER       = 0b01000;
        const MODERATOR   = 0b10000;
    }
}

impl Permissions {
    fn to_voteit_strings(&self) -> Vec<&'static str> {
        self.iter()
            .filter_map(|b| {
                bitflags_match!(b, {
                    Permissions::PARTICIPANT => Some("pa"),
                    Permissions::DISCUSSER => Some("di"),
                    Permissions::PROPOSER => Some("pr"),
                    Permissions::VOTER => Some("vo"),
                    Permissions::MODERATOR => Some("mo"),
                    _ => None,
                })
            })
            .collect()
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::PARTICIPANT
    }
}

pub async fn check_in(req: VoteItRequest) -> std::io::Result<()> {
    update_attendance(req).await
}

pub async fn check_out(mut req: VoteItRequest) -> std::io::Result<()> {
    req.perms &= Permissions::MODERATOR;
    req.perms |= Permissions::default();

    update_attendance(req).await
}

async fn update_attendance(req: VoteItRequest) -> std::io::Result<()> {
    let body = body_builder(req.email, req.perms);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Api-Key {}", req.token))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?,
    );

    reqwest::Client::new()
        .post(format!(
            "{}/token-api/invites/",
            &env::var("VOTEIT_URL").expect("VoteIT url not found")
        ))
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e))?;

    Ok(())
}

fn body_builder(email: String, permissions: Permissions) -> Value {
    let permission_strings = permissions.to_voteit_strings();
    json!({
        "roles": permission_strings,
        "data": [
            {"email": email}
        ]
    })
}
