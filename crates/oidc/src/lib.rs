// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr
// Added October 10, 2026. Independent of Bichon storage and user models.

use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use std::{collections::BTreeMap, env, error::Error, time::Duration};

pub type OidcResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct Config {
    issuer: IssuerUrl,
    client_id: ClientId,
    client_secret: Option<ClientSecret>,
    redirect: RedirectUrl,
    users: BTreeMap<String, String>,
    http: reqwest::Client,
}

pub struct Flow {
    pub state: CsrfToken,
    nonce: Nonce,
    verifier: PkceCodeVerifier,
}

pub struct Identity {
    pub subject: String,
    pub username: String,
}

impl Config {
    pub fn redirect_origin(&self) -> String {
        self.redirect.url().origin().ascii_serialization()
    }

    pub fn from_env() -> OidcResult<Option<Self>> {
        let issuer = match env::var("BICHON_OIDC_ISSUER") {
            Ok(value) if !value.is_empty() => value,
            Ok(_) | Err(env::VarError::NotPresent) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        Ok(Some(Self::new(
            &issuer,
            &env::var("BICHON_OIDC_CLIENT_ID")?,
            match env::var("BICHON_OIDC_CLIENT_SECRET") {
                Ok(value) => Some(value).filter(|v| !v.is_empty()),
                Err(env::VarError::NotPresent) => None,
                Err(env::VarError::NotUnicode(_)) => {
                    return Err("OIDC client secret must be UTF-8".into())
                }
            },
            &env::var("BICHON_OIDC_REDIRECT_URL")?,
            serde_json::from_str(&env::var("BICHON_OIDC_USER_MAPPING")?)?,
        )?))
    }

    pub fn new(
        issuer: &str,
        client_id: &str,
        secret: Option<String>,
        redirect: &str,
        users: BTreeMap<String, String>,
    ) -> OidcResult<Self> {
        for value in [issuer, redirect] {
            let url = url::Url::parse(value)?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("OIDC issuer and redirect must be HTTPS URLs without credentials, query or fragment".into());
            }
        }
        if client_id.is_empty()
            || users.is_empty()
            || users
                .iter()
                .any(|(subject, username)| subject.is_empty() || username.is_empty())
        {
            return Err(
                "OIDC requires a client ID and explicit subject-to-local-user mappings".into(),
            );
        }
        Ok(Self {
            issuer: IssuerUrl::new(issuer.to_owned())?,
            client_id: ClientId::new(client_id.to_owned()),
            client_secret: secret.map(ClientSecret::new),
            redirect: RedirectUrl::new(redirect.to_owned())?,
            users,
            http: reqwest::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(15))
                .build()?,
        })
    }

    async fn client(&self) -> OidcResult<Client> {
        // Refresh discovery/JWKS for each flow so provider key rotation works.
        let metadata =
            CoreProviderMetadata::discover_async(self.issuer.clone(), &self.http).await?;
        Ok(CoreClient::from_provider_metadata(
            metadata,
            self.client_id.clone(),
            self.client_secret.clone(),
        )
        .set_redirect_uri(self.redirect.clone()))
    }

    pub async fn authorize(&self) -> OidcResult<(url::Url, Flow)> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .add_scope(Scope::new("email".into()))
            .set_pkce_challenge(challenge)
            .url();
        Ok((
            url,
            Flow {
                state,
                nonce,
                verifier,
            },
        ))
    }

    pub async fn authenticate(&self, flow: Flow, code: String) -> OidcResult<Identity> {
        let client = self.client().await?;
        let tokens = client
            .exchange_code(AuthorizationCode::new(code))?
            .set_pkce_verifier(flow.verifier)
            .request_async(&self.http)
            .await?;
        let verifier = client.id_token_verifier();
        self.verify_tokens(&verifier, &flow.nonce, &tokens)
    }

    fn verify_tokens(
        &self,
        verifier: &openidconnect::core::CoreIdTokenVerifier<'_>,
        nonce: &Nonce,
        tokens: &openidconnect::core::CoreTokenResponse,
    ) -> OidcResult<Identity> {
        let token = tokens
            .id_token()
            .ok_or("OIDC token response has no ID token")?;
        // The SDK checks signature, issuer, audience, expiration and nonce.
        let claims = token.claims(verifier, nonce)?;
        if let Some(expected) = claims.access_token_hash() {
            let actual = AccessTokenHash::from_token(
                tokens.access_token(),
                token.signing_alg()?,
                token.signing_key(verifier)?,
            )?;
            if actual != *expected {
                return Err("OIDC access token hash does not match".into());
            }
        }
        let subject = claims.subject().as_str();
        let username = self
            .users
            .get(subject)
            .ok_or("OIDC subject is not authorized")?;
        Ok(Identity {
            subject: subject.into(),
            username: username.clone(),
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod config_tests {
    use super::*;

    fn users() -> BTreeMap<String, String> {
        BTreeMap::from([("owner-subject".into(), "admin".into())])
    }

    #[test]
    fn insecure_provider_is_rejected() {
        assert!(Config::new(
            "http://id.example",
            "client",
            None,
            "https://app.example/callback",
            users()
        )
        .is_err());
    }

    #[test]
    fn missing_subject_mapping_is_rejected() {
        assert!(Config::new(
            "https://id.example",
            "client",
            None,
            "https://app.example/callback",
            BTreeMap::new()
        )
        .is_err());
    }

    #[test]
    fn provider_configuration_is_not_pocket_id_specific() {
        assert!(Config::new(
            "https://any-provider.example/realm/home",
            "client",
            Some("test-secret".into()),
            "https://app.example/archive/api/auth/oidc/callback",
            users()
        )
        .is_ok());
    }
}
