//! The driving (Axum) adapter for the spools slice: a read-only, filterable
//! and sortable list (`GET /spools`) whose filter/sort bar issues htmx GETs
//! to `GET /spools/rows`, swapping only the `<tbody>` — no full reload.
//! Mirrors `web::materials` for locale/theme resolution and Tera rendering.
//!
//! The material dropdown is populated by calling `AppState::materials`
//! (the materials use cases already wired into shared state) — this is
//! web-layer composition across two driving-adapter handlers, not a domain
//! import across slices (the spools domain never depends on materials).

use crate::web::router::{resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Router,
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use domain::shared::{DomainError, Grams, MaterialId, Money};
use domain::spools::{
    Colour, Diameter, NewSpool, RepositoryError, Spool, SpoolDetail, SpoolFilter, SpoolId,
    SpoolListItem, SpoolSort, SpoolStatus,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tera::Context;

/// Template-shaped view of a `SpoolListItem`: plain strings/numbers plus the
/// derived percentage/length fields (the domain never exposes a "view
/// model").
#[derive(Serialize)]
pub struct SpoolView {
    pub id: String,
    pub material_name: String,
    pub colour_hex: String,
    /// The colour's human name if set, else the hex code — always
    /// something displayable (e.g. as a swatch `title`).
    pub colour_label: String,
    pub diameter: String, // "1.75" | "2.85"
    pub remaining_weight: f64,
    pub net_weight: f64,
    pub remaining_pct: u8,
    pub remaining_length_m: f64,
    pub status: String, // "Sealed" | "Open" | "Empty" | "Archived"
}

impl From<SpoolListItem> for SpoolView {
    fn from(item: SpoolListItem) -> Self {
        let remaining_pct = (item.remaining_ratio() * 100.0).round();
        let remaining_length_m = round1(item.remaining_length_m());
        Self {
            id: item.id.as_str().to_string(),
            material_name: item.material_name.clone(),
            colour_hex: item.colour.hex().to_string(),
            colour_label: item
                .colour
                .name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| item.colour.hex().to_string()),
            diameter: item.diameter.as_str().to_string(),
            remaining_weight: round1(item.remaining_weight.value()),
            net_weight: round1(item.net_weight.value()),
            remaining_pct: remaining_pct as u8, // saturating cast — no panic on out-of-range ratios
            remaining_length_m,
            status: item.status.as_str().to_string(),
        }
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Template-shaped view of a `SpoolDetail`: every stored field as plain
/// strings/numbers, plus the same derived percentage/length fields as
/// `SpoolView` (same rounding/percent conventions — the detail page and the
/// list must never disagree on what "80%" or "268.2 m" means for the same
/// spool).
#[derive(Serialize)]
pub struct SpoolDetailView {
    pub id: String,
    pub material_name: String,
    pub colour_hex: String,
    /// The colour's human name if set, else the hex code — always
    /// something displayable (e.g. as a swatch `title`).
    pub colour_label: String,
    /// Whether a colour name was actually set — lets the template show the
    /// name next to the swatch only when there's a name distinct from the
    /// hex code.
    pub has_colour_name: bool,
    pub diameter: String, // "1.75" | "2.85"
    pub net_weight: f64,
    pub remaining_weight: f64,
    pub remaining_pct: u8,
    pub remaining_length_m: f64,
    pub price_paid: String,
    pub status: String, // "Sealed" | "Open" | "Empty" | "Archived"
}

impl From<SpoolDetail> for SpoolDetailView {
    fn from(d: SpoolDetail) -> Self {
        let remaining_pct = (d.remaining_ratio() * 100.0).round();
        let remaining_length_m = round1(d.remaining_length_m());
        Self {
            id: d.id.as_str().to_string(),
            material_name: d.material_name.clone(),
            colour_hex: d.colour.hex().to_string(),
            colour_label: d
                .colour
                .name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| d.colour.hex().to_string()),
            has_colour_name: d.colour.name().is_some(),
            diameter: d.diameter.as_str().to_string(),
            net_weight: round1(d.net_weight.value()),
            remaining_weight: round1(d.remaining_weight.value()),
            remaining_pct: remaining_pct as u8, // saturating cast — no panic on out-of-range ratios
            remaining_length_m,
            price_paid: d.price_paid.to_string(),
            status: d.status.as_str().to_string(),
        }
    }
}

/// A material option for the filter dropdown — the web layer's own
/// composition of `AppState::materials`, not a domain read model.
#[derive(Serialize)]
pub struct MaterialOption {
    pub id: String,
    pub name: String,
}

/// Query params accepted by both `GET /spools` and `GET /spools/rows`.
/// `#[serde(default)]` so any/all of them may be absent from the query
/// string (a fresh page load supplies none).
#[derive(Debug, Deserialize, Default)]
pub struct SpoolQuery {
    #[serde(default)]
    pub material_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
}

impl SpoolQuery {
    fn to_filter(&self) -> SpoolFilter {
        SpoolFilter {
            material_id: self
                .material_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| MaterialId::new(s.to_string())),
            status: self
                .status
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|s| SpoolStatus::parse(s).ok()),
        }
    }

    fn to_sort(&self) -> SpoolSort {
        match self.sort.as_deref() {
            Some("remaining_asc") => SpoolSort::RemainingRatioAsc,
            Some("remaining_desc") => SpoolSort::RemainingRatioDesc,
            _ => SpoolSort::CreatedDesc,
        }
    }

    fn selected_sort(&self) -> &str {
        match self.sort.as_deref() {
            Some("remaining_asc") => "remaining_asc",
            Some("remaining_desc") => "remaining_desc",
            _ => "created_desc",
        }
    }
}

async fn material_options(st: &AppState) -> Result<Vec<MaterialOption>, Response> {
    st.materials
        .list()
        .await
        .map(|ms| {
            ms.into_iter()
                .map(|m| MaterialOption {
                    id: m.id.as_str().to_string(),
                    name: m.name.as_str().to_string(),
                })
                .collect()
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
}

fn render_rows(
    st: &AppState,
    locale: &str,
    items: Vec<SpoolListItem>,
    stock_value: &str,
) -> Response {
    let views: Vec<SpoolView> = items.into_iter().map(Into::into).collect();
    let mut ctx = Context::new();
    ctx.insert("spools", &views);
    ctx.insert("stock_value", stock_value);
    match st.renderer.render("_spool_rows.html", locale, "", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn list_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpoolQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);

    let materials = match material_options(&st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };

    let filter = q.to_filter();
    let stock_value = match st.spools.stock_value(filter.clone()).await {
        Ok(v) => v.to_string(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match st.spools.list(filter, q.to_sort()).await {
        Ok(items) => {
            let views: Vec<SpoolView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("spools", &views);
            ctx.insert("materials", &materials);
            ctx.insert("selected_material", q.material_id.as_deref().unwrap_or(""));
            ctx.insert("selected_status", q.status.as_deref().unwrap_or(""));
            ctx.insert("selected_sort", q.selected_sort());
            ctx.insert("stock_value", &stock_value);
            match st
                .renderer
                .render("spools.html", &locale, theme.data_attr(), ctx)
            {
                Ok(html) => Html(html).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn rows(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpoolQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let filter = q.to_filter();
    let stock_value = match st.spools.stock_value(filter.clone()).await {
        Ok(v) => v.to_string(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    match st.spools.list(filter, q.to_sort()).await {
        Ok(items) => render_rows(&st, &locale, items, &stock_value),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// The `POST /spools` form payload. Every field is a raw string (rather
/// than a typed `f64`/`Decimal`) so malformed input — a bad hex, a
/// non-numeric weight, an unparsable price — reaches this handler's own
/// validation and can be echoed back on the re-rendered form, instead of
/// being rejected by Axum's `Form` extractor (a plain 400) before the
/// handler even runs.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct SpoolForm {
    #[serde(default)]
    pub material_id: String,
    #[serde(default)]
    pub colour_hex: String,
    #[serde(default)]
    pub colour_name: String,
    #[serde(default)]
    pub diameter: String,
    #[serde(default)]
    pub net_weight: String,
    #[serde(default)]
    pub price_paid: String,
}

/// The fields shared by `NewSpool` (create) and an edited `Spool` (update) —
/// everything the form itself supplies, validated once and reused by both
/// `to_new` and `to_edit` so the two call sites can't drift.
struct SpoolFormFields {
    material_id: MaterialId,
    colour: Colour,
    diameter: Diameter,
    net_weight: Grams,
    price_paid: Money,
}

impl SpoolForm {
    /// Validates the raw form, rejecting invalid
    /// hex/diameter/weight/price/material with an i18n key (the caller
    /// turns this into a 422 + re-rendered form) rather than panicking or
    /// 500-ing on user input.
    fn parse(&self) -> Result<SpoolFormFields, &'static str> {
        if self.material_id.trim().is_empty() {
            return Err("spools.new.error.material");
        }
        let name = self.colour_name.trim();
        let colour_name = if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        };
        let colour = Colour::new(self.colour_hex.trim().to_string(), colour_name)
            .map_err(|_| "spools.new.error.colour")?;
        let diameter =
            Diameter::parse(self.diameter.trim()).map_err(|_| "spools.new.error.diameter")?;
        let net_weight: f64 = self
            .net_weight
            .trim()
            .parse()
            .map_err(|_| "spools.new.error.weight")?;
        let net_weight = Grams::new(net_weight).map_err(|_| "spools.new.error.weight")?;
        if net_weight.value() <= 0.0 {
            return Err("spools.new.error.weight");
        }
        let price_paid_dec = Decimal::from_str_exact(self.price_paid.trim())
            .map_err(|_| "spools.new.error.price")?;
        let price_paid =
            Money::from_decimal(price_paid_dec).map_err(|_| "spools.new.error.price")?;
        Ok(SpoolFormFields {
            material_id: MaterialId::new(self.material_id.trim().to_string()),
            colour,
            diameter,
            net_weight,
            price_paid,
        })
    }

    /// Maps the raw form into a domain `NewSpool` (create path).
    fn to_new(&self) -> Result<NewSpool, &'static str> {
        let f = self.parse()?;
        Ok(NewSpool {
            material_id: f.material_id,
            colour: f.colour,
            diameter: f.diameter,
            net_weight: f.net_weight,
            price_paid: f.price_paid,
        })
    }

    /// Maps the raw form into a domain `Spool` (edit path), carrying over
    /// `remaining_weight` and `status` from the spool's current state —
    /// this form never edits those, so the caller supplies them from a
    /// prior `SpoolsUseCases::view` rather than trusting the form for them.
    fn to_edit(
        &self,
        id: SpoolId,
        remaining_weight: Grams,
        status: SpoolStatus,
    ) -> Result<Spool, &'static str> {
        let f = self.parse()?;
        Ok(Spool {
            id,
            material_id: f.material_id,
            colour: f.colour,
            diameter: f.diameter,
            net_weight: f.net_weight,
            remaining_weight,
            price_paid: f.price_paid,
            status,
        })
    }
}

/// Renders the add-spool form (fresh on `GET /spools/new`, or re-populated
/// with the submitted values + a localized error on a failed `POST
/// /spools`). `error_key` is an i18n key looked up by the template's own
/// `t(key=error_key)` call, not pre-translated here, so locale switching
/// works the same way it does for every other string on the page.
async fn render_form(
    st: &AppState,
    locale: &str,
    theme_attr: &str,
    status: StatusCode,
    form: &SpoolForm,
    error_key: Option<&str>,
) -> Response {
    let materials = match material_options(st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let mut ctx = Context::new();
    ctx.insert("materials", &materials);
    ctx.insert("material_id", &form.material_id);
    ctx.insert("colour_hex", &form.colour_hex);
    ctx.insert("colour_name", &form.colour_name);
    ctx.insert("diameter", &form.diameter);
    ctx.insert("net_weight", &form.net_weight);
    ctx.insert("price_paid", &form.price_paid);
    ctx.insert("error_key", &error_key);
    match st
        .renderer
        .render("spools_new.html", locale, theme_attr, ctx)
    {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn new_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let form = SpoolForm {
        diameter: "1.75".to_string(),
        ..Default::default()
    };
    render_form(&st, &locale, theme.data_attr(), StatusCode::OK, &form, None).await
}

async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SpoolForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);

    let new = match form.to_new() {
        Ok(n) => n,
        Err(key) => {
            return render_form(
                &st,
                &locale,
                theme.data_attr(),
                StatusCode::UNPROCESSABLE_ENTITY,
                &form,
                Some(key),
            )
            .await;
        }
    };

    match st.spools.add(new).await {
        Ok(_) => Redirect::to("/spools").into_response(),
        Err(RepositoryError::UnknownMaterial(_)) => {
            render_form(
                &st,
                &locale,
                theme.data_attr(),
                StatusCode::UNPROCESSABLE_ENTITY,
                &form,
                Some("spools.new.error.material"),
            )
            .await
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Localized 404 body for an unknown spool id — there's no page shell to
/// render into here (unlike the form templates), so this is a bare
/// translated string rather than a full Tera render.
fn not_found(st: &AppState, locale: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(st.renderer.t(locale, "spools.edit.not_found")),
    )
        .into_response()
}

/// `GET /spools/{id}` — the read-only detail page for one spool: every
/// stored field plus the derived Remaining Ratio/Length, built from
/// `SpoolsUseCases::view`. 404s (localized) on an unknown id, same pattern
/// as `edit_page`.
async fn detail_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);

    let detail = match st.spools.view(SpoolId::new(id)).await {
        Ok(d) => d,
        Err(RepositoryError::NotFound(_)) => return not_found(&st, &locale),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let view: SpoolDetailView = detail.into();
    let mut ctx = Context::new();
    ctx.insert("spool", &view);
    // The page includes `_spool_detail_card.html`, which always references
    // `error_key` (to show/hide its op-error line) — insert `None` here so
    // that include doesn't hit an undefined variable on a fresh page load.
    ctx.insert("error_key", &Option::<&str>::None);
    match st
        .renderer
        .render("spools_detail.html", &locale, theme.data_attr(), ctx)
    {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Renders the detail-card fragment (`_spool_detail_card.html`) — the
/// weight/status `<dl>` plus the op forms (set-remaining/consume/archive or
/// restore) — used both standalone (an op handler's htmx `outerHTML` swap
/// target) and `{% include %}`d inside the full detail page. `error_key`,
/// when set, is shown as a localized op-error line inside the card.
async fn render_card(
    st: &AppState,
    locale: &str,
    status: StatusCode,
    detail: SpoolDetail,
    error_key: Option<&str>,
) -> Response {
    let view: SpoolDetailView = detail.into();
    let mut ctx = Context::new();
    ctx.insert("spool", &view);
    ctx.insert("error_key", &error_key);
    match st
        .renderer
        .render("_spool_detail_card.html", locale, "", ctx)
    {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Re-fetches `id`'s detail and renders the card fragment — the shared tail
/// of every op handler's success and domain-error paths (both need a fresh
/// `SpoolDetail`, since the mutated `Spool` returned by the use case lacks
/// the display-only `material_name`/`density` joined by `view`).
async fn render_card_for(
    st: &AppState,
    locale: &str,
    id: SpoolId,
    status: StatusCode,
    error_key: Option<&str>,
) -> Response {
    match st.spools.view(id).await {
        Ok(detail) => render_card(st, locale, status, detail, error_key).await,
        Err(RepositoryError::NotFound(_)) => not_found(st, locale),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Maps a lifecycle `DomainError` from a weight/archive op to the i18n key
/// shown in the re-rendered card fragment. The archive-lifecycle variants
/// (`SpoolArchived` — a weight op on an archived spool, `SpoolAlreadyArchived`,
/// `SpoolNotArchived`) all share one "this spool is archived" message; any
/// other `DomainError` (defensive catch-all — the four ops here only ever
/// produce the variants above) falls back to the generic weight error.
fn op_error_key(e: &DomainError) -> &'static str {
    match e {
        DomainError::RemainingAboveNet => "spools.op.error.remaining_above_net",
        DomainError::SpoolArchived
        | DomainError::SpoolAlreadyArchived
        | DomainError::SpoolNotArchived => "spools.op.error.archived",
        _ => "spools.op.error.weight",
    }
}

/// Maps an op's `Result<Spool, RepositoryError>` to a response: success and
/// domain-error both re-render the card fragment (200 / 422 respectively),
/// `NotFound` 404s, anything else 500s (TD-005's existing `e.to_string()`
/// pattern, unchanged).
async fn finish_op(
    st: &AppState,
    locale: &str,
    id: SpoolId,
    result: Result<Spool, RepositoryError>,
) -> Response {
    match result {
        Ok(_) => render_card_for(st, locale, id, StatusCode::OK, None).await,
        Err(RepositoryError::NotFound(_)) => not_found(st, locale),
        Err(RepositoryError::Domain(e)) => {
            let key = op_error_key(&e);
            render_card_for(st, locale, id, StatusCode::UNPROCESSABLE_ENTITY, Some(key)).await
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Parses a raw form weight field (`remaining`/`amount`) into a valid
/// `Grams`, rejecting non-numeric input the same way `Grams::new` rejects
/// negative values — both collapse to the caller's generic
/// `spools.op.error.weight` message. Non-finite floats (`NaN`/`Infinity`)
/// are rejected by `Grams::new` itself, at the domain root.
fn parse_grams(raw: &str) -> Option<Grams> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .and_then(|v| Grams::new(v).ok())
}

/// The shared "couldn't even parse the weight field" response for
/// `set_remaining`/`consume` — re-renders the card fragment (422) with the
/// generic weight-error key, same shape as a domain-rejected weight.
async fn invalid_weight(st: &AppState, locale: &str, id: SpoolId) -> Response {
    render_card_for(
        st,
        locale,
        id,
        StatusCode::UNPROCESSABLE_ENTITY,
        Some("spools.op.error.weight"),
    )
    .await
}

/// The `POST /spools/{id}/remaining` form payload — a raw string (see
/// `SpoolForm`'s doc comment for why: malformed input reaches this
/// handler's own validation rather than a plain 400 from `Form`).
#[derive(Debug, Deserialize, Default)]
pub struct RemainingForm {
    #[serde(default)]
    pub remaining: String,
}

/// The `POST /spools/{id}/consume` form payload.
#[derive(Debug, Deserialize, Default)]
pub struct ConsumeForm {
    #[serde(default)]
    pub amount: String,
}

/// `POST /spools/{id}/remaining` — sets the spool's remaining weight
/// directly (e.g. after a physical re-weigh). Rejects a remaining weight
/// above net weight and archived spools (both via `Spool::set_remaining`).
async fn set_remaining(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RemainingForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let spool_id = SpoolId::new(id);
    let Some(grams) = parse_grams(&form.remaining) else {
        return invalid_weight(&st, &locale, spool_id).await;
    };
    let result = st.spools.set_remaining(spool_id.clone(), grams).await;
    finish_op(&st, &locale, spool_id, result).await
}

/// `POST /spools/{id}/consume` — draws down the spool's remaining weight by
/// `amount` grams, flooring at zero (via `Spool::consume`).
async fn consume(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<ConsumeForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let spool_id = SpoolId::new(id);
    let Some(grams) = parse_grams(&form.amount) else {
        return invalid_weight(&st, &locale, spool_id).await;
    };
    let result = st.spools.consume(spool_id.clone(), grams).await;
    finish_op(&st, &locale, spool_id, result).await
}

/// `POST /spools/{id}/archive` — no body. Rejects an already-archived
/// spool.
async fn archive(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let spool_id = SpoolId::new(id);
    let result = st.spools.archive(spool_id.clone()).await;
    finish_op(&st, &locale, spool_id, result).await
}

/// `POST /spools/{id}/restore` — no body. Rejects a spool that isn't
/// archived.
async fn restore(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let spool_id = SpoolId::new(id);
    let result = st.spools.restore(spool_id.clone()).await;
    finish_op(&st, &locale, spool_id, result).await
}

/// The parts of `render_edit_form`'s response that vary by call site,
/// bundled into one struct so the function itself stays under clippy's
/// too-many-arguments threshold.
struct EditFormRender<'a> {
    status: StatusCode,
    id: &'a str,
    error_key: Option<&'static str>,
    /// The full page (extends `base.html`) on a fresh `GET
    /// /spools/{id}/edit`, or just the `<form>` fragment (matching its own
    /// `hx-target="this"` / `hx-swap="outerHTML"`) when re-rendering a
    /// failed `PUT /spools/{id}` in place.
    full_page: bool,
}

async fn render_edit_form(
    st: &AppState,
    locale: &str,
    theme_attr: &str,
    form: &SpoolForm,
    opts: EditFormRender<'_>,
) -> Response {
    let materials = match material_options(st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let mut ctx = Context::new();
    ctx.insert("id", opts.id);
    ctx.insert("materials", &materials);
    ctx.insert("material_id", &form.material_id);
    ctx.insert("colour_hex", &form.colour_hex);
    ctx.insert("colour_name", &form.colour_name);
    ctx.insert("diameter", &form.diameter);
    ctx.insert("net_weight", &form.net_weight);
    ctx.insert("price_paid", &form.price_paid);
    ctx.insert("error_key", &opts.error_key);
    let template = if opts.full_page {
        "spools_edit.html"
    } else {
        "_spool_edit_form.html"
    };
    match st.renderer.render(template, locale, theme_attr, ctx) {
        Ok(html) => (opts.status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn edit_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);

    let detail = match st.spools.view(SpoolId::new(id.clone())).await {
        Ok(d) => d,
        Err(RepositoryError::NotFound(_)) => return not_found(&st, &locale),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let form = SpoolForm {
        material_id: detail.material_id.as_str().to_string(),
        colour_hex: detail.colour.hex().to_string(),
        colour_name: detail.colour.name().unwrap_or("").to_string(),
        diameter: detail.diameter.as_str().to_string(),
        net_weight: detail.net_weight.value().to_string(),
        price_paid: detail.price_paid.to_string(),
    };
    render_edit_form(
        &st,
        &locale,
        theme.data_attr(),
        &form,
        EditFormRender {
            status: StatusCode::OK,
            id: &id,
            error_key: None,
            full_page: true,
        },
    )
    .await
}

/// `PUT /spools/{id}`, issued by the edit form's own `hx-put` (a genuine
/// HTTP PUT via htmx's JS, same mechanism the materials table's inline
/// edits use — no method-override hack). Loads the spool's current
/// `remaining_weight`/`status` so they're preserved (this form never edits
/// them), then delegates the net-weight clamp to `SpoolsUseCases::edit`.
async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<SpoolForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let spool_id = SpoolId::new(id.clone());

    let current = match st.spools.view(spool_id.clone()).await {
        Ok(d) => d,
        Err(RepositoryError::NotFound(_)) => return not_found(&st, &locale),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let spool = match form.to_edit(spool_id, current.remaining_weight, current.status) {
        Ok(s) => s,
        Err(key) => {
            return render_edit_form(
                &st,
                &locale,
                theme.data_attr(),
                &form,
                EditFormRender {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    id: &id,
                    error_key: Some(key),
                    full_page: false,
                },
            )
            .await;
        }
    };

    match st.spools.edit(spool).await {
        // htmx's `HX-Redirect` header triggers a full client-side navigation
        // (`window.location`) rather than swapping this response into the
        // form's target — success leaves the edit form for the spool list.
        Ok(_) => Response::builder()
            .status(StatusCode::OK)
            .header("HX-Redirect", "/spools")
            .body(Body::empty())
            .unwrap(),
        Err(RepositoryError::UnknownMaterial(_)) => {
            render_edit_form(
                &st,
                &locale,
                theme.data_attr(),
                &form,
                EditFormRender {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    id: &id,
                    error_key: Some("spools.new.error.material"),
                    full_page: false,
                },
            )
            .await
        }
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/spools", get(list_page).post(create))
        .route("/spools/rows", get(rows))
        .route("/spools/new", get(new_page))
        .route("/spools/{id}/edit", get(edit_page))
        .route("/spools/{id}", get(detail_page).put(update))
        .route("/spools/{id}/remaining", post(set_remaining))
        .route("/spools/{id}/consume", post(consume))
        .route("/spools/{id}/archive", post(archive))
        .route("/spools/{id}/restore", post(restore))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;

    fn view(id: &str, status: &str) -> SpoolView {
        SpoolView {
            id: id.into(),
            material_name: "PLA".into(),
            colour_hex: "#1A9E4B".into(),
            colour_label: "vert sapin".into(),
            diameter: "1.75".into(),
            remaining_weight: 800.0,
            net_weight: 1000.0,
            remaining_pct: 80,
            remaining_length_m: 268.2,
            status: status.into(),
        }
    }

    fn material_option() -> MaterialOption {
        MaterialOption {
            id: "01HMAT".into(),
            name: "PLA".into(),
        }
    }

    fn render_list(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Sealed")]);
        ctx.insert("materials", &vec![material_option()]);
        ctx.insert("selected_material", "");
        ctx.insert("selected_status", "");
        ctx.insert("selected_sort", "created_desc");
        ctx.insert("stock_value", "37.50");
        r.render("spools.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn list_page_shows_spool_and_material_option_no_raw_keys() {
        let html = render_list("en");
        assert!(html.contains("PLA"));
        assert!(html.contains("80")); // remaining pct
        assert!(html.contains("#1A9E4B"));
        assert!(html.contains("/spools/01HSP"));
        assert!(!html.contains("spools.col.")); // no raw i18n key leaks
        assert!(!html.contains("spools.status.")); // status label resolved
    }

    #[test]
    fn list_page_localises_to_french() {
        let html = render_list("fr");
        assert!(html.contains("Bobines") || html.contains("Matériau"));
        assert!(!html.contains("spools.col."));
    }

    #[test]
    fn list_page_shows_stock_value_stat_no_raw_keys() {
        let html = render_list("en");
        assert!(html.contains(r#"id="stock-value""#));
        assert!(html.contains("Stock value"));
        assert!(html.contains("37.50"));
        assert!(!html.contains("spools.stock_value")); // no raw i18n key leaks
    }

    #[test]
    fn list_page_stock_value_localises_to_french() {
        let html = render_list("fr");
        assert!(html.contains("Valeur du stock"));
        assert!(!html.contains("spools.stock_value"));
    }

    #[test]
    fn rows_fragment_renders_only_rows_no_page_shell() {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Open")]);
        ctx.insert("stock_value", "37.50");
        let html = r.render("_spool_rows.html", "en", "", ctx).unwrap();
        assert!(html.contains("01HSP"));
        assert!(!html.contains("<html")); // fragment only, no full page shell
        assert!(!html.contains("<table")); // tbody content only, no wrapper
    }

    #[test]
    fn rows_fragment_includes_oob_stock_value_span() {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Open")]);
        ctx.insert("stock_value", "37.50");
        let html = r.render("_spool_rows.html", "en", "", ctx).unwrap();
        assert!(html.contains(r#"id="stock-value" hx-swap-oob="true""#));
        assert!(html.contains("37.50"));
    }

    fn detail_view(id: &str) -> SpoolDetailView {
        SpoolDetailView {
            id: id.into(),
            material_name: "PLA".into(),
            colour_hex: "#1A9E4B".into(),
            colour_label: "vert sapin".into(),
            has_colour_name: true,
            diameter: "1.75".into(),
            net_weight: 1000.0,
            remaining_weight: 800.0,
            remaining_pct: 80,
            remaining_length_m: 268.2,
            price_paid: "24.99".into(),
            status: "Open".into(),
        }
    }

    fn render_detail(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spool", &detail_view("01HSP"));
        r.render("spools_detail.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn detail_page_shows_all_fields_and_derived_values_no_raw_keys() {
        let html = render_detail("en");
        assert!(html.contains("PLA"));
        assert!(html.contains("#1A9E4B"));
        assert!(html.contains("vert sapin"));
        assert!(html.contains("1.75"));
        assert!(html.contains("1000"));
        assert!(html.contains("800"));
        assert!(html.contains("80%"));
        assert!(html.contains("268.2"));
        assert!(html.contains("24.99"));
        assert!(html.contains("/spools/01HSP/edit"));
        assert!(!html.contains("spools.col."));
        assert!(!html.contains("spools.detail."));
        assert!(!html.contains("spools.status."));
    }

    #[test]
    fn detail_page_localises_to_french() {
        let html = render_detail("fr");
        assert!(html.contains("Détail de la bobine") || html.contains("Matériau"));
        assert!(!html.contains("spools.col."));
        assert!(!html.contains("spools.detail."));
    }

    #[test]
    fn query_maps_status_and_material_filter() {
        let q = SpoolQuery {
            material_id: Some("01HMAT".into()),
            status: Some("Open".into()),
            sort: Some("remaining_asc".into()),
        };
        let filter = q.to_filter();
        assert_eq!(filter.material_id, Some(MaterialId::new("01HMAT")));
        assert_eq!(filter.status, Some(SpoolStatus::Open));
        assert_eq!(q.to_sort(), SpoolSort::RemainingRatioAsc);
    }

    #[test]
    fn empty_query_means_no_filter_and_created_desc_sort() {
        let q = SpoolQuery::default();
        let filter = q.to_filter();
        assert_eq!(filter.material_id, None);
        assert_eq!(filter.status, None);
        assert_eq!(q.to_sort(), SpoolSort::CreatedDesc);
        assert_eq!(q.selected_sort(), "created_desc");
    }

    fn render_new_form(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("materials", &vec![material_option()]);
        ctx.insert("material_id", "");
        ctx.insert("colour_hex", "");
        ctx.insert("colour_name", "");
        ctx.insert("diameter", "1.75");
        ctx.insert("net_weight", "");
        ctx.insert("price_paid", "");
        ctx.insert("error_key", &Option::<&str>::None);
        r.render("spools_new.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn new_form_lists_material_option_no_raw_keys() {
        let html = render_new_form("en");
        assert!(html.contains("PLA"));
        assert!(html.contains(r#"value="01HMAT""#));
        assert!(!html.contains("spools.new.")); // no raw i18n key leaks
    }

    #[test]
    fn new_form_localises_to_french() {
        let html = render_new_form("fr");
        assert!(html.contains("Ajouter une bobine") || html.contains("Matériau"));
        assert!(!html.contains("spools.new."));
    }

    #[test]
    fn new_form_shows_localized_error_when_error_key_set() {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("materials", &vec![material_option()]);
        ctx.insert("material_id", "");
        ctx.insert("colour_hex", "not-a-hex");
        ctx.insert("colour_name", "");
        ctx.insert("diameter", "1.75");
        ctx.insert("net_weight", "1000");
        ctx.insert("price_paid", "24.99");
        ctx.insert("error_key", &Some("spools.new.error.colour"));
        let html = r.render("spools_new.html", "en", "", ctx).unwrap();
        assert!(html.contains("must be a valid #RRGGBB hex code"));
        assert!(html.contains(r#"value="not-a-hex""#)); // submitted value echoed back
    }

    fn valid_form(material_id: &str) -> SpoolForm {
        SpoolForm {
            material_id: material_id.to_string(),
            colour_hex: "#1A9E4B".to_string(),
            colour_name: "vert sapin".to_string(),
            diameter: "1.75".to_string(),
            net_weight: "1000".to_string(),
            price_paid: "24.99".to_string(),
        }
    }

    /// A `NewSpool` matching `valid_form`'s values — used by the handler
    /// tests below to seed a stub-backed spool via `SpoolsUseCases::add`.
    fn valid_new_spool(material_id: &str) -> NewSpool {
        valid_form(material_id).to_new().unwrap()
    }

    #[test]
    fn to_new_maps_valid_form_to_domain_values() {
        let new = valid_form("01HMAT").to_new().unwrap();
        assert_eq!(new.material_id, MaterialId::new("01HMAT"));
        assert_eq!(new.colour.hex(), "#1A9E4B");
        assert_eq!(new.colour.name(), Some("vert sapin"));
        assert_eq!(new.diameter, Diameter::Mm1_75);
        assert_eq!(new.net_weight.value(), 1000.0);
        assert_eq!(
            new.price_paid,
            Money::from_decimal(Decimal::from_str_exact("24.99").unwrap()).unwrap()
        );
    }

    #[test]
    fn to_new_rejects_bad_hex() {
        let mut f = valid_form("01HMAT");
        f.colour_hex = "not-a-hex".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.colour"));
    }

    #[test]
    fn to_new_rejects_unknown_diameter() {
        let mut f = valid_form("01HMAT");
        f.diameter = "3.00".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.diameter"));
    }

    #[test]
    fn to_new_rejects_non_numeric_weight() {
        let mut f = valid_form("01HMAT");
        f.net_weight = "not-a-number".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.weight"));
    }

    #[test]
    fn to_new_rejects_negative_weight() {
        let mut f = valid_form("01HMAT");
        f.net_weight = "-5".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.weight"));
    }

    #[test]
    fn to_new_rejects_zero_weight() {
        let mut f = valid_form("01HMAT");
        f.net_weight = "0".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.weight"));
    }

    #[test]
    fn to_new_rejects_bad_decimal_price() {
        let mut f = valid_form("01HMAT");
        f.price_paid = "not-a-price".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.price"));
    }

    #[test]
    fn to_new_rejects_negative_price() {
        let mut f = valid_form("01HMAT");
        f.price_paid = "-1.00".to_string();
        assert_eq!(f.to_new(), Err("spools.new.error.price"));
    }

    #[test]
    fn to_new_rejects_blank_material() {
        let f = valid_form("  ");
        assert_eq!(f.to_new(), Err("spools.new.error.material"));
    }

    // --- Handler-level tests: exercise `new_page`/`create` directly against
    // an in-memory `AppState` (stub repositories + a lazily-connected pool
    // that is never queried by these handlers) — mirrors materials' render
    // tests but also drives the actual async handlers, per the add-spool
    // task brief.
    mod handlers {
        use super::*;
        use crate::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
        use axum::body::to_bytes;
        use domain::locations::stubs::StubLocationRepository;
        use domain::locations::{LocationsService, LocationsUseCases};
        use domain::materials::stubs::StubMaterialRepository;
        use domain::materials::{
            Density, DryingParams, MaterialName, MaterialRepository, MaterialsService,
            MaterialsUseCases, NewMaterial, Sensitivity, Temperature,
        };
        use domain::spools::stubs::StubSpoolRepository;
        use domain::spools::{SpoolRepository, SpoolsService, SpoolsUseCases};
        use sqlx::PgPool;
        use std::sync::Arc;

        fn sample_new_material() -> NewMaterial {
            NewMaterial {
                name: MaterialName::new("PLA").unwrap(),
                density: Density::new(1.24).unwrap(),
                drying: DryingParams {
                    temp: Temperature::new(45),
                    time_h: 6,
                },
                sensitivity: Sensitivity::Low,
                nozzle: Temperature::new(210),
                bed: Temperature::new(60),
            }
        }

        /// A ready-to-use `AppState` backed by in-memory stub repositories.
        /// `db` is a lazily-connected pool (never actually dialed — none of
        /// `new_page`/`create` touch `AppState::db`), so this needs no
        /// database to run.
        async fn test_state() -> (AppState, String) {
            let materials_repo: Arc<dyn MaterialRepository> =
                Arc::new(StubMaterialRepository::new());
            let materials: Arc<dyn MaterialsUseCases> =
                Arc::new(MaterialsService::new(materials_repo));
            let seeded = materials.add(sample_new_material()).await.unwrap();

            let spools_repo: Arc<dyn SpoolRepository> = Arc::new(StubSpoolRepository::new());
            let spools: Arc<dyn SpoolsUseCases> = Arc::new(SpoolsService::new(spools_repo));

            let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(Arc::new(
                StubLocationRepository::new(),
            )));

            let db = PgPool::connect_lazy("postgres://user:pass@localhost/db").unwrap();
            let cfg = Config {
                server: ServerConfig {
                    bind: "127.0.0.1:0".into(),
                },
                database: DatabaseConfig {
                    url: "postgres://user:pass@localhost/db".into(),
                },
                i18n: I18nConfig {
                    default_locale: "en".into(),
                },
            };
            (
                AppState::new(db, &cfg, materials, spools, locations),
                seeded.id.as_str().to_string(),
            )
        }

        async fn body_of(res: Response) -> String {
            let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        }

        #[tokio::test]
        async fn get_new_renders_form_with_material_option() {
            let (st, material_id) = test_state().await;
            let res = new_page(State(st), HeaderMap::new()).await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("PLA"));
            assert!(html.contains(&format!(r#"value="{material_id}""#)));
        }

        #[tokio::test]
        async fn post_valid_form_adds_spool_and_redirects() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let res = create(State(st), HeaderMap::new(), Form(valid_form(&material_id))).await;
            assert_eq!(res.status(), StatusCode::SEE_OTHER);
            assert_eq!(
                res.headers().get("location").unwrap().to_str().unwrap(),
                "/spools"
            );
            let created = spools
                .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
                .await
                .unwrap();
            assert_eq!(created.len(), 1);
        }

        #[tokio::test]
        async fn post_bad_hex_rerenders_form_with_error_and_does_not_create() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let mut form = valid_form(&material_id);
            form.colour_hex = "not-a-hex".to_string();
            let res = create(State(st), HeaderMap::new(), Form(form)).await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains("must be a valid #RRGGBB hex code"));
            assert!(html.contains(r#"value="not-a-hex""#)); // submitted value echoed
            let after = spools
                .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
                .await
                .unwrap();
            assert!(after.is_empty());
        }

        #[tokio::test]
        async fn post_zero_weight_rerenders_form_with_error_and_does_not_create() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let mut form = valid_form(&material_id);
            form.net_weight = "0".to_string();
            let res = create(State(st), HeaderMap::new(), Form(form)).await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains("greater than 0"));
            let after = spools
                .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
                .await
                .unwrap();
            assert!(after.is_empty());
        }

        #[tokio::test]
        async fn post_negative_price_rerenders_form_with_error_and_does_not_create() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let mut form = valid_form(&material_id);
            form.price_paid = "-1.00".to_string();
            let res = create(State(st), HeaderMap::new(), Form(form)).await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains(r#"value="-1.00""#)); // submitted value echoed
            let after = spools
                .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
                .await
                .unwrap();
            assert!(after.is_empty());
        }

        // --- Task 11: GET /spools/{id}/edit and PUT /spools/{id}.

        // --- Task 8: Stock Value stat on the list page.

        #[tokio::test]
        async fn get_list_shows_stock_value_for_seeded_spool() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            spools.add(valid_new_spool(&material_id)).await.unwrap();

            let res = list_page(State(st), HeaderMap::new(), Query(SpoolQuery::default())).await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains(r#"id="stock-value""#));
            assert!(html.contains("24.99")); // full sealed spool: remaining == net -> full price
        }

        #[tokio::test]
        async fn get_rows_includes_oob_stock_value_for_seeded_spool() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            spools.add(valid_new_spool(&material_id)).await.unwrap();

            let res = rows(State(st), HeaderMap::new(), Query(SpoolQuery::default())).await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains(r#"id="stock-value" hx-swap-oob="true""#));
            assert!(html.contains("24.99"));
        }

        #[tokio::test]
        async fn get_edit_prefills_form_from_stored_spool() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let res = edit_page(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains(&format!(r#"value="{material_id}""#)));
            assert!(html.contains(r##"value="#1A9E4B""##));
            assert!(html.contains(r#"value="1000""#));
            assert!(html.contains(r#"value="24.99""#));
        }

        #[tokio::test]
        async fn get_edit_unknown_id_returns_404() {
            let (st, _material_id) = test_state().await;
            let res = edit_page(
                State(st),
                HeaderMap::new(),
                Path("does-not-exist".to_string()),
            )
            .await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn put_valid_changes_preserves_remaining_and_status() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let mut created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            // Simulate a spool that's been opened and drawn down before this edit.
            created.status = SpoolStatus::Open;
            created.remaining_weight = Grams::new(800.0).unwrap();
            spools.edit(created.clone()).await.unwrap();

            let mut form = valid_form(&material_id);
            form.colour_hex = "#00FF00".to_string();
            form.colour_name = "vert".to_string();
            let res = update(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(form),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get("HX-Redirect").unwrap().to_str().unwrap(),
                "/spools"
            );

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.colour.hex(), "#00FF00");
            assert_eq!(detail.status, SpoolStatus::Open);
            assert_eq!(detail.remaining_weight.value(), 800.0);
            assert_eq!(detail.net_weight.value(), 1000.0);
        }

        #[tokio::test]
        async fn put_lowering_net_below_remaining_clamps_remaining() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let mut created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            created.remaining_weight = Grams::new(800.0).unwrap();
            spools.edit(created.clone()).await.unwrap();

            let mut form = valid_form(&material_id);
            form.net_weight = "500".to_string();
            let res = update(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(form),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.net_weight.value(), 500.0);
            assert_eq!(detail.remaining_weight.value(), 500.0); // clamped by the use case
        }

        #[tokio::test]
        async fn put_bad_hex_rerenders_form_with_error_and_does_not_change_spool() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let mut form = valid_form(&material_id);
            form.colour_hex = "not-a-hex".to_string();
            let res = update(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(form),
            )
            .await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains("must be a valid #RRGGBB hex code"));
            assert!(html.contains(r#"value="not-a-hex""#)); // submitted value echoed

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.colour.hex(), "#1A9E4B"); // unchanged
        }

        // --- Task 12: GET /spools/{id} (detail view).

        #[tokio::test]
        async fn get_detail_renders_spool_fields_and_derived_values() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let mut created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            // Draw the spool down so remaining ratio/length differ from net.
            created.remaining_weight = Grams::new(800.0).unwrap();
            spools.edit(created.clone()).await.unwrap();

            let res = detail_page(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("stub")); // material name (stub repo's fixed join value)
            assert!(html.contains("#1A9E4B")); // colour hex
            assert!(html.contains("vert sapin")); // colour name
            assert!(html.contains("1.75")); // diameter
            assert!(html.contains("1000")); // net weight
            assert!(html.contains("800")); // remaining weight
            assert!(html.contains("80%")); // remaining ratio (800/1000)
            assert!(html.contains("24.99")); // price paid
            assert!(html.contains(&format!("/spools/{}/edit", created.id.as_str()))); // edit link
            assert!(!html.contains("spools.col.")); // no raw i18n key leaks
            assert!(!html.contains("spools.detail.")); // no raw i18n key leaks
        }

        #[tokio::test]
        async fn get_detail_unknown_id_returns_404() {
            let (st, _material_id) = test_state().await;
            let res = detail_page(
                State(st),
                HeaderMap::new(),
                Path("does-not-exist".to_string()),
            )
            .await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn put_unknown_id_returns_404() {
            let (st, material_id) = test_state().await;
            let res = update(
                State(st),
                HeaderMap::new(),
                Path("does-not-exist".to_string()),
                Form(valid_form(&material_id)),
            )
            .await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
        }

        // --- Task 7: detail-page weight ops + archive/restore (htmx).

        #[tokio::test]
        async fn post_consume_drives_status_open_and_returns_fragment() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let res = consume(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(ConsumeForm {
                    amount: "300".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(!html.contains("<html")); // fragment only, no page shell
            assert!(!html.contains("<table"));
            assert!(html.contains("700")); // 1000 - 300 remaining
            assert!(html.contains("Open")); // spools.status.open label (en)

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.remaining_weight.value(), 700.0);
            assert_eq!(detail.status, SpoolStatus::Open);
        }

        #[tokio::test]
        async fn post_set_remaining_above_net_returns_422_with_error() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let res = set_remaining(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(RemainingForm {
                    remaining: "1500".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains("Remaining cannot exceed net weight."));

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.remaining_weight.value(), 1000.0); // unchanged
        }

        #[tokio::test]
        async fn post_archive_then_restore_toggles_card_controls() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            let id = created.id.as_str().to_string();
            let restore_post = format!(r#"hx-post="/spools/{id}/restore""#);
            let remaining_post = format!(r#"hx-post="/spools/{id}/remaining""#);
            let consume_post = format!(r#"hx-post="/spools/{id}/consume""#);

            let archived = archive(State(st.clone()), HeaderMap::new(), Path(id.clone())).await;
            assert_eq!(archived.status(), StatusCode::OK);
            let html = body_of(archived).await;
            assert!(html.contains(&restore_post)); // Restore control shown
            assert!(!html.contains(&remaining_post)); // weight forms hidden
            assert!(!html.contains(&consume_post));

            let restored = restore(State(st), HeaderMap::new(), Path(id)).await;
            assert_eq!(restored.status(), StatusCode::OK);
            let html = body_of(restored).await;
            assert!(html.contains(&remaining_post)); // ops forms shown again
            assert!(html.contains(&consume_post));
            assert!(!html.contains(&restore_post));
        }

        #[tokio::test]
        async fn post_consume_unknown_id_returns_404() {
            let (st, _material_id) = test_state().await;
            let res = consume(
                State(st),
                HeaderMap::new(),
                Path("does-not-exist".to_string()),
                Form(ConsumeForm {
                    amount: "10".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
        }

        // --- Review fix 1: non-finite weight input (NaN/Inf) must be
        // rejected the same way a negative or non-numeric weight is —
        // `Grams::new` rejects non-finite values at the domain root, so
        // `"nan"`/`"inf"` are caught by `parse_grams`'s `Grams::new` call.
        #[tokio::test]
        async fn post_consume_nan_amount_returns_422_and_does_not_change_spool() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            let before = spools.view(created.id.clone()).await.unwrap();

            let res = consume(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(ConsumeForm {
                    amount: "nan".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains("Enter a weight of 0 or more."));

            let after = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(
                after.remaining_weight.value(),
                before.remaining_weight.value()
            );
        }

        // --- Review fix 3: a weight op on an already-archived spool must
        // map `RepositoryError::Domain(DomainError::SpoolArchived)` to a 422
        // with the localized "This spool is archived." text, over HTTP.
        #[tokio::test]
        async fn post_consume_on_archived_spool_returns_422_with_archived_error() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            spools.archive(created.id.clone()).await.unwrap();

            let res = consume(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(ConsumeForm {
                    amount: "10".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let html = body_of(res).await;
            assert!(html.contains("This spool is archived."));
        }

        #[test]
        fn card_fragment_localises_to_french_no_raw_keys() {
            let r = Renderer::new(Catalog::load("en"));
            let mut ctx = Context::new();
            ctx.insert("spool", &detail_view("01HSP"));
            ctx.insert("error_key", &Option::<&str>::None);
            let html = r.render("_spool_detail_card.html", "fr", "", ctx).unwrap();
            // Tera HTML-escapes the apostrophe (`&#39;`) — match the actual
            // rendered entity, not the raw catalog string.
            assert!(html.contains("Enregistrer l&#39;utilisation"));
            assert!(html.contains("Archiver"));
            assert!(!html.contains("spools.")); // no raw i18n key leaks
        }
    }
}
