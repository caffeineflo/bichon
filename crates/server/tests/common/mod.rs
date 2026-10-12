// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr

use bichon_core::{
    context::Initialize, settings::dir::DataDirManager, users::manager::UserManager,
};

// Each authentication mode runs in its own integration-test process because
// the server reads OIDC configuration once at startup.
pub async fn setup(oidc: bool) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("BICHON_ROOT_DIR", root.path());
    std::env::set_var("BICHON_ENCRYPT_PASSWORD", "synthetic-test-password");
    if oidc {
        for (key, value) in [
            ("BICHON_OIDC_ISSUER", "https://issuer.invalid"),
            ("BICHON_OIDC_CLIENT_ID", "synthetic-client"),
            ("BICHON_OIDC_CLIENT_SECRET", ""),
            (
                "BICHON_OIDC_REDIRECT_URL",
                "https://archive.invalid/api/auth/oidc/callback",
            ),
            ("BICHON_OIDC_USER_MAPPING", r#"{"owner":"admin"}"#),
        ] {
            std::env::set_var(key, value);
        }
    } else {
        std::env::remove_var("BICHON_OIDC_ISSUER");
    }
    DataDirManager::initialize().await.unwrap();
    UserManager::initialize().await.unwrap();
    root
}
