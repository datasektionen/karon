use crate::{server::Error, voteit::Permissions};

#[derive(Copy, Clone, Debug)]
pub enum MemberTypes {
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

#[derive(Debug)]
pub struct Member {
    pub kth_id: String,
    pub email: String,
    pub member_type: MemberTypes,
}

pub async fn get_member_info(card_uid: &str) -> Result<Member, Error> {
    todo!()
}

pub async fn check_moderator(kth_id: &str) -> Result<bool, Error> {
    Ok(false)
}
