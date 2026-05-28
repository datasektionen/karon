use crate::{server::Error, voteit::Permissions};

enum MemberTypes {
    Ordinary,
    Alumni,
    Guest,
    Junior,
    Honorary,
    NonMember,
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

pub async fn onboard(_card_uid: &str, _kth_id: &str) -> Result<(), Error> {
    todo!()
}

pub async fn get_kth_id(card_uid: &str) -> Result<String, Error> {
    match card_uid {
        "04:5D:31:02:E4:11:90" => Ok("osen".to_string()),
        "ED:3C:4C:25" => Ok("frblo".to_string()),
        _ => Err(Error::CardNotExisting(card_uid.to_string())),
    }
}
