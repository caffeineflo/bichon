// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr

mod common;

#[tokio::test]
async fn configured_oidc_rejects_valid_local_credentials() {
    let _root = common::setup(true).await;
    let cli = poem::test::TestClient::new(
        poem::Route::new()
            .at(
                "/api/login",
                poem::post(bichon_server::rest::public::login::login),
            )
            .at(
                "/api/v1/features",
                poem::get(bichon_server::rest::public::features::get_features),
            ),
    );
    let response = cli
        .post("/api/login")
        .body_json(&serde_json::json!({
            "username": "admin", "password": "admin@bichon"
        }))
        .send()
        .await;
    response.assert_status(http::StatusCode::FORBIDDEN);
    let features: serde_json::Value = cli
        .get("/api/v1/features")
        .send()
        .await
        .json()
        .await
        .value()
        .deserialize();
    assert_eq!(features["oidc_only"], true);
}
