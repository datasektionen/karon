//! Functions related directly to SSO and Hive integration.

use std::{env, fmt};

use reqwest::StatusCode;
use serde_json::json;

use crate::{server::Error, voteit::Permissions};

/// The types of memberships a chapter member can have.
#[derive(Copy, Clone, Debug)]
pub enum MemberTypes {
    Ordinary,
    Alumni,
    Guest,
    Junior,
    Honorary,
    NonMember,
}

impl fmt::Display for MemberTypes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ordinary => write!(f, "Ordinary"),
            Self::Alumni => write!(f, "Alumni"),
            Self::Guest => write!(f, "Guest"),
            Self::Junior => write!(f, "Junior"),
            Self::Honorary => write!(f, "Honorary"),
            Self::NonMember => write!(f, "NonMember"),
        }
    }
}

impl From<MemberTypes> for Permissions {
    fn from(value: MemberTypes) -> Self {
        match value {
            MemberTypes::Ordinary => {
                Permissions::default()
                    | Permissions::DISCUSSER
                    | Permissions::PROPOSER
                    | Permissions::VOTER
            }
            MemberTypes::Alumni => {
                Permissions::default() | Permissions::DISCUSSER | Permissions::PROPOSER
            }
            MemberTypes::Guest => Permissions::default() | Permissions::DISCUSSER,
            MemberTypes::Junior => Permissions::default() | Permissions::DISCUSSER,
            MemberTypes::Honorary => Permissions::default() | Permissions::DISCUSSER,
            MemberTypes::NonMember => Permissions::no_permissions(),
        }
    }
}

impl From<&str> for MemberTypes {
    fn from(value: &str) -> Self {
        match value {
            "honorary" => Self::Honorary,
            "junior" => Self::Junior,
            "guest" => Self::Guest,
            "alumni" => Self::Alumni,
            "ordinary" => Self::Ordinary,
            _ => Self::NonMember,
        }
    }
}

/// Adds a card uid to a member in SSO.
pub async fn onboard(card_uid: &str, kth_id: &str) -> Result<(), Error> {
    reqwest::Client::new()
        .post(format!(
            "{}/api/nfc",
            &env::var("SSO_URL").expect("SSO URL not found.")
        ))
        .json(&json!({
            "kthid": kth_id,
            "nfc_id": card_uid
        }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Representation of a member with the necessary information for Karon.
#[derive(Debug, Clone)]
pub struct Member {
    pub kth_id: String,
    pub name: String,
    pub email: String,
    pub member_type: MemberTypes,
    pub picture: String,
}

impl From<SsoMember> for Member {
    fn from(value: SsoMember) -> Self {
        let email = format!("{}@kth.se", value.kthid);
        Member {
            kth_id: value.kthid,
            name: format!("{} {}", value.first_name, value.family_name),
            email,
            member_type: value.membership.as_str().into(),
            picture: value.picture,
        }
    }
}

/// Representation of a member in our membership database.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SsoMember {
    pub kthid: String,
    pub first_name: String,
    pub family_name: String,
    pub picture: String,
    pub membership: String,
}

/// Requests [`Member`] information from the chapter's membership database.
pub async fn get_member_info(card_uid: &str) -> Result<Member, Error> {
    let b: SsoMember = reqwest::Client::new()
        .get(format!(
            "{}/api/users?format=single&picture=full&u={card_uid}",
            &env::var("SSO_URL").expect("SSO URL not found.")
        ))
        .send()
        .await?
        .error_for_status()
        .map_err(|error| {
            if let Some(status) = error.status()
                && status == StatusCode::NOT_FOUND
            {
                Error::CardNotExisting(String::from(card_uid))
            } else {
                error.into()
            }
        })?
        .json()
        .await?;

    Ok(b.into())
}

/// Checks Hive if a user should be made a VoteIT moderator.
pub async fn check_moderator(kth_id: &str) -> Result<bool, Error> {
    const VOTEIT_MOD_PERM_ID: &str = "moderator";

    match reqwest::Client::new()
        .get(format!(
            "{}/api/v1/user/{kth_id}/permission/{VOTEIT_MOD_PERM_ID}",
            &env::var("HIVE_API_URL").expect("Hive URL not found.")
        ))
        .bearer_auth(&env::var("HIVE_API_KEY").expect("Hive KEY not found."))
        .send()
        .await?
        .text()
        .await?
        .as_str()
    {
        "true" => Ok(true),
        _ => Ok(false),
    }
}
