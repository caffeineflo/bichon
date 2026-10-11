// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr
// Added October 10, 2026. Adapter to Bichon's existing users and sessions.

use bichon_core::{
    settings::cli::SETTINGS,
    token::AccessTokenModel,
    users::{LoginResult, UserModel},
};
use bichon_oidc::{Config, Flow};
use http::StatusCode;
use poem::{handler, web::Query, IntoResponse, Request, Response};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

const FLOW_COOKIE: &str = "__Host-bichon-oidc-flow";
const SESSION_COOKIE: &str = "__Host-bichon-oidc-session";
const MAX_PENDING: usize = 1024;
static CONFIG: LazyLock<Option<Config>> =
    LazyLock::new(|| Config::from_env().expect("Invalid Bichon OIDC configuration"));
static FLOWS: LazyLock<Mutex<Pending<Flow>>> = LazyLock::new(|| Mutex::new(Pending::default()));
static SESSIONS: LazyLock<Mutex<Pending<u64>>> = LazyLock::new(|| Mutex::new(Pending::default()));

struct Pending<T>(HashMap<String, (Instant, T)>);

impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<T> Pending<T> {
    fn insert(
        &mut self,
        key: String,
        value: T,
        now: Instant,
        ttl: Duration,
    ) -> Result<(), StatusCode> {
        self.0.retain(|_, (expires, _)| *expires > now);
        if self.0.len() >= MAX_PENDING {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        self.0.insert(key, (now + ttl, value));
        Ok(())
    }

    fn take(&mut self, key: &str, now: Instant) -> Option<T> {
        self.0
            .remove(key)
            .and_then(|(expires, value)| (expires > now).then_some(value))
    }
}

fn take_bound<T>(
    pending: &mut Pending<T>,
    browser_state: Option<&str>,
    state: &str,
    now: Instant,
) -> Option<T> {
    if browser_state != Some(state) {
        return None;
    }
    pending.take(state, now)
}

pub fn initialize() {
    LazyLock::force(&CONFIG);
}
pub fn enabled() -> bool {
    CONFIG.is_some()
}

fn cookie(req: &Request, name: &str) -> Option<String> {
    req.headers()
        .get(http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_string()))
}

fn cookie_header(name: &str, value: &str, seconds: u64) -> String {
    format!("{name}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={seconds}")
}

fn failure(status: StatusCode, message: &'static str) -> Response {
    Response::builder()
        .status(status)
        .header("Cache-Control", "no-store")
        .header("Referrer-Policy", "no-referrer")
        .body(message)
}

#[handler]
pub async fn login() -> Response {
    let Some(config) = CONFIG.as_ref() else {
        return failure(StatusCode::NOT_FOUND, "OIDC is disabled.");
    };
    let (url, flow) = match config.authorize().await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!("OIDC discovery or authorization setup failed");
            return failure(
                StatusCode::BAD_GATEWAY,
                "Unable to contact the identity provider.",
            );
        }
    };
    let state = flow.state.secret().clone();
    if let Err(status) = FLOWS.lock().expect("OIDC flow lock poisoned").insert(
        state.clone(),
        flow,
        Instant::now(),
        Duration::from_secs(300),
    ) {
        return failure(status, "Too many pending sign-ins. Try again later.");
    }
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header("Location", url.as_str())
        .header("Cache-Control", "no-store")
        .header("Referrer-Policy", "no-referrer")
        .header("Set-Cookie", cookie_header(FLOW_COOKIE, &state, 300))
        .finish()
}

#[derive(Deserialize)]
pub struct Callback {
    state: String,
    code: Option<String>,
    error: Option<String>,
}

#[handler]
pub async fn callback(req: &Request, Query(query): Query<Callback>) -> Response {
    let Some(config) = CONFIG.as_ref() else {
        return failure(StatusCode::NOT_FOUND, "OIDC is disabled.");
    };
    if cookie(req, FLOW_COOKIE).as_deref() != Some(query.state.as_str()) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "Sign-in state does not match this browser.",
        );
    }
    let flow = take_bound(
        &mut FLOWS.lock().expect("OIDC flow lock poisoned"),
        cookie(req, FLOW_COOKIE).as_deref(),
        &query.state,
        Instant::now(),
    );
    let Some(flow) = flow else {
        return failure(
            StatusCode::UNAUTHORIZED,
            "Sign-in has expired or was already used.",
        );
    };
    let identity = match (query.error, query.code) {
        (None, Some(code)) => match config.authenticate(flow, code).await {
            Ok(identity) => identity,
            Err(_) => {
                tracing::warn!("OIDC token exchange, validation or subject authorization failed");
                return failure(
                    StatusCode::UNAUTHORIZED,
                    "Sign-in failed. Return to the login page and try again.",
                );
            }
        },
        _ => {
            return failure(
                StatusCode::UNAUTHORIZED,
                "The identity provider did not authorize sign-in.",
            )
        }
    };
    let user = match UserModel::list_all() {
        Ok(users) => users
            .into_iter()
            .find(|user| user.username == identity.username),
        Err(error) => {
            tracing::error!("OIDC local user lookup failed: {error}");
            return failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to load the local user.",
            );
        }
    };
    let Some(user) = user else {
        return failure(StatusCode::FORBIDDEN, "No authorized local account exists.");
    };
    let handoff = uuid::Uuid::new_v4().to_string();
    if let Err(status) = SESSIONS.lock().expect("OIDC session lock poisoned").insert(
        handoff.clone(),
        user.id,
        Instant::now(),
        Duration::from_secs(60),
    ) {
        return failure(status, "Too many pending sign-ins. Try again later.");
    }
    let base = SETTINGS.bichon_base_url.trim_end_matches('/');
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header("Location", format!("{base}/oidc-callback"))
        .header("Cache-Control", "no-store")
        .header("Referrer-Policy", "no-referrer")
        .header("Set-Cookie", cookie_header(FLOW_COOKIE, "", 0))
        .header("Set-Cookie", cookie_header(SESSION_COOKIE, &handoff, 60))
        .finish()
}

#[handler]
pub async fn session(req: &Request) -> Response {
    let Some(config) = CONFIG.as_ref() else {
        return failure(StatusCode::NOT_FOUND, "OIDC is disabled.");
    };
    if req
        .headers()
        .get(http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        != Some(config.redirect_origin().as_str())
    {
        return failure(
            StatusCode::FORBIDDEN,
            "Session exchange requires the application origin.",
        );
    }
    let Some(key) = cookie(req, SESSION_COOKIE) else {
        return failure(StatusCode::UNAUTHORIZED, "No pending sign-in.");
    };
    let user_id = SESSIONS
        .lock()
        .expect("OIDC session lock poisoned")
        .take(&key, Instant::now());
    let Some(user_id) = user_id else {
        return failure(
            StatusCode::UNAUTHORIZED,
            "Sign-in has expired or was already used.",
        );
    };
    let result = (|| {
        let user = UserModel::find(user_id)?.ok_or_else(|| {
            bichon_core::raise_error!(
                "OIDC mapped local user no longer exists".to_string(),
                bichon_core::error::code::ErrorCode::InternalError
            )
        })?;
        let (token, challenge) = if user.totp_enabled {
            (None, Some(AccessTokenModel::new_mfa_challenge(user.id)?))
        } else {
            (Some(AccessTokenModel::reset_webui_token(user.id)?), None)
        };
        Ok::<_, bichon_core::error::BichonError>(LoginResult {
            success: true,
            access_token: token,
            theme: user.theme,
            language: user.language,
            mfa_required: challenge.is_some(),
            mfa_challenge: challenge,
            error_message: None,
        })
    })();
    match result {
        Ok(result) => {
            let mut response = poem::web::Json(result).into_response();
            response
                .headers_mut()
                .insert("Cache-Control", http::HeaderValue::from_static("no-store"));
            response.headers_mut().insert(
                "Set-Cookie",
                cookie_header(SESSION_COOKIE, "", 0)
                    .parse()
                    .expect("valid session cookie"),
            );
            response
        }
        Err(error) => {
            tracing::error!("OIDC local session creation failed: {error}");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to create the local session.",
            )
        }
    }
}

#[handler]
pub async fn source() -> Response {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(
            "Location",
            format!(
                "https://github.com/caffeineflo/bichon-temporary-oidc/tree/{}",
                option_env!("BICHON_SOURCE_REVISION").unwrap_or("main")
            ),
        )
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_sign_in_can_only_be_consumed_once() {
        let mut pending = Pending::default();
        let now = Instant::now();
        pending
            .insert("state".into(), 42, now, Duration::from_secs(300))
            .unwrap();
        pending.take("state", now).unwrap();
        assert_eq!(pending.take("state", now), None);
    }

    #[test]
    fn expired_sign_in_is_rejected() {
        let mut pending = Pending::default();
        let now = Instant::now();
        pending
            .insert("state".into(), 42, now, Duration::from_secs(300))
            .unwrap();
        assert_eq!(pending.take("state", now + Duration::from_secs(300)), None);
    }

    #[test]
    fn unrelated_state_does_not_consume_the_valid_sign_in() {
        let mut pending = Pending::default();
        let now = Instant::now();
        pending
            .insert("state".into(), 42, now, Duration::from_secs(300))
            .unwrap();
        pending.take("wrong-state", now);
        assert_eq!(pending.take("state", now), Some(42));
    }

    #[test]
    fn callback_from_another_browser_is_rejected_without_consuming_the_flow() {
        let mut pending = Pending::default();
        let now = Instant::now();
        pending
            .insert("state".into(), 42, now, Duration::from_secs(300))
            .unwrap();
        assert_eq!(
            take_bound(&mut pending, Some("another-browser"), "state", now),
            None
        );
        assert_eq!(
            take_bound(&mut pending, Some("state"), "state", now),
            Some(42)
        );
    }

    #[test]
    fn callback_without_browser_cookie_is_rejected() {
        let mut pending = Pending::default();
        let now = Instant::now();
        pending
            .insert("state".into(), 42, now, Duration::from_secs(300))
            .unwrap();
        assert_eq!(take_bound(&mut pending, None, "state", now), None);
    }
}
