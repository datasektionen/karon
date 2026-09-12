use actix_session::{Session, SessionExt, SessionInsertError};
use actix_web::{
    HttpMessage, HttpRequest, HttpResponse, ResponseError,
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    get,
    http::StatusCode,
    web,
};
use jsonwebtoken::DecodingKey;
use openidconnect::{
    AccessTokenHash, AuthorizationCode, CsrfToken, Nonce, OAuth2TokenResponse, Scope,
    TokenResponse, core::CoreAuthenticationFlow,
};
use std::{future::Future, pin::Pin};
use thiserror::Error;
use tracing_log::log;

use jsonwebtoken::EncodingKey;
use serde::Deserialize;
use std::future::{Ready, ready};

use crate::{
    cookies::{CookieError, LoginContext},
    oidc::{HivePermission, OIDCClient},
};

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

#[derive(Debug, Error)]
enum AuthError {
    #[error("{0}")]
    SessionInsertError(#[from] SessionInsertError),
    #[error("login provider does not support oidc")]
    OidcMissingToken,
    #[error("oidc provider did not responde with a access token hash")]
    OidcMissingAccessTokenHash,
    #[error("failed to verify claims: {0}")]
    OidcClaimsVerificationError(#[from] openidconnect::ClaimsVerificationError),
    #[error("failed to sign token hash: {0}")]
    OidcSigningError(#[from] openidconnect::SigningError),
    #[error["failed to get signing alg from token: {0}"]]
    OidcSignatureVerificationError(#[from] openidconnect::SignatureVerificationError),
    #[error("failed to request token from oidc provider: {0}")]
    OidcRequestTokenError(
        #[source]
        openidconnect::RequestTokenError<
            openidconnect::HttpClientError<openidconnect::reqwest::Error>,
            openidconnect::StandardErrorResponse<openidconnect::core::CoreErrorResponseType>,
        >,
    ),
    #[error("token hashes did not match for subject {0}")]
    OidcInvalidTokenHash(String),
    #[error("oidc configuration error: {0}")]
    OidcConfigurationError(#[from] openidconnect::ConfigurationError),
    #[error("login request has invalid CSRF token")]
    InvalidCSRFToken,
    #[error("{0}")]
    CookieError(#[from] CookieError),
    #[error("user has insufficient permissions")]
    InsufficientPermissions,
}

impl ResponseError for AuthError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        log::error!("{}", self);
        HttpResponse::build(self.status_code()).body(self.to_string())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::OidcConfigurationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcMissingToken => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcMissingAccessTokenHash => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcClaimsVerificationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcSigningError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcSignatureVerificationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcRequestTokenError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::OidcInvalidTokenHash(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidCSRFToken => StatusCode::UNAUTHORIZED,
            Self::SessionInsertError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::CookieError(_) => StatusCode::UNAUTHORIZED,
            Self::InsufficientPermissions => StatusCode::UNAUTHORIZED,
        }
    }
}

#[get("/callback")]
pub async fn callback(
    query: web::Query<CallbackQuery>,
    oidc: web::Data<OIDCClient>,
    decoding_key: web::Data<DecodingKey>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    let login_context =
        LoginContext::extract_token(req.cookie("login_context"), decoding_key.as_ref())?;

    if *login_context.csrf_token.secret() != query.state {
        return Err(AuthError::InvalidCSRFToken);
    }

    let token_respones = oidc
        .client
        .exchange_code(AuthorizationCode::new(query.code.clone()))?
        .request_async(&oidc.http_client)
        .await
        .map_err(|e| AuthError::OidcRequestTokenError(e))?;

    let id_token = token_respones
        .id_token()
        .ok_or(AuthError::OidcMissingToken)?;

    let id_token_verifier = oidc.client.id_token_verifier();

    let claims = id_token.claims(&id_token_verifier, &login_context.nonce)?;

    let expected_access_token_hash = claims
        .access_token_hash()
        .ok_or(AuthError::OidcMissingAccessTokenHash)?;

    let actual_access_token_hash = AccessTokenHash::from_token(
        token_respones.access_token(),
        id_token.signing_alg()?,
        id_token.signing_key(&id_token_verifier)?,
    )?;

    if actual_access_token_hash != *expected_access_token_hash {
        return Err(AuthError::OidcInvalidTokenHash(
            claims.subject().to_string(),
        ));
    }

    if !claims
        .additional_claims()
        .permissions
        .iter()
        .any(|perm| perm.id == "karon-admin")
    {
        return Err(AuthError::InsufficientPermissions.into());
    }

    session.insert("kthid", claims.subject().to_string())?;

    Ok(HttpResponse::TemporaryRedirect()
        .insert_header(("location", "/"))
        .finish())
}

pub type LocalBoxFuture<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

pub struct AuthMiddleware {
    pub oidc: web::Data<OIDCClient>,
    pub encoding_key: web::Data<EncodingKey>,
}

impl AuthMiddleware {
    pub fn new(oidc: web::Data<OIDCClient>, encoding_key: web::Data<EncodingKey>) -> Self {
        AuthMiddleware { oidc, encoding_key }
    }
}

pub struct InnerAuthMiddleware<S> {
    pub oidc: web::Data<OIDCClient>,
    pub encoding_key: web::Data<EncodingKey>,
    pub service: S,
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Transform = InnerAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(InnerAuthMiddleware {
            service,
            encoding_key: self.encoding_key.clone(),
            oidc: self.oidc.clone(),
        }))
    }
}

impl<S, B> Service<ServiceRequest> for InnerAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let session = req.get_session();
        let kthid = if let Some(kthid) = session.get::<String>("kthid").unwrap() {
            kthid
        } else {
            let (auth_url, csrf_token, nonce) = self
                .oidc
                .client
                .authorize_url(
                    CoreAuthenticationFlow::AuthorizationCode,
                    CsrfToken::new_random,
                    Nonce::new_random,
                )
                .add_scope(Scope::new(String::from("permissions")))
                .url();

            let token = LoginContext::new(String::from("karon"), csrf_token, nonce);
            let cookie = token.cookie(&self.encoding_key).unwrap();

            let response = HttpResponse::TemporaryRedirect()
                .insert_header(("location", auth_url.to_string()))
                .cookie(cookie)
                .finish()
                .map_into_right_body();

            let (request, _pl) = req.into_parts();
            return Box::pin(async move { Ok(ServiceResponse::new(request, response)) });
        };

        req.extensions_mut().insert(kthid);

        let res = self.service.call(req);

        Box::pin(async move {
            let res = res.await.unwrap();

            Ok(res.map_into_left_body())
        })
    }
}
