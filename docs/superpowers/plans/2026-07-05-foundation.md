# Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A running single-binary Filature app that boots (config → SQLite/WAL → migrations), embeds all assets, and serves the app-shell (sidebar layout) in light/dark themes and en/fr locales — the substrate every domain slice plugs into.

**Architecture:** Two-crate hexagonal workspace ([ADR-0002](../../adr/0002-crate-structure.md)): `crates/domain` (pure) + `crates/app` (Axum/SQLx/Tera + adapters + `main`). i18n is a `web/` concern only ([ADR-0001](../../adr/0001-language-and-i18n.md)); the domain never sees a locale. Templates, static assets, migrations and i18n catalogs are embedded in the binary.

**Tech Stack:** Rust, Axum 0.8, SQLx (SQLite, WAL), Tera, htmx, Tokio, rust-embed, figment (config), rust_decimal, thiserror.

---

## File structure

```
Cargo.toml                              # workspace
crates/domain/Cargo.toml
crates/domain/src/lib.rs                # re-exports shared
crates/domain/src/shared/mod.rs         # Grams, Money, DomainError (minimal kernel)
crates/app/Cargo.toml
crates/app/src/main.rs                  # composition root: config→db→migrate→router→serve
crates/app/src/config.rs                # Config (TOML + env)
crates/app/src/persistence/mod.rs       # SQLite pool (WAL) + migrate runner
crates/app/src/web/mod.rs               # router, AppState, index handler, static serving
crates/app/src/web/i18n.rs              # Catalog: embedded JSON → t(locale, key)
crates/app/src/web/theme.rs             # Theme resolution from cookie
crates/app/src/web/templates.rs         # Tera built from embedded strings + `t` function
crates/app/assets/templates/base.html   # app shell (sidebar), theme+locale aware
crates/app/assets/templates/index.html  # placeholder landing extending base
crates/app/assets/i18n/en.json          # translation catalog
crates/app/assets/i18n/fr.json
crates/app/assets/static/app.css        # tokens (light/dark) from the design handoff
crates/app/assets/static/htmx.min.js    # vendored htmx (no build step)
crates/app/migrations/0001_foundation.sql  # empty seam; slices add their own migrations
crates/app/tests/it_persistence.rs      # integration: pool opens + migrates in-memory
crates/app/tests/e2e_shell.rs           # e2e: GET / renders shell in en + fr
```

Tests co-located: domain unit tests inline in `shared/mod.rs`; app i18n/theme/template tests inline; integration in `crates/app/tests/it_*.rs`; e2e in `crates/app/tests/e2e_*.rs` (matches the CI test-file globs).

---

### Task 0: Workspace + crate skeletons

**Files:**
- Create: `Cargo.toml`, `crates/domain/Cargo.toml`, `crates/domain/src/lib.rs`, `crates/app/Cargo.toml`, `crates/app/src/main.rs`

- [ ] **Step 1: Workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/domain", "crates/app"]

[workspace.package]
edition = "2021"
license = "MIT"

[workspace.dependencies]
thiserror = "2"
rust_decimal = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Domain crate manifest** — only allowlisted deps (domain-purity sensor)

`crates/domain/Cargo.toml`:
```toml
[package]
name = "domain"
version = "0.0.0"
edition.workspace = true

[dependencies]
thiserror.workspace = true
rust_decimal.workspace = true
```
> NOTE: no serde in domain (keeps it framework-free; DTOs live in `web/`). If a domain type ever needs serialization, that's a smell — map in the adapter.

- [ ] **Step 3: App crate manifest**

`crates/app/Cargo.toml`:
```toml
[package]
name = "filature"
version = "0.1.0"
edition.workspace = true

[[bin]]
name = "filature"
path = "src/main.rs"

[dependencies]
domain = { path = "../domain" }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["trace"] }
tera = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }
rust-embed = "8"
figment = { version = "0.10", features = ["toml", "env"] }
serde.workspace = true
serde_json.workspace = true
rust_decimal.workspace = true
thiserror.workspace = true
mime_guess = "2"
```

- [ ] **Step 4: Stub lib + main**

`crates/domain/src/lib.rs`:
```rust
pub mod shared;
```
`crates/domain/src/shared/mod.rs`:
```rust
// minimal kernel — filled in Task 1
```
`crates/app/src/main.rs`:
```rust
fn main() {
    println!("filature");
}
```

- [ ] **Step 5: Verify it builds**

Run: `SQLX_OFFLINE=true cargo build --workspace`
Expected: compiles (warnings ok for now).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/
git commit -m "feat(foundation): cargo workspace + crate skeletons"
```

---

### Task 1: Shared kernel domain types (TDD)

**Files:**
- Modify: `crates/domain/src/shared/mod.rs`

- [ ] **Step 1: Write the failing tests**

`crates/domain/src/shared/mod.rs`:
```rust
use rust_decimal::Decimal;
use thiserror::Error;

/// A weight of filament in grams. Non-negative by construction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Grams(f64);

/// Monetary amount (prices). Decimal to avoid float drift.
pub type Money = Decimal;

#[derive(Debug, Error, PartialEq)]
pub enum DomainError {
    #[error("weight must be non-negative, got {0}")]
    NegativeWeight(f64),
}

impl Grams {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if value < 0.0 {
            return Err(DomainError::NegativeWeight(value));
        }
        Ok(Self(value))
    }
    pub fn value(self) -> f64 {
        self.0
    }
    /// Remaining as a fraction of an initial (net) weight, 0.0..=1.0+.
    pub fn ratio_of(self, net: Grams) -> f64 {
        if net.0 <= 0.0 {
            0.0
        } else {
            self.0 / net.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grams_rejects_negative() {
        assert_eq!(Grams::new(-1.0), Err(DomainError::NegativeWeight(-1.0)));
    }

    #[test]
    fn grams_ratio_of_net() {
        let remaining = Grams::new(250.0).unwrap();
        let net = Grams::new(1000.0).unwrap();
        assert!((remaining.ratio_of(net) - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn grams_ratio_guards_zero_net() {
        assert_eq!(Grams::new(5.0).unwrap().ratio_of(Grams::new(0.0).unwrap()), 0.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass** (implementation is inline above)

Run: `cargo test -p domain --lib`
Expected: 3 passed.

- [ ] **Step 3: Domain purity sensor stays green**

Run: `bash tools/check-domain-purity.sh`
Expected: `Domain purity: OK`

- [ ] **Step 4: Commit**

```bash
git add crates/domain/src/shared/mod.rs
git commit -m "feat(foundation): shared kernel — Grams, Money, DomainError"
```

---

### Task 2: Config loader (TDD)

**Files:**
- Create: `crates/app/src/config.rs`
- Modify: `crates/app/src/main.rs` (declare `mod config;`)

- [ ] **Step 1: Write the failing test**

`crates/app/src/config.rs`:
```rust
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub i18n: I18nConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct I18nConfig {
    pub default_locale: String,
}

impl Config {
    /// TOML file (if present) with FILATURE_ env overrides (double underscore = nesting,
    /// e.g. FILATURE_SERVER__BIND=0.0.0.0:9000).
    pub fn load(toml_path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(toml_path))
            .merge(Env::prefixed("FILATURE_").split("__"))
            .extract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_overrides_toml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "filature.toml",
                r#"
                [server]
                bind = "127.0.0.1:8080"
                [database]
                url = "sqlite://filature.db"
                [i18n]
                default_locale = "fr"
                "#,
            )?;
            jail.set_env("FILATURE_SERVER__BIND", "0.0.0.0:9000");
            let cfg = Config::load("filature.toml").unwrap();
            assert_eq!(cfg.server.bind, "0.0.0.0:9000"); // env wins
            assert_eq!(cfg.i18n.default_locale, "fr"); // toml value kept
            Ok(())
        });
    }
}
```

`crates/app/src/main.rs`:
```rust
mod config;

fn main() {
    println!("filature");
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `SQLX_OFFLINE=true cargo test -p filature --lib config::`
Expected: `env_overrides_toml ... ok`

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/config.rs crates/app/src/main.rs
git commit -m "feat(foundation): config loader (TOML + FILATURE_ env overrides)"
```

---

### Task 3: Persistence — SQLite pool (WAL) + migrations (integration test)

**Files:**
- Create: `crates/app/src/persistence/mod.rs`, `crates/app/migrations/0001_foundation.sql`, `crates/app/tests/it_persistence.rs`
- Modify: `crates/app/src/main.rs` (`mod persistence;`)

- [ ] **Step 1: Empty initial migration (seam for slices)**

`crates/app/migrations/0001_foundation.sql`:
```sql
-- Foundation migration. Domain tables are added by their slices
-- (materials, spools, locations) as later numbered migrations.
-- This file establishes the migrations table + ordering baseline.
PRAGMA foreign_keys = ON;
```

- [ ] **Step 2: Pool builder with WAL + embedded migrate**

`crates/app/src/persistence/mod.rs`:
```rust
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

pub type Db = Pool<Sqlite>;

/// Open the pool, enable WAL, and run embedded migrations.
/// `url` accepts "sqlite://file.db" (creates if missing) or "sqlite::memory:".
pub async fn connect_and_migrate(url: &str) -> Result<Db, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

- [ ] **Step 3: Write the failing integration test**

`crates/app/tests/it_persistence.rs`:
```rust
use filature::persistence::connect_and_migrate;

#[tokio::test]
async fn opens_and_migrates_in_memory() {
    let db = connect_and_migrate("sqlite::memory:").await.unwrap();
    // migrations ran => the sqlx bookkeeping table exists
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(count >= 1);
}
```
> For this to compile, `crates/app` must expose a lib target. Add to `crates/app/Cargo.toml`:
> ```toml
> [lib]
> name = "filature"
> path = "src/lib.rs"
> ```
> and create `crates/app/src/lib.rs` re-exporting modules (see Step 4). `main.rs` then uses the lib.

- [ ] **Step 4: Introduce the app lib target**

`crates/app/src/lib.rs`:
```rust
pub mod config;
pub mod persistence;
pub mod web;
```
`crates/app/src/main.rs` becomes:
```rust
use filature::{config::Config, persistence, web};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::load("filature.toml")?;
    let db = persistence::connect_and_migrate(&cfg.database.url).await?;
    let app = web::router(web::AppState::new(db, &cfg));
    let listener = tokio::net::TcpListener::bind(&cfg.server.bind).await?;
    println!("filature listening on {}", cfg.server.bind);
    axum::serve(listener, app).await?;
    Ok(())
}
```
> `web` module + `AppState` are built in Tasks 4–6; this main won't compile until then. That's expected in TDD ordering — Task 3's integration test only needs `persistence`, so run it with `--test it_persistence` before main is complete by temporarily stubbing `web` (Step 5 note), or land Tasks 4-6 before running `cargo build` on the bin.

- [ ] **Step 5: Run the integration test**

Run: `SQLX_OFFLINE=true cargo test -p filature --test it_persistence`
Expected: `opens_and_migrates_in_memory ... ok`
(If the bin fails to build due to unfinished `web`, add a temporary `pub mod web { ... }` stub or comment `mod` lines in `lib.rs` not yet created; remove stubs as Tasks 4-6 land.)

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/persistence crates/app/migrations crates/app/tests/it_persistence.rs crates/app/src/lib.rs crates/app/src/main.rs crates/app/Cargo.toml
git commit -m "feat(foundation): sqlite pool (WAL) + embedded migrations"
```

---

### Task 4: i18n catalog (TDD)

**Files:**
- Create: `crates/app/src/web/i18n.rs`, `crates/app/assets/i18n/en.json`, `crates/app/assets/i18n/fr.json`
- Modify: `crates/app/src/web/mod.rs` (created here)

- [ ] **Step 1: Catalogs**

`crates/app/assets/i18n/en.json`:
```json
{
  "app.name": "Filature",
  "nav.dashboard": "Dashboard",
  "nav.spools": "Spools",
  "nav.humidity": "Humidity",
  "nav.materials": "Materials",
  "shell.tagline": "Zig Factory · self-hosted"
}
```
`crates/app/assets/i18n/fr.json`:
```json
{
  "app.name": "Filature",
  "nav.dashboard": "Tableau de bord",
  "nav.spools": "Bobines",
  "nav.humidity": "Humidité",
  "nav.materials": "Matériaux",
  "shell.tagline": "Zig Factory · auto-hébergé"
}
```

- [ ] **Step 2: Write the failing test + Catalog impl**

`crates/app/src/web/i18n.rs`:
```rust
use rust_embed::RustEmbed;
use std::collections::HashMap;

#[derive(RustEmbed)]
#[folder = "assets/i18n"]
struct I18nAssets;

/// All locale catalogs loaded from embedded JSON at startup.
#[derive(Clone)]
pub struct Catalog {
    locales: HashMap<String, HashMap<String, String>>,
    default_locale: String,
}

impl Catalog {
    pub fn load(default_locale: &str) -> Self {
        let mut locales = HashMap::new();
        for file in I18nAssets::iter() {
            let name = file.as_ref();
            if let Some(code) = name.strip_suffix(".json") {
                let bytes = I18nAssets::get(name).unwrap().data;
                let map: HashMap<String, String> =
                    serde_json::from_slice(&bytes).expect("valid i18n json");
                locales.insert(code.to_string(), map);
            }
        }
        Self { locales, default_locale: default_locale.to_string() }
    }

    /// Look up a key in `locale`, falling back to the default locale, then the key itself.
    pub fn t(&self, locale: &str, key: &str) -> String {
        self.locales
            .get(locale)
            .and_then(|m| m.get(key))
            .or_else(|| self.locales.get(&self.default_locale).and_then(|m| m.get(key)))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn has_locale(&self, locale: &str) -> bool {
        self.locales.contains_key(locale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_known_key_per_locale() {
        let c = Catalog::load("en");
        assert_eq!(c.t("fr", "nav.spools"), "Bobines");
        assert_eq!(c.t("en", "nav.spools"), "Spools");
    }

    #[test]
    fn falls_back_to_default_then_key() {
        let c = Catalog::load("en");
        assert_eq!(c.t("de", "nav.spools"), "Spools"); // unknown locale -> default
        assert_eq!(c.t("en", "missing.key"), "missing.key"); // unknown key -> key
    }
}
```
`crates/app/src/web/mod.rs` (start it — expanded in Tasks 5-6):
```rust
pub mod i18n;
pub mod theme;
pub mod templates;
```

- [ ] **Step 3: Run the test**

Run: `SQLX_OFFLINE=true cargo test -p filature --lib web::i18n::`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/web/i18n.rs crates/app/assets/i18n crates/app/src/web/mod.rs
git commit -m "feat(foundation): embedded i18n catalog (en, fr) with fallback"
```

---

### Task 5: Theme + Tera templates + render test (TDD)

**Files:**
- Create: `crates/app/src/web/theme.rs`, `crates/app/src/web/templates.rs`, `crates/app/assets/templates/base.html`, `crates/app/assets/templates/index.html`

- [ ] **Step 1: Theme resolution**

`crates/app/src/web/theme.rs`:
```rust
/// Theme chosen by the user. `Auto` follows the OS via CSS `prefers-color-scheme`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Auto,
    Light,
    Dark,
}

impl Theme {
    /// Parse the `theme` cookie value; anything unknown => Auto.
    pub fn from_cookie(value: Option<&str>) -> Self {
        match value {
            Some("light") => Theme::Light,
            Some("dark") => Theme::Dark,
            _ => Theme::Auto,
        }
    }
    /// The `data-theme` attribute value, or empty for Auto (CSS handles OS default).
    pub fn data_attr(self) -> &'static str {
        match self {
            Theme::Auto => "",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_cookie() {
        assert_eq!(Theme::from_cookie(Some("dark")), Theme::Dark);
        assert_eq!(Theme::from_cookie(Some("zzz")), Theme::Auto);
        assert_eq!(Theme::from_cookie(None), Theme::Auto);
    }
}
```

- [ ] **Step 2: base + index templates** (app shell — sidebar per design handoff §Navigation)

`crates/app/assets/templates/base.html`:
```html
<!doctype html>
<html lang="{{ locale }}"{% if theme_attr %} data-theme="{{ theme_attr }}"{% endif %}>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{{ t(key="app.name") }}</title>
  <link rel="stylesheet" href="/static/app.css">
  <script src="/static/htmx.min.js" defer></script>
</head>
<body>
  <aside class="sidebar">
    <div class="wordmark">{{ t(key="app.name") }}</div>
    <nav>
      <a href="/">{{ t(key="nav.dashboard") }}</a>
      <a href="/spools">{{ t(key="nav.spools") }}</a>
      <a href="/materials">{{ t(key="nav.materials") }}</a>
    </nav>
    <div class="sidebar-foot">{{ t(key="shell.tagline") }}</div>
  </aside>
  <main class="content">
    {% block content %}{% endblock content %}
  </main>
</body>
</html>
```
`crates/app/assets/templates/index.html`:
```html
{% extends "base.html" %}
{% block content %}
  <h1>{{ t(key="nav.dashboard") }}</h1>
{% endblock content %}
```

- [ ] **Step 3: Tera engine from embedded templates + `t` function + render test**

`crates/app/src/web/templates.rs`:
```rust
use crate::web::i18n::Catalog;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Context, Tera, Value};

#[derive(RustEmbed)]
#[folder = "assets/templates"]
struct TemplateAssets;

#[derive(Clone)]
pub struct Renderer {
    tera: Arc<Tera>,
    catalog: Catalog,
}

impl Renderer {
    pub fn new(catalog: Catalog) -> Self {
        let mut tera = Tera::default();
        let mut raw: Vec<(String, String)> = Vec::new();
        for name in TemplateAssets::iter() {
            let bytes = TemplateAssets::get(name.as_ref()).unwrap().data;
            raw.push((name.to_string(), String::from_utf8(bytes.to_vec()).unwrap()));
        }
        tera.add_raw_templates(raw.iter().map(|(n, s)| (n.as_str(), s.as_str())))
            .expect("templates compile");
        Self { tera: Arc::new(tera), catalog }
    }

    /// Render `template` with a per-request locale + theme; registers a locale-bound `t`.
    pub fn render(
        &self,
        template: &str,
        locale: &str,
        theme_attr: &str,
        mut ctx: Context,
    ) -> Result<String, tera::Error> {
        ctx.insert("locale", locale);
        ctx.insert("theme_attr", theme_attr);
        // Bind a `t(key=...)` Tera function to this locale.
        let catalog = self.catalog.clone();
        let locale_owned = locale.to_string();
        let mut tera = (*self.tera).clone();
        tera.register_function(
            "t",
            move |args: &HashMap<String, Value>| -> tera::Result<Value> {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| tera::Error::msg("t() needs a `key`"))?;
                Ok(Value::String(catalog.t(&locale_owned, key)))
            },
        );
        tera.render(template, &ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderer() -> Renderer {
        Renderer::new(Catalog::load("en"))
    }

    #[test]
    fn renders_shell_in_french() {
        let html = renderer()
            .render("index.html", "fr", "dark", Context::new())
            .unwrap();
        assert!(html.contains("Tableau de bord")); // fr nav label
        assert!(html.contains(r#"lang="fr""#));
        assert!(html.contains(r#"data-theme="dark""#));
    }

    #[test]
    fn renders_shell_in_english_auto_theme() {
        let html = renderer()
            .render("index.html", "en", "", Context::new())
            .unwrap();
        assert!(html.contains("Dashboard"));
        assert!(!html.contains("data-theme")); // Auto => no attribute
    }

    #[test]
    fn missing_i18n_key_would_surface() {
        // A template referencing an undefined key renders the key verbatim,
        // so render tests catch typos at `cargo test`, not in prod (brief §6).
        let html = renderer().render("index.html", "en", "", Context::new()).unwrap();
        assert!(!html.contains("nav.dashboard")); // the raw key must NOT leak
    }
}
```
> NOTE: cloning `Tera` per render to bind a locale-specific `t` is simple and correct for this scale. The clone cost is O(all templates registered in the engine) — it grows with the whole template set, not the single page rendered. Revisit — switch to passing locale through the Tera context and a function that reads it — when htmx fragments multiply the template count, not merely if profiling flags it (deferred, YAGNI).

- [ ] **Step 4: Run the tests**

Run: `SQLX_OFFLINE=true cargo test -p filature --lib web::theme:: web::templates::`
Expected: theme (1) + templates (3) passed.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/web/theme.rs crates/app/src/web/templates.rs crates/app/assets/templates
git commit -m "feat(foundation): tera renderer, theme, app-shell templates (en/fr render tests)"
```

---

### Task 6: Web router + AppState + static assets + e2e (TDD)

**Files:**
- Modify: `crates/app/src/web/mod.rs`
- Create: `crates/app/assets/static/app.css`, `crates/app/assets/static/htmx.min.js`, `crates/app/tests/e2e_shell.rs`

- [ ] **Step 1: Vendored static assets**

Fetch htmx (no build step) into `crates/app/assets/static/htmx.min.js`:
```bash
curl -sL https://unpkg.com/htmx.org@2/dist/htmx.min.js -o crates/app/assets/static/htmx.min.js
```
`crates/app/assets/static/app.css` — paste the light+dark token blocks from
`init_assets/design_handoff_filature/README.md` §Design Tokens into `:root` and
`html[data-theme="light|dark"]`, plus minimal sidebar/content layout. (Full
styling is refined in the UI slices; foundation needs the tokens + shell layout.)
```css
:root { /* dark defaults via prefers-color-scheme handled below */ }
/* paste --bg/--surface/--text/... light values under html[data-theme="light"]
   and dark values under html[data-theme="dark"] + a @media(prefers-color-scheme)
   default. See handoff README §Couleurs. */
.sidebar { width: 216px; }
.content { padding: 28px; }
```

- [ ] **Step 2: Router + AppState + handlers + static serving + e2e test**

`crates/app/src/web/mod.rs`:
```rust
pub mod i18n;
pub mod templates;
pub mod theme;

use crate::config::Config;
use crate::persistence::Db;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use i18n::Catalog;
use rust_embed::RustEmbed;
use templates::Renderer;
use tera::Context;
use theme::Theme;

#[derive(RustEmbed)]
#[folder = "assets/static"]
struct StaticAssets;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub renderer: Renderer,
    pub default_locale: String,
}

impl AppState {
    pub fn new(db: Db, cfg: &Config) -> Self {
        let catalog = Catalog::load(&cfg.i18n.default_locale);
        Self {
            db,
            renderer: Renderer::new(catalog),
            default_locale: cfg.i18n.default_locale.clone(),
        }
    }
}

/// Resolve locale from the `lang` cookie, else the configured default.
fn resolve_locale(headers: &HeaderMap, default: &str) -> String {
    read_cookie(headers, "lang").unwrap_or_else(|| default.to_string())
}
fn resolve_theme(headers: &HeaderMap) -> Theme {
    Theme::from_cookie(read_cookie(headers, "theme").as_deref())
}
fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

async fn index(State(st): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let locale = resolve_locale(&headers, &st.default_locale);
    let theme = resolve_theme(&headers);
    match st.renderer.render("index.html", &locale, theme.data_attr(), Context::new()) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn static_handler(Path(path): Path<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/static/{*path}", get(static_handler))
        .with_state(state)
}
```

`crates/app/tests/e2e_shell.rs`:
```rust
use axum::body::to_bytes;
use axum::http::{header, Request, StatusCode};
use filature::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
use filature::{persistence, web};
use tower::ServiceExt; // oneshot

fn test_config() -> Config {
    Config {
        server: ServerConfig { bind: "127.0.0.1:0".into() },
        database: DatabaseConfig { url: "sqlite::memory:".into() },
        i18n: I18nConfig { default_locale: "en".into() },
    }
}

async fn app() -> axum::Router {
    let db = persistence::connect_and_migrate("sqlite::memory:").await.unwrap();
    web::router(web::AppState::new(db, &test_config()))
}

#[tokio::test]
async fn index_renders_default_locale() {
    let res = app().await
        .oneshot(Request::builder().uri("/").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Dashboard")); // en default
}

#[tokio::test]
async fn index_honours_lang_cookie() {
    let res = app().await
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::COOKIE, "lang=fr")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Tableau de bord")); // fr via cookie
}
```
> Add `tower = "0.5"` to `crates/app` `[dev-dependencies]` for `ServiceExt::oneshot`.
> The `Config` struct fields must be `pub` (they are) so tests can build one directly.

- [ ] **Step 3: Run e2e**

Run: `SQLX_OFFLINE=true cargo test -p filature --test e2e_shell`
Expected: 2 passed.

- [ ] **Step 4: Full build + all sensors**

Run:
```bash
SQLX_OFFLINE=true cargo build --workspace
SQLX_OFFLINE=true cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
bash tools/check-domain-purity.sh
bash tools/check-slice-isolation.sh
cargo fmt --all -- --check
```
Expected: all green; purity OK; slice-isolation OK (no slices yet).

- [ ] **Step 5: Manual smoke (verify skill)**

```bash
printf '[server]\nbind="127.0.0.1:8080"\n[database]\nurl="sqlite://filature.db"\n[i18n]\ndefault_locale="en"\n' > filature.toml
SQLX_OFFLINE=true cargo run
# visit http://127.0.0.1:8080 — sidebar shell renders; toggle OS dark/light;
# set a `lang=fr` cookie in devtools -> labels switch to French.
```

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/web crates/app/assets/static crates/app/tests/e2e_shell.rs crates/app/Cargo.toml
git commit -m "feat(foundation): axum router, app-shell, static assets, e2e (theme + locale)"
```

---

## Self-review

- **Spec coverage:** workspace+2 crates (ADR-0002) ✓; domain purity allowlist ✓; config TOML+env ✓; SQLite WAL + embedded migrations ✓; embedded assets (templates, static, i18n, migrations) ✓; Tera ✓; i18n en+fr extensible + fallback (ADR-0001) ✓; theme light/dark/auto server-rendered ✓; app shell sidebar (handoff §Navigation) ✓; render tests incl. non-default locale (brief §6) ✓; e2e critical path ✓. Deferred humidity nav item intentionally omitted from the shell.
- **Placeholders:** app.css tokens are the one "paste from handoff" step — acceptable (verbatim copy of a documented token table, not invented logic). Everything else is complete code.
- **Type consistency:** `Config`/`ServerConfig`/`DatabaseConfig`/`I18nConfig`, `AppState::new(db, &cfg)`, `Renderer::new(catalog)`, `Catalog::load(default)`, `connect_and_migrate(url)`, `Theme::data_attr()` used consistently across tasks and the e2e test.

## Verify (SQLX_OFFLINE note)

`sqlx::migrate!` does not need a live DB or `.sqlx/` metadata (it reads the migrations dir at compile time). `SQLX_OFFLINE=true` matters only once slices introduce `query!` macros; set it now so the habit + CI env match. No `.sqlx/` dir until the first `query!` lands (materials slice).
