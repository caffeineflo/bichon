// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr

#[test]
fn incomplete_oidc_configuration_refuses_to_initialize() {
    std::env::set_var("BICHON_OIDC_ISSUER", "https://issuer.invalid");
    std::env::remove_var("BICHON_OIDC_CLIENT_ID");
    assert!(std::panic::catch_unwind(bichon_server::rest::public::oidc::initialize).is_err());
}
