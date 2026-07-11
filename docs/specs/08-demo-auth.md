## Agent Brief — 08 Demo auth gate (login + session cookie)

**Category:** feature
**Summary:** Protect a deployed demo instance behind a single login page. One
operator credential lives in config (`[auth]` table: `username` + argon2
`password_hash`). A successful `POST /login` sets a signed-nothing opaque session
cookie; every other route (including `/static/*`) is redirected to `/login` when
that cookie is absent or wrong. Enforcement is **always active in production**.

**Riskiest constraint (from the human):** *no part of the app may be served
without a valid session cookie, except the login page itself.* The middleware is
default-deny with an allowlist of exactly `/login` (GET + POST). `/static/*` is
**not** exempt — so the login page must be fully self-contained (inline CSS, no
`/static` refs, plain form POST, no htmx).

**Slice / context:**
- Stack: Axum 0.8, figment config, Tera renderer (`web::templates::Renderer`),
  rust-embed templates/i18n/static, cookie helpers already in `web::router`
  (`read_cookie`). No auth exists today (security AR-001).
- Tests build `web::router(state)` directly (4 `e2e_*` files + `it_*`). They must
  stay green **unchanged**, so enforcement must NOT live inside `web::router`.

**Design (approved):**

1. **Config is separate — `Config` struct is untouched.** A new `AuthConfig`
   (`web::auth`) is loaded by its own figment extract in `main.rs`, same sources
   as `Config::load` (Toml `filature.toml` + `Env::prefixed("FILATURE_").split("__")`),
   keyed on the `[auth]` table:
   ```toml
   [auth]
   username = "demo"
   password_hash = "$argon2id$v=19$..."
   ```
   `[auth]` absent → hard error at boot (⇒ always active). Zero churn to the 4
   test `Config` literals because auth is not a `Config` field.

2. **Enforcement = outer layer in `main.rs`, not in `web::router`.**
   `web::auth::protect(router, auth, renderer, default_locale) -> Router` wraps the
   app router. `main.rs` calls `web::router(state)` then `protect(...)`. Tests call
   `web::router` directly → no auth layer → green unchanged.

3. **`web::auth::protect` adds:**
   - `GET /login` → renders standalone `login.html` (self-contained, inline CSS
     using the design tokens, no `/static`/htmx). Optional `?error` shows a
     localized failure message.
   - `POST /login` (form: `username`, `password`) → constant-time username check +
     `argon2` `verify_password` against `password_hash`. Success → `Set-Cookie:
     session=<token>; HttpOnly; SameSite=Lax; Path=/` then 302 `/`. Failure →
     re-render login with error (401).
   - `GET /logout` → clears the cookie (`Max-Age=0`), 302 `/login`.
   - Middleware (`from_fn_with_state`): allowlist path == `/login` (any method) →
     pass; else `session` cookie constant-time-equals the boot token → pass; else
     302 `/login`.

4. **Session token:** 32 random bytes from `argon2::password_hash::rand_core::OsRng`,
   hex-encoded, generated **once at boot**, held in the auth layer state. One
   shared token (single demo user). Restart invalidates all sessions (re-login) —
   accepted, keeps zero secret material in config. Cookie compare is constant-time
   (`subtle`-style / byte-wise, no early return).

5. **Password hash generation:** `filature hash-password <password>` subcommand in
   `main.rs` (arg-sniff before the tokio server boots): prints the argon2 PHC
   string and exits. Keeps a single binary and makes the config value obtainable.

**Deps added:** `argon2` (verify + hash + `OsRng`). No `rand`/`hex` crate — hex is
a small local helper, randomness comes via `OsRng`.

**Acceptance criteria (done contract):**
- Unauthenticated `GET /`, `/spools`, `/materials`, `/static/app.css`, any path
  ≠ `/login` → **302 → `/login`** (no app HTML/CSS/data in the body).
- `GET /login` → 200, self-contained HTML, references no `/static` asset.
- `POST /login` good creds → 302 `/` + `Set-Cookie: session=…; HttpOnly; SameSite=Lax`.
- `POST /login` bad creds → 401, re-rendered login, **no** session cookie set.
- With a valid `session` cookie, all routes behave as before (existing e2e parity).
- `GET /logout` → clears cookie, 302 `/login`.
- Missing `[auth]` config → boot fails with a clear error.
- `filature hash-password foo` prints a `$argon2id$…` string that
  `verify_password("foo", …)` accepts.
- Existing `web::router` e2e/it suites pass unchanged (no auth layer in tests).

**Out of scope (YAGNI):** multiple users, roles, registration, password reset,
rate-limiting/lockout, CSRF token (single-user demo; SameSite=Lax is the guard),
persistent sessions across restart.

**Security note:** closes AR-001 for deployed demos (app no longer open by
default when `[auth]` is set — and it is required). Update
`docs/security/accepted-risks.md` accordingly.
