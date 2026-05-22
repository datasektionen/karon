use actix_web::{HttpRequest, HttpResponse, http::Error, post};

#[post("/card")]
async fn card_api(req: HttpRequest) -> Result<HttpResponse, Error> {
    println!("hello boyo");
    todo!()
}
