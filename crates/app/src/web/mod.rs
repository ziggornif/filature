//! The driving (Axum) adapter: routing, request handlers, Tera rendering,
//! htmx-facing HTML, locale/theme resolution. Split by responsibility:
//! `router` (routes + handlers), `state` (`AppState`), `templates` (Tera),
//! `i18n` (translation catalog), `theme` (light/dark cookie).

pub mod i18n;
pub mod router;
pub mod state;
pub mod templates;
pub mod theme;

pub use router::router;
pub use state::AppState;
