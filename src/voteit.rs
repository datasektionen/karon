use std::env;

use bitflags::{bitflags, bitflags_match};
use serde_json::{Value, json};
use sqlx::types::Uuid;

use crate::{db::Action, server::Error};

pub type VoteItToken = String;

pub struct VoteItRequest {
    pub meeting_id: Uuid,
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
    pub fn to_voteit_strings(&self) -> Vec<&'static str> {
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

    pub fn no_permissions() -> Self {
        Self::empty()
    }

    pub fn has_suffrage(&self) -> bool {
        self.contains(Permissions::VOTER)
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::PARTICIPANT
    }
}

pub async fn update_attendance(req: &VoteItRequest) -> Result<(), Error> {
    let body = body_builder(&req.email, req.perms);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Api-Key {}", req.meeting_id))
            .map_err(|_| Error::ReqestParseError(req.meeting_id.to_string()))?,
    );

    reqwest::Client::new()
        .post(format!(
            "{}/token-api/invites/",
            &env::var("VOTEIT_URL").expect("VoteIT url not found")
        ))
        .headers(headers)
        .json(&body)
        .send()
        .await?;

    Ok(())
}

fn body_builder(email: &str, permissions: Permissions) -> Value {
    let permission_strings = permissions.to_voteit_strings();
    json!({
        "roles": permission_strings,
        "data": [
            {"email": email}
        ]
    })
}
