//! Functions related directly to VoteIT integration.

use std::{env, fmt::Display};

use bitflags::{bitflags, bitflags_match};
use serde_json::{Value, json};
use sqlx::types::Uuid;

use crate::server::Error;

pub type VoteItToken = String;

/// Necessary information for a request to VoteIT.
pub struct VoteItRequest {
    pub meeting_id: Uuid,
    pub voteit_token: VoteItToken,
    pub email: String,
    pub perms: Permissions,
}

impl Display for VoteItRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at meeting {}", self.email, self.meeting_id)
    }
}

bitflags! {
    /// The types of permissions which exists for users in VoteIT.
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
    /// Translates the VoteIT permissions to their string representations which VoteIT uses.
    pub fn to_voteit_strings(&self) -> Vec<&'static str> {
        self.iter()
            .filter_map(|b| {
                bitflags_match!(b, {
                    Permissions::PARTICIPANT => Some("pa"),
                    Permissions::DISCUSSER => Some("di"),
                    Permissions::PROPOSER => Some("pr"),
                    Permissions::VOTER => Some("pv"),
                    Permissions::MODERATOR => Some("mo"),
                    _ => None,
                })
            })
            .collect()
    }

    /// Returns [`Permissions`] with no rightsno rights.
    pub fn no_permissions() -> Self {
        Self::empty()
    }

    /// Checks if a [`Permissions`] has voting rights.
    pub fn has_suffrage(&self) -> bool {
        self.contains(Permissions::VOTER)
    }
}

impl Default for Permissions {
    /// Returns [`Permissions`] with only the right to participate in a VoteIT meeting.
    fn default() -> Self {
        Self::PARTICIPANT
    }
}

/// Updates a persons attendance in VoteIT via the VoteIT `token-api`.
pub async fn update_attendance(req: &VoteItRequest) -> Result<(), Error> {
    let body = body_builder(&req.email, req.perms);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Api-Key {}", req.voteit_token))
            .map_err(|_| Error::ReqestParseError(req.meeting_id.to_string()))?,
    );

    // TODO: reuse client
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

/// Builds the request body for sending to VoteIT.
fn body_builder(email: &str, permissions: Permissions) -> Value {
    let permission_strings = permissions.to_voteit_strings();
    json!({
        "roles": permission_strings,
        "data": [
            {"email": email}
        ]
    })
}
