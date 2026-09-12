use std::env;

use openidconnect::{
    AdditionalClaims, Client, ClientId, ClientSecret, EmptyAdditionalClaims, EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet, IdTokenFields, IssuerUrl, RedirectUrl, RevocationErrorResponseType, StandardErrorResponse, StandardTokenIntrospectionResponse, StandardTokenResponse, core::{
        CoreAuthDisplay, CoreAuthPrompt, CoreErrorResponseType, CoreGenderClaim, CoreJsonWebKey,
        CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm, CoreProviderMetadata,
        CoreRevocableToken, CoreTokenType,
    }, reqwest
};
use serde::{Deserialize, Serialize};

pub type OIDCClientType = openidconnect::Client<
    SSOAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    AuthTokenResponse,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, CoreTokenType>,
    CoreRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub type AuthTokenResponse = StandardTokenResponse<
    IdTokenFields<
        SSOAdditionalClaims,
        EmptyExtraTokenFields,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >,
    CoreTokenType,
>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HivePermission {
    pub id: String,
    pub scope: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SSOAdditionalClaims {
    pub permissions: Vec<HivePermission>
}

impl AdditionalClaims for SSOAdditionalClaims {}

#[derive(Clone)]
pub struct OIDCClient {
    pub client: OIDCClientType,
    pub http_client: reqwest::Client,
}

impl OIDCClient {
    pub async fn new() -> Result<Self, String> {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Client should build");

        let provider =
            IssuerUrl::new(env::var("OIDC_PROVIDER").expect("OIDC_PROVIDER to be set")).unwrap();

        let provider_metadata = CoreProviderMetadata::discover_async(provider, &http_client)
            .await
            .map_err(|_| "temp".to_string())?;

        let client = Client::from_provider_metadata(
            provider_metadata,
            ClientId::new(env::var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID to be set")),
            Some(ClientSecret::new(
                env::var("OIDC_CLIENT_SECRET").expect("OIDC_CLIENT_SECRET to be set"),
            )),
        )
        .set_redirect_uri(
            RedirectUrl::new(env::var("OIDC_REDIRECT_URL").expect("OIDC_REDIRECT_URL to be set"))
                .expect("Failed to create redirect url"),
        );

        Ok(Self {
            client,
            http_client,
        })
    }
}
