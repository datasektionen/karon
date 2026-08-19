use actix_web::cookie::{Cookie, SameSite};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, get_current_timestamp,
};
use openidconnect::{CsrfToken, Nonce};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginContext {
    pub sub: String,
    pub nonce: Nonce,
    pub csrf_token: CsrfToken,
    pub exp: u64,
}

#[derive(Debug, Error)]
pub enum CookieError {
    #[error("failed to encode jwt: {0}")]
    JwtEncodingError(#[source] jsonwebtoken::errors::Error),
    #[error("failed to decode jwt: {0}")]
    JwtDecodingError(#[source] jsonwebtoken::errors::Error),
    #[error("failed to get session cookie")]
    MissingSessionCookie,
    #[error("trying to finish auth flow but missing context")]
    MissingLoginContext,
}

impl LoginContext {
    pub fn new(sub: String, csrf_token: CsrfToken, nonce: Nonce) -> Self {
        Self {
            sub,
            csrf_token,
            nonce,
            exp: get_current_timestamp() + 5 * 60,
        }
    }

    pub fn cookie(&'_ self, secret: &EncodingKey) -> Result<Cookie<'_>, CookieError> {
        let token = jsonwebtoken::encode(&Header::default(), &self, secret)
            .map_err(|e| CookieError::JwtEncodingError(e))?;

        Ok(Cookie::build("login_context", token)
            .same_site(SameSite::Lax)
            .secure(true)
            .http_only(true)
            .path("/")
            .finish())
    }

    pub fn extract_token(
        cookie: Option<Cookie>,
        secret: &DecodingKey,
    ) -> Result<Self, CookieError> {
        let token = cookie.ok_or(CookieError::MissingLoginContext)?;

        let token =
            jsonwebtoken::decode::<Self>(token.value(), secret, &Validation::new(Algorithm::HS256))
                .map_err(|e| CookieError::JwtDecodingError(e))?;

        Ok(token.claims)
    }
}
