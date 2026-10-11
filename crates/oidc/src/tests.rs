// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr
// Added October 10, 2026. All keys and identities here are synthetic test fixtures.

use super::*;
use chrono::{TimeZone, Utc};
use openidconnect::{
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreIdTokenFields, CoreJsonWebKeySet,
        CoreJwsSigningAlgorithm, CoreRsaPrivateSigningKey, CoreTokenResponse, CoreTokenType,
    },
    AccessToken, Audience, EmptyAdditionalClaims, EmptyExtraTokenFields, PrivateSigningKey,
    StandardClaims, SubjectIdentifier,
};

fn now() -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000, 0).single().unwrap()
}

struct Fixture {
    config: Config,
    client: Client,
    signing_key: CoreRsaPrivateSigningKey,
}

impl Fixture {
    fn new() -> Self {
        let config = Config::new(
            "https://id.example",
            "client",
            None,
            "https://app.example/callback",
            BTreeMap::from([("owner".into(), "admin".into())]),
        )
        .unwrap();
        let signing_key = CoreRsaPrivateSigningKey::from_pem(
            include_str!("../tests/fixtures/test-only-rsa.pem"),
            None,
        )
        .unwrap();
        let metadata: CoreProviderMetadata = serde_json::from_value(serde_json::json!({
            "issuer": "https://id.example",
            "authorization_endpoint": "https://id.example/authorize",
            "token_endpoint": "https://id.example/token",
            "jwks_uri": "https://id.example/jwks",
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        }))
        .unwrap();
        let metadata = metadata.set_jwks(CoreJsonWebKeySet::new(vec![
            signing_key.as_verification_key()
        ]));
        let client = CoreClient::from_provider_metadata(metadata, config.client_id.clone(), None);
        Self {
            config,
            client,
            signing_key,
        }
    }

    fn tokens(
        &self,
        issuer: &str,
        audience: &str,
        expiry: i64,
        nonce: &str,
        subject: &str,
    ) -> CoreTokenResponse {
        let access = AccessToken::new("synthetic-access-token".into());
        let claims = CoreIdTokenClaims::new(
            IssuerUrl::new(issuer.into()).unwrap(),
            vec![Audience::new(audience.into())],
            now() + chrono::Duration::seconds(expiry),
            now(),
            StandardClaims::new(SubjectIdentifier::new(subject.into())),
            EmptyAdditionalClaims {},
        )
        .set_nonce(Some(Nonce::new(nonce.into())));
        let token = CoreIdToken::new(
            claims,
            &self.signing_key,
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            Some(&access),
            None,
        )
        .unwrap();
        CoreTokenResponse::new(
            access,
            CoreTokenType::Bearer,
            CoreIdTokenFields::new(Some(token), EmptyExtraTokenFields {}),
        )
    }

    fn verify(&self, tokens: &CoreTokenResponse) -> OidcResult<Identity> {
        self.config.verify_tokens(
            &self.client.id_token_verifier().set_time_fn(now),
            &Nonce::new("nonce".into()),
            tokens,
        )
    }

    fn valid(&self) -> CoreTokenResponse {
        self.tokens("https://id.example", "client", 300, "nonce", "owner")
    }
}

#[test]
fn signed_authorized_identity_maps_to_the_existing_local_username() {
    let fixture = Fixture::new();
    assert_eq!(fixture.verify(&fixture.valid()).unwrap().username, "admin");
}

#[test]
fn wrong_issuer_is_rejected() {
    let fixture = Fixture::new();
    assert!(fixture
        .verify(&fixture.tokens("https://other.example", "client", 300, "nonce", "owner"))
        .is_err());
}

#[test]
fn wrong_audience_is_rejected() {
    let fixture = Fixture::new();
    assert!(fixture
        .verify(&fixture.tokens("https://id.example", "other-client", 300, "nonce", "owner"))
        .is_err());
}

#[test]
fn expired_token_is_rejected() {
    let fixture = Fixture::new();
    assert!(fixture
        .verify(&fixture.tokens("https://id.example", "client", -1, "nonce", "owner"))
        .is_err());
}

#[test]
fn wrong_nonce_is_rejected() {
    let fixture = Fixture::new();
    assert!(fixture
        .verify(&fixture.tokens("https://id.example", "client", 300, "wrong-nonce", "owner"))
        .is_err());
}

#[test]
fn unlisted_subject_is_rejected() {
    let fixture = Fixture::new();
    assert!(fixture
        .verify(&fixture.tokens("https://id.example", "client", 300, "nonce", "someone-else"))
        .is_err());
}

#[test]
fn modified_signature_is_rejected() {
    let fixture = Fixture::new();
    let mut json = serde_json::to_value(fixture.valid()).unwrap();
    let jwt = json["id_token"].as_str().unwrap();
    let (body, signature) = jwt.rsplit_once('.').unwrap();
    let replacement = if signature.starts_with('A') { "B" } else { "A" };
    json["id_token"] = format!("{body}.{replacement}{}", &signature[1..]).into();
    assert!(fixture
        .verify(&serde_json::from_value(json).unwrap())
        .is_err());
}

#[test]
fn access_token_hash_mismatch_is_rejected() {
    let fixture = Fixture::new();
    let mut tokens = fixture.valid();
    tokens.set_access_token(AccessToken::new("different-access-token".into()));
    assert!(fixture.verify(&tokens).is_err());
}
