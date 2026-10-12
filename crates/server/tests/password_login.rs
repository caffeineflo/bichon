// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr

mod common;

#[tokio::test]
async fn local_credentials_work_when_oidc_is_not_configured() {
    let _root = common::setup(false).await;
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
    response.assert_status_is_ok();
    let result: bichon_core::users::LoginResult = response.json().await.value().deserialize();
    assert!(result.success && result.access_token.is_some());
    let features: serde_json::Value = cli
        .get("/api/v1/features")
        .send()
        .await
        .json()
        .await
        .value()
        .deserialize();
    assert_eq!(features["oidc_only"], false);
}
