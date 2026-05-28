use std::env;

use bitflags::bitflags;
use serde_json::{Value, json};

pub type VoteItToken = String;

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    struct Permissions: u8 {
        const PARTICIPANT = 0b00001;
        const DISCUSSER   = 0b00010;
        const PROPOSER    = 0b00100;
        const VOTER       = 0b01000;
        const MODERATOR   = 0b10000;
    }
}

const PARTICIPANT_STR: &str = "pa";
const DISCUSSER_STR: &str = "di";
const PROPOSER_STR: &str = "pr";
const VOTER_STR: &str = "vo";
const MODERATOR_STR: &str = "mo";

impl Permissions {
    fn to_voteit_strings(&self) -> Vec<&str> {
        let mut perms = Vec::with_capacity(5);
        if self.contains(Permissions::PARTICIPANT) {
            perms.push(PARTICIPANT_STR);
        }
        if self.contains(Permissions::DISCUSSER) {
            perms.push(DISCUSSER_STR);
        }
        if self.contains(Permissions::PROPOSER) {
            perms.push(PROPOSER_STR);
        }
        if self.contains(Permissions::VOTER) {
            perms.push(VOTER_STR);
        }
        if self.contains(Permissions::MODERATOR) {
            perms.push(MODERATOR_STR);
        }

        perms
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::PARTICIPANT | Self::DISCUSSER
    }
}

pub async fn check_in(
    token: VoteItToken,
    email: String,
    suffrage: bool,
    moderator: bool,
) -> std::io::Result<()> {
    let mut perms = Permissions::default();
    if suffrage {
        perms |= Permissions::VOTER | Permissions::PROPOSER;
    }
    if moderator {
        perms |= Permissions::MODERATOR;
    }

    update_attendance(token, email, perms).await
}

pub async fn check_out(token: VoteItToken, email: String, moderator: bool) -> std::io::Result<()> {
    let mut perms = Permissions::default();
    if moderator {
        perms |= Permissions::MODERATOR;
    }

    update_attendance(token, email, perms).await
}

async fn update_attendance(
    token: VoteItToken,
    email: String,
    permissions: Permissions,
) -> std::io::Result<()> {
    let body = body_builder(email, permissions);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Api-Key {}", token))
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
