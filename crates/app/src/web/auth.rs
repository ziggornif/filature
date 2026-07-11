//! Demo-auth gate (slice 08). A single-operator login page + opaque session
//! cookie that protects a deployed demo instance.
//!
//! Deliberately isolated from the rest of the driving adapter: `protect` is the
//! only wiring `main.rs` calls, and it is applied as an **outer layer** around
//! `web::router` — never inside it — so the `web::router`-based e2e/it suites
//! run against the unprotected router and stay open. Enforcement is default-deny
//! with an allowlist of exactly `/login`; `/static/*` is intentionally NOT
//! exempt, so nothing of the app is served without a valid session cookie
//! except the self-contained login page.
//!
//! The credential lives in config (`[auth]` table, argon2 `password_hash`); the
//! session token is 32 random bytes minted once at boot and held in memory, so
//! a restart invalidates sessions and no secret material sits in config.

use crate::web::router::{internal_error, read_cookie};
use crate::web::templates::Renderer;
use crate::web::theme::Theme;
use axum::{
    Form, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use std::sync::Arc;
use tera::Context;

/// The operator credential, loaded from the `[auth]` config table. Kept out of
/// the main `Config` struct so the feature is purely additive (no churn to the
/// existing `Config` literals in the test suites).
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub username: String,
    /// argon2 PHC string (`$argon2id$…`), e.g. from `filature hash-password`.
    pub password_hash: String,
}

impl AuthConfig {
    /// Load the `[auth]` table from the same sources as [`crate::config::Config`]
    /// (TOML file + `FILATURE_` env with `__` nesting). A missing `[auth]` table
    /// is an error — the gate is always active in production.
    // `figment::Error` is large but this runs once at startup (see `Config::load`).
    #[allow(clippy::result_large_err)]
    pub fn load(toml_path: &str) -> Result<Self, figment::Error> {
        #[derive(Deserialize)]
        struct Wrapper {
            auth: AuthConfig,
        }
        Figment::new()
            .merge(Toml::file(toml_path))
            .merge(Env::prefixed("FILATURE_").split("__"))
            .extract::<Wrapper>()
            .map(|w| w.auth)
    }
}

/// Shared state for the auth routes and the enforcement middleware.
#[derive(Clone)]
struct AuthState {
    username: Arc<str>,
    password_hash: Arc<str>,
    /// The one valid session value for this process lifetime.
    token: Arc<str>,
    renderer: Renderer,
    default_locale: Arc<str>,
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

/// Wrap `app` in the demo-auth gate: adds `/login` (GET+POST) and `/logout`, and
/// a default-deny middleware that redirects any other path to `/login` unless a
/// valid `session` cookie is present.
pub fn protect(
    app: Router,
    auth: AuthConfig,
    renderer: Renderer,
    default_locale: String,
) -> Router {
    let state = AuthState {
        username: auth.username.into(),
        password_hash: auth.password_hash.into(),
        token: new_token().into(),
        renderer,
        default_locale: default_locale.into(),
    };
    Router::new()
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", get(logout))
        .with_state(state.clone())
        .merge(app)
        .layer(middleware::from_fn_with_state(state, enforce))
}

/// Default-deny gate: `/login` always passes; every other path requires the
/// session cookie to constant-time-match the boot token, else 303 → `/login`.
async fn enforce(State(st): State<AuthState>, req: Request, next: Next) -> Response {
    if req.uri().path() == "/login" {
        return next.run(req).await;
    }
    let authed = read_cookie(req.headers(), "session")
        .map(|c| ct_eq(c.as_bytes(), st.token.as_bytes()))
        .unwrap_or(false);
    if authed {
        next.run(req).await
    } else {
        Redirect::to("/login").into_response()
    }
}

async fn login_page(State(st): State<AuthState>, headers: HeaderMap) -> Response {
    render_login(&st, &headers, false, StatusCode::OK)
}

async fn login_submit(
    State(st): State<AuthState>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    // Always run the (constant-time) password verify so a wrong username and a
    // wrong password take the same path — no user-enumeration timing signal.
    let pass_ok = verify_password(&form.password, &st.password_hash);
    let user_ok = ct_eq(form.username.as_bytes(), st.username.as_bytes());
    if user_ok && pass_ok {
        let cookie = format!("session={}; HttpOnly; SameSite=Lax; Path=/", st.token);
        let mut res = Redirect::to("/").into_response();
        res.headers_mut().insert(
            header::SET_COOKIE,
            header::HeaderValue::from_str(&cookie).expect("ascii cookie"),
        );
        res
    } else {
        render_login(&st, &headers, true, StatusCode::UNAUTHORIZED)
    }
}

async fn logout() -> Response {
    let mut res = Redirect::to("/login").into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        header::HeaderValue::from_static("session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
    );
    res
}

fn render_login(st: &AuthState, headers: &HeaderMap, error: bool, status: StatusCode) -> Response {
    let locale = read_cookie(headers, "lang")
        .filter(|l| st.renderer.knows_locale(l))
        .unwrap_or_else(|| st.default_locale.to_string());
    let theme_attr = Theme::from_cookie(read_cookie(headers, "theme").as_deref()).data_attr();
    let mut ctx = Context::new();
    ctx.insert("error", &error);
    match st.renderer.render("login.html", &locale, theme_attr, ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => internal_error(e),
    }
}

/// Hash a password into an argon2id PHC string (`$argon2id$…`). Used by the
/// `hash-password` subcommand to produce the value that goes in `[auth]`.
pub fn hash_password(password: &str) -> String {
    use argon2::Argon2;
    use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("argon2 hashing does not fail on valid input")
        .to_string()
}

/// Verify `password` against a stored argon2 PHC `hash`. A malformed hash
/// verifies as `false` rather than panicking.
pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::Argon2;
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// 32 cryptographically-random bytes, hex-encoded — the per-boot session value.
fn new_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    let mut s = String::with_capacity(64);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Length-checked constant-time byte comparison (no early return on the first
/// differing byte). The length branch is acceptable: the compared values are
/// fixed-length here.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_verifies_correct_password_only() {
        let hash = hash_password("correct horse");
        assert!(verify_password("correct horse", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn malformed_hash_verifies_false() {
        assert!(!verify_password("x", "not-a-phc-string"));
    }

    #[test]
    fn ct_eq_matches_equal_only() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }

    #[test]
    fn token_is_64_hex_chars_and_unique() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
