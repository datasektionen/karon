use crate::server::Error;

pub async fn onboard(card_uid: String, kth_id: String) -> std::io::Result<()> {
    todo!()
}

pub async fn get_kth_id(card_uid: String) -> Result<String, Error> {
    match card_uid.as_str() {
        "04:5D:31:02:E4:11:90" => Ok("osen".to_string()),
        "ED:3C:4C:25" => Ok("frblo".to_string()),
        _ => Err(Error::CardNotExisting(card_uid.clone())),
    }
}
