//! The driving (Axum) adapter for the spools slice: a read-only, filterable
//! and sortable list (`GET /spools`) whose filter/sort bar issues htmx GETs
//! to `GET /spools/rows`, swapping only the `<tbody>` — no full reload.
//! Mirrors `web::materials` for locale/theme resolution and Tera rendering.
//!
//! The material dropdown is populated by calling `AppState::materials`
//! (the materials use cases already wired into shared state) — this is
//! web-layer composition across two driving-adapter handlers, not a domain
//! import across slices (the spools domain never depends on materials).

use crate::web::router::{internal_error, resolve_locale, resolve_theme};
use crate::web::state::AppState;
use crate::web::templates::Renderer;
use axum::{
    Router,
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use domain::manufacturers::{ManufacturerName, NewManufacturer};
use domain::shared::{DomainError, Grams, LocationId, ManufacturerId, MaterialId, Money};
use domain::spools::{
    Colour, Diameter, EditSpool, NewSpool, RepositoryError, Spool, SpoolCondition, SpoolDetail,
    SpoolFilter, SpoolId, SpoolListItem, SpoolSort, SpoolStatus,
    remaining_length_m as calculate_length_m,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tera::Context;
use time::{Date, format_description};

/// Template-shaped view of a `SpoolListItem`: plain strings/numbers plus the
/// derived percentage/length fields (the domain never exposes a "view
/// model").
#[derive(Serialize)]
pub struct SpoolView {
    pub id: String,
    pub material_name: String,
    pub colour_hex: String,
    /// The localized human name derived from the colour value.
    pub colour_label: String,
    pub diameter: String, // "1.75" | "2.85"
    pub remaining_weight: f64,
    pub net_weight: f64,
    pub remaining_pct: u8,
    pub remaining_length_m: f64,
    pub status: String, // "Sealed" | "Open" | "Empty" | "Archived"
    /// The assigned location's display name, or `None` when unassigned —
    /// the card view shows this as the spool's storage.
    pub location_name: Option<String>,
    /// The manufacturer's display name, or `None` when unattributed.
    pub manufacturer_name: Option<String>,
}

impl SpoolView {
    fn localized(item: SpoolListItem, renderer: &Renderer, locale: &str) -> Self {
        let remaining_pct = (item.remaining_ratio() * 100.0).round();
        let remaining_length_m = round1(item.remaining_length_m());
        let (colour_hex, colour_label) = item
            .colour
            .as_ref()
            .map(|colour| {
                (
                    colour.hex().to_string(),
                    localized_colour_name(renderer, locale, colour),
                )
            })
            .unwrap_or_default();
        Self {
            location_name: item.location_name.clone(),
            manufacturer_name: item.manufacturer_name.clone(),
            id: item.id.as_str().to_string(),
            material_name: item.material_name.clone(),
            colour_hex,
            colour_label,
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
    /// The localized human name derived from the colour value.
    pub colour_label: String,
    /// Whether a colour is present.
    pub has_colour_name: bool,
    pub diameter: String, // "1.75" | "2.85"
    pub net_weight: f64,
    pub remaining_weight: f64,
    pub remaining_pct: u8,
    pub remaining_length_m: f64,
    pub total_length_m: f64,
    pub price_paid: String,
    pub current_value: String,
    pub status: String, // "Sealed" | "Open" | "Empty" | "Archived"
    /// The assigned location's display name, or `None` when unassigned —
    /// the detail card shows this (or the "unassigned" label).
    pub location_name: Option<String>,
    /// The assigned location's id, or `None` when unassigned — used to
    /// preselect the current option in the reassign `<select>`.
    pub location_id: Option<String>,
    /// The manufacturer's display name, or `None` when unattributed.
    pub manufacturer_name: Option<String>,
    pub notes: Option<String>,
    pub purchased_at: Option<String>,
    pub opened_at: Option<String>,
}

impl SpoolDetailView {
    fn localized(d: SpoolDetail, renderer: &Renderer, locale: &str) -> Self {
        let remaining_pct = (d.remaining_ratio() * 100.0).round();
        let remaining_length_m = round1(d.remaining_length_m());
        let total_length_m = round1(calculate_length_m(d.net_weight, d.density, d.diameter));
        let current_value = Money::from_decimal(
            d.price_paid.value()
                * Decimal::from_f64_retain(d.remaining_ratio()).unwrap_or(Decimal::ZERO),
        )
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "0.00".to_string());
        let (colour_hex, colour_label, has_colour_name) = d
            .colour
            .as_ref()
            .map(|colour| {
                (
                    colour.hex().to_string(),
                    localized_colour_name(renderer, locale, colour),
                    true,
                )
            })
            .unwrap_or_default();
        Self {
            id: d.id.as_str().to_string(),
            material_name: d.material_name.clone(),
            colour_hex,
            colour_label,
            has_colour_name,
            diameter: d.diameter.as_str().to_string(),
            net_weight: round1(d.net_weight.value()),
            remaining_weight: round1(d.remaining_weight.value()),
            remaining_pct: remaining_pct as u8, // saturating cast — no panic on out-of-range ratios
            remaining_length_m,
            total_length_m,
            price_paid: d.price_paid.to_string(),
            current_value,
            status: d.status.as_str().to_string(),
            location_name: d.location_name.clone(),
            location_id: d.location_id.clone(),
            manufacturer_name: d.manufacturer_name.clone(),
            notes: d.notes.clone(),
            purchased_at: d.purchased_at.map(|date| date.to_string()),
            opened_at: d.opened_at.map(|date| date.to_string()),
        }
    }
}

fn localized_colour_name(renderer: &Renderer, locale: &str, colour: &Colour) -> String {
    let key = colour.name().expect("a colour always has a derived name");
    renderer.t(locale, &format!("spools.colour.preset.{key}"))
}

/// A material option for the filter dropdown — the web layer's own
/// composition of `AppState::materials`, not a domain read model.
#[derive(Serialize)]
pub struct MaterialOption {
    pub id: String,
    pub name: String,
}

/// A location option for the assignment `<select>` — the web layer's own
/// composition of `AppState::locations`, mirroring `MaterialOption`. The
/// `spools` domain never depends on `locations` (slice isolation); this is
/// web-layer composition across two driving-adapter handlers.
#[derive(Serialize)]
pub struct LocationOption {
    pub id: String,
    pub name: String,
}

/// A manufacturer option for the add-spool `<select>` — the web layer's own
/// composition of `AppState::manufacturers`, mirroring `LocationOption`.
#[derive(Serialize)]
pub struct ManufacturerOption {
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
    pub manufacturer_id: Option<String>,
    #[serde(default)]
    pub location_id: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
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
            manufacturer_id: self
                .manufacturer_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| ManufacturerId::new(s.to_string())),
            location_id: self
                .location_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| LocationId::new(s.to_string())),
            search: self
                .search
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
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
        .map_err(internal_error)
}

async fn location_options(st: &AppState) -> Result<Vec<LocationOption>, Response> {
    st.locations
        .list()
        .await
        .map(|ls| {
            ls.into_iter()
                .map(|l| LocationOption {
                    id: l.id.as_str().to_string(),
                    name: l.name.as_str().to_string(),
                })
                .collect()
        })
        .map_err(internal_error)
}

async fn manufacturer_options(st: &AppState) -> Result<Vec<ManufacturerOption>, Response> {
    st.manufacturers
        .list()
        .await
        .map(|ms| {
            ms.into_iter()
                .map(|m| ManufacturerOption {
                    id: m.id.as_str().to_string(),
                    name: m.name.as_str().to_string(),
                })
                .collect()
        })
        .map_err(internal_error)
}

fn render_rows(
    st: &AppState,
    locale: &str,
    items: Vec<SpoolListItem>,
    stock_value: &str,
    total_count: usize,
    low_stock_threshold_pct: u8,
) -> Response {
    let filtered_count = items.len();
    let views: Vec<SpoolView> = items
        .into_iter()
        .map(|item| SpoolView::localized(item, &st.renderer, locale))
        .collect();
    let mut ctx = Context::new();
    ctx.insert("spools", &views);
    ctx.insert("stock_value", stock_value);
    ctx.insert("filtered_count", &filtered_count);
    ctx.insert("total_count", &total_count);
    ctx.insert("low_stock_threshold_pct", &low_stock_threshold_pct);
    match st.renderer.render("_spool_rows.html", locale, "", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal_error(e),
    }
}

/// Denominator of the list's "X sur Y affichées" counter: the number of
/// spools in the current *status* scope, ignoring the refining facets
/// (search / material / manufacturer / location). Scoping by status keeps
/// the filtered count a subset of the total (so it never reads "3 sur 2"
/// when the Archived filter is active). Reuses `list` length rather than a
/// dedicated count query (brief-sanctioned).
async fn status_scoped_total(st: &AppState, q: &SpoolQuery) -> Result<usize, Response> {
    let scope = SpoolFilter {
        status: q.to_filter().status,
        ..Default::default()
    };
    st.spools
        .list(scope, SpoolSort::CreatedDesc)
        .await
        .map(|items| items.len())
        .map_err(internal_error)
}

async fn list_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpoolQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let low_stock_threshold_pct = match st.instance_configuration.get().await {
        Ok(configuration) => configuration.low_stock_threshold.percent(),
        Err(e) => return internal_error(e),
    };

    let materials = match material_options(&st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let manufacturers = match manufacturer_options(&st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let locations = match location_options(&st).await {
        Ok(ls) => ls,
        Err(resp) => return resp,
    };
    let total_count = match status_scoped_total(&st, &q).await {
        Ok(n) => n,
        Err(resp) => return resp,
    };

    let filter = q.to_filter();
    let stock_value = match st.spools.stock_value(filter.clone()).await {
        Ok(v) => v.to_string(),
        Err(e) => return internal_error(e),
    };

    match st.spools.list(filter, q.to_sort()).await {
        Ok(items) => {
            let filtered_count = items.len();
            let views: Vec<SpoolView> = items
                .into_iter()
                .map(|item| SpoolView::localized(item, &st.renderer, &locale))
                .collect();
            let mut ctx = Context::new();
            ctx.insert("spools", &views);
            ctx.insert("materials", &materials);
            ctx.insert("manufacturers", &manufacturers);
            ctx.insert("locations", &locations);
            ctx.insert("selected_material", q.material_id.as_deref().unwrap_or(""));
            ctx.insert("selected_status", q.status.as_deref().unwrap_or(""));
            ctx.insert(
                "selected_manufacturer",
                q.manufacturer_id.as_deref().unwrap_or(""),
            );
            ctx.insert("selected_location", q.location_id.as_deref().unwrap_or(""));
            ctx.insert("search", q.search.as_deref().unwrap_or(""));
            ctx.insert("selected_sort", q.selected_sort());
            ctx.insert("stock_value", &stock_value);
            ctx.insert("filtered_count", &filtered_count);
            ctx.insert("total_count", &total_count);
            ctx.insert("low_stock_threshold_pct", &low_stock_threshold_pct);
            ctx.insert("page", "spools");
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &st.nav_printer_count().await);
            match st
                .renderer
                .render("spools.html", &locale, theme.data_attr(), ctx)
            {
                Ok(html) => Html(html).into_response(),
                Err(e) => internal_error(e),
            }
        }
        Err(e) => internal_error(e),
    }
}

async fn rows(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SpoolQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let low_stock_threshold_pct = match st.instance_configuration.get().await {
        Ok(configuration) => configuration.low_stock_threshold.percent(),
        Err(e) => return internal_error(e),
    };
    let filter = q.to_filter();
    let stock_value = match st.spools.stock_value(filter.clone()).await {
        Ok(v) => v.to_string(),
        Err(e) => return internal_error(e),
    };
    let total_count = match status_scoped_total(&st, &q).await {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    match st.spools.list(filter, q.to_sort()).await {
        Ok(items) => render_rows(
            &st,
            &locale,
            items,
            &stock_value,
            total_count,
            low_stock_threshold_pct,
        ),
        Err(e) => internal_error(e),
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
    pub condition: String,
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
    pub remaining_weight: String,
    #[serde(default)]
    pub price_paid: String,
    /// The `<select name="location_id">` value — blank (the leading empty
    /// `<option>`) means unassigned, matching `SpoolQuery::material_id`'s
    /// "blank = no filter" convention. `#[serde(default)]` so a form payload
    /// posted without this field at all (defensive) still deserializes.
    #[serde(default)]
    pub location_id: String,
    /// The `<select name="manufacturer_id">` value — blank means no
    /// manufacturer, same "blank = unassigned" convention as `location_id`.
    #[serde(default)]
    pub manufacturer_id: String,
    #[serde(default)]
    pub manufacturer_name: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub purchased_at: String,
    #[serde(default)]
    pub opened_at: String,
}

/// The fields shared by `NewSpool` (create) and an edited `Spool` (update) —
/// everything the form itself supplies, validated once and reused by both
/// `to_new` and `to_edit` so the two call sites can't drift.
struct SpoolFormFields {
    material_id: MaterialId,
    colour: Option<Colour>,
    diameter: Diameter,
    net_weight: Grams,
    price_paid: Money,
    location_id: Option<LocationId>,
    manufacturer_id: Option<ManufacturerId>,
    notes: Option<String>,
    purchased_at: Option<Date>,
    opened_at: Option<Date>,
}

fn parse_optional_date(raw: &str) -> Result<Option<Date>, &'static str> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let format = format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|_| "spools.new.error.date")?;
    Date::parse(value, &format)
        .map(Some)
        .map_err(|_| "spools.new.error.date")
}

/// Maps a raw `location_id` form value to the domain optional id: blank or
/// whitespace-only ⇒ unassigned (`None`), matching the "blank select ⇒
/// unassigned" convention used across all three assignment surfaces (add
/// form, edit form, detail-card reassign). No format validation happens
/// here — an unknown-but-non-blank id is caught defensively by the
/// persistence adapter's FK constraint (`RepositoryError::UnknownLocation`),
/// not by this parse step.
fn parse_location_id(raw: &str) -> Option<LocationId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(LocationId::new(trimmed.to_string()))
    }
}

/// Maps a raw `manufacturer_id` form value to the domain optional id: blank
/// ⇒ no manufacturer. Mirrors `parse_location_id`; an unknown-but-non-blank
/// id is caught defensively by the FK constraint
/// (`RepositoryError::UnknownManufacturer`).
fn parse_manufacturer_id(raw: &str) -> Option<ManufacturerId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "__other" {
        None
    } else {
        Some(ManufacturerId::new(trimmed.to_string()))
    }
}

enum ManufacturerResolutionError {
    InvalidName,
    Response(Response),
}

async fn resolve_manufacturer_id(
    st: &AppState,
    form: &SpoolForm,
) -> Result<Option<ManufacturerId>, ManufacturerResolutionError> {
    if form.manufacturer_id != "__other" {
        return Ok(parse_manufacturer_id(&form.manufacturer_id));
    }
    let name = ManufacturerName::new(form.manufacturer_name.clone())
        .map_err(|_| ManufacturerResolutionError::InvalidName)?;
    match st
        .manufacturers
        .add(NewManufacturer {
            name: name.clone(),
            country: None,
        })
        .await
    {
        Ok(created) => Ok(Some(created.id)),
        Err(domain::manufacturers::RepositoryError::Duplicate(_)) => st
            .manufacturers
            .list()
            .await
            .map_err(|e| ManufacturerResolutionError::Response(internal_error(e)))?
            .into_iter()
            .find(|manufacturer| manufacturer.name == name)
            .map(|manufacturer| Some(manufacturer.id))
            .ok_or_else(|| {
                ManufacturerResolutionError::Response(internal_error(
                    "duplicate manufacturer could not be resolved",
                ))
            }),
        Err(e) => Err(ManufacturerResolutionError::Response(internal_error(e))),
    }
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
        let colour = if self.colour_hex.trim().is_empty() {
            None
        } else {
            Some(
                Colour::from_hex(self.colour_hex.trim().to_string())
                    .map_err(|_| "spools.new.error.colour")?,
            )
        };
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
            location_id: parse_location_id(&self.location_id),
            manufacturer_id: parse_manufacturer_id(&self.manufacturer_id),
            notes: (!self.notes.trim().is_empty()).then(|| self.notes.trim().to_string()),
            purchased_at: parse_optional_date(&self.purchased_at)?,
            opened_at: parse_optional_date(&self.opened_at)?,
        })
    }

    /// Maps the raw form into a domain `NewSpool` (create path). Blank
    /// `location_id` ⇒ unassigned.
    fn to_new(&self) -> Result<NewSpool, &'static str> {
        let f = self.parse()?;
        let condition = SpoolCondition::parse(self.condition.trim())
            .map_err(|_| "spools.new.error.condition")?;
        let remaining_weight = if condition == SpoolCondition::Opened {
            let value = self
                .remaining_weight
                .trim()
                .parse::<f64>()
                .map_err(|_| "spools.new.error.remaining_weight")?;
            Some(Grams::new(value).map_err(|_| "spools.new.error.remaining_weight")?)
        } else {
            None
        };
        Ok(NewSpool {
            condition,
            material_id: f.material_id,
            colour: f.colour,
            diameter: f.diameter,
            net_weight: f.net_weight,
            price_paid: f.price_paid,
            location_id: f.location_id,
            manufacturer_id: f.manufacturer_id,
            notes: f.notes,
            purchased_at: f.purchased_at,
            opened_at: f.opened_at,
            remaining_weight,
        })
    }

    /// Maps the raw form into the edit use case's command. Status and
    /// remaining weight are derived from the selected condition by the
    /// domain service, exactly as on creation.
    fn to_edit(&self, id: SpoolId) -> Result<EditSpool, &'static str> {
        let f = self.parse()?;
        let condition = SpoolCondition::parse(self.condition.trim())
            .map_err(|_| "spools.new.error.condition")?;
        let remaining_weight = if condition == SpoolCondition::Opened {
            let value = self
                .remaining_weight
                .trim()
                .parse::<f64>()
                .map_err(|_| "spools.new.error.remaining_weight")?;
            Some(Grams::new(value).map_err(|_| "spools.new.error.remaining_weight")?)
        } else {
            None
        };
        Ok(EditSpool {
            id,
            condition,
            material_id: f.material_id,
            colour: f.colour,
            diameter: f.diameter,
            net_weight: f.net_weight,
            remaining_weight,
            price_paid: f.price_paid,
            location_id: f.location_id,
            manufacturer_id: f.manufacturer_id,
            notes: f.notes,
            purchased_at: f.purchased_at,
            opened_at: f.opened_at,
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
    full_page: bool,
) -> Response {
    let materials = match material_options(st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let locations = match location_options(st).await {
        Ok(ls) => ls,
        Err(resp) => return resp,
    };
    let manufacturers = match manufacturer_options(st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let mut ctx = Context::new();
    ctx.insert("materials", &materials);
    ctx.insert("locations", &locations);
    ctx.insert("manufacturers", &manufacturers);
    ctx.insert("condition", &form.condition);
    ctx.insert("material_id", &form.material_id);
    ctx.insert("location_id", &form.location_id);
    ctx.insert("manufacturer_id", &form.manufacturer_id);
    ctx.insert("manufacturer_name", &form.manufacturer_name);
    ctx.insert("colour_hex", &form.colour_hex);
    let derived_colour_name = Colour::from_hex(form.colour_hex.clone())
        .ok()
        .and_then(|colour| colour.name().map(str::to_string))
        .unwrap_or_default();
    ctx.insert("colour_name", &derived_colour_name);
    ctx.insert("diameter", &form.diameter);
    ctx.insert("net_weight", &form.net_weight);
    ctx.insert("remaining_weight", &form.remaining_weight);
    ctx.insert("price_paid", &form.price_paid);
    ctx.insert("notes", &form.notes);
    ctx.insert("purchased_at", &form.purchased_at);
    ctx.insert("opened_at", &form.opened_at);
    ctx.insert("error_key", &error_key);
    ctx.insert("wizard_step", "details");
    ctx.insert("edit_mode", &false);
    ctx.insert(
        "net_weight_is_custom",
        &net_weight_is_custom(&form.net_weight),
    );
    if full_page {
        ctx.insert("nav_spool_count", &st.nav_spool_count().await);
        ctx.insert("nav_printer_count", &st.nav_printer_count().await);
    }
    let template = if full_page {
        "spools_new.html"
    } else {
        "_spool_wizard_details.html"
    };
    match st.renderer.render(template, locale, theme_attr, ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => internal_error(e),
    }
}

fn net_weight_is_custom(net_weight: &str) -> bool {
    !matches!(
        net_weight,
        "" | "250" | "500" | "750" | "900" | "1000" | "2000" | "3000" | "5000"
    )
}

async fn render_condition(
    st: &AppState,
    locale: &str,
    theme_attr: &str,
    full_page: bool,
) -> Response {
    let mut ctx = Context::new();
    ctx.insert("page", "spools");
    ctx.insert("wizard_step", "condition");
    ctx.insert("edit_mode", &false);
    if full_page {
        ctx.insert("nav_spool_count", &st.nav_spool_count().await);
        ctx.insert("nav_printer_count", &st.nav_printer_count().await);
    }
    let template = if full_page {
        "spools_new.html"
    } else {
        "_spool_wizard_condition.html"
    };
    match st.renderer.render(template, locale, theme_attr, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn new_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    render_condition(&st, &locale, theme.data_attr(), true).await
}

#[derive(Deserialize)]
struct ConditionQuery {
    condition: String,
}

async fn condition_step(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    render_condition(&st, &locale, "", false).await
}

async fn details_step(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConditionQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    if SpoolCondition::parse(&query.condition).is_err() {
        return render_condition(&st, &locale, "", false).await;
    }
    let form = SpoolForm {
        condition: query.condition,
        diameter: "1.75".to_string(),
        net_weight: "1000".to_string(),
        ..Default::default()
    };
    render_form(&st, &locale, "", StatusCode::OK, &form, None, false).await
}

async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SpoolForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);

    let mut new = match form.to_new() {
        Ok(n) => n,
        Err(key) => {
            return render_form(
                &st,
                &locale,
                theme.data_attr(),
                StatusCode::UNPROCESSABLE_ENTITY,
                &form,
                Some(key),
                true,
            )
            .await;
        }
    };

    new.manufacturer_id = match resolve_manufacturer_id(&st, &form).await {
        Ok(id) => id,
        Err(ManufacturerResolutionError::InvalidName) => {
            return render_form(
                &st,
                &locale,
                theme.data_attr(),
                StatusCode::UNPROCESSABLE_ENTITY,
                &form,
                Some("spools.new.error.manufacturer"),
                true,
            )
            .await;
        }
        Err(ManufacturerResolutionError::Response(response)) => return response,
    };

    match st.spools.add(new).await {
        Ok(created) => Redirect::to(&format!("/spools/{}", created.id.as_str())).into_response(),
        Err(RepositoryError::UnknownMaterial(_)) => {
            render_form(
                &st,
                &locale,
                theme.data_attr(),
                StatusCode::UNPROCESSABLE_ENTITY,
                &form,
                Some("spools.new.error.material"),
                true,
            )
            .await
        }
        // An unknown location id from a rendered <select> is defensive
        // (never happens through normal use) — surface it as not-found,
        // mirroring how `NotFound` is handled elsewhere, rather than
        // misreporting it as an unknown material or 500-ing.
        Err(RepositoryError::UnknownLocation(_)) => not_found(&st, &locale),
        Err(e) => internal_error(e),
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
        Err(e) => return internal_error(e),
    };
    // The page includes `_spool_detail_card.html`, which references
    // `locations` for its reassign `<select>` — fetch it the same way
    // `render_card` does for the op-handler render path.
    let locations = match location_options(&st).await {
        Ok(ls) => ls,
        Err(resp) => return resp,
    };
    let low_stock_threshold_pct = match st.instance_configuration.get().await {
        Ok(configuration) => configuration.low_stock_threshold.percent(),
        Err(e) => return internal_error(e),
    };

    let view = SpoolDetailView::localized(detail, &st.renderer, &locale);
    let mut ctx = Context::new();
    ctx.insert("spool", &view);
    ctx.insert("locations", &locations);
    ctx.insert("page", "spools");
    ctx.insert("low_stock_threshold_pct", &low_stock_threshold_pct);
    ctx.insert("editing_remaining", &false);
    ctx.insert("is_fragment", &false);
    ctx.insert("nav_spool_count", &st.nav_spool_count().await);
    ctx.insert("nav_printer_count", &st.nav_printer_count().await);
    // The page includes `_spool_detail_card.html`, which always references
    // `error_key` (to show/hide its op-error line) — insert `None` here so
    // that include doesn't hit an undefined variable on a fresh page load.
    ctx.insert("error_key", &Option::<&str>::None);
    match st
        .renderer
        .render("spools_detail.html", &locale, theme.data_attr(), ctx)
    {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal_error(e),
    }
}

/// Renders the autonomous detail fragment (`_spool_detail_card.html`) — hero,
/// information cards and stock operations — both after htmx mutations and as
/// an include in the full page. `error_key`, when set, is shown as a localized
/// operation error inside the fragment.
async fn render_card(
    st: &AppState,
    locale: &str,
    status: StatusCode,
    detail: SpoolDetail,
    error_key: Option<&str>,
    editing_remaining: bool,
) -> Response {
    // The card is re-rendered by every op handler (weight/consume/archive/
    // restore/reassign), not just the reassign one — so it fetches the
    // locations list itself rather than relying on a caller to have done
    // so, keeping every render path consistent.
    let locations = match location_options(st).await {
        Ok(ls) => ls,
        Err(resp) => return resp,
    };
    let low_stock_threshold_pct = match st.instance_configuration.get().await {
        Ok(configuration) => configuration.low_stock_threshold.percent(),
        Err(e) => return internal_error(e),
    };
    let view = SpoolDetailView::localized(detail, &st.renderer, locale);
    let mut ctx = Context::new();
    ctx.insert("spool", &view);
    ctx.insert("locations", &locations);
    ctx.insert("error_key", &error_key);
    ctx.insert("low_stock_threshold_pct", &low_stock_threshold_pct);
    ctx.insert("editing_remaining", &editing_remaining);
    ctx.insert("is_fragment", &true);
    match st
        .renderer
        .render("_spool_detail_card.html", locale, "", ctx)
    {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => internal_error(e),
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
        Ok(detail) => render_card(st, locale, status, detail, error_key, false).await,
        Err(RepositoryError::NotFound(_)) => not_found(st, locale),
        Err(e) => internal_error(e),
    }
}

/// Returns the detail fragment in its normal display state. It is used by
/// the inline remaining-weight form's cancel action, so cancelling never
/// navigates away from the detail screen.
async fn detail_card(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    render_card_for(&st, &locale, SpoolId::new(id), StatusCode::OK, None).await
}

/// Opens the hero's inline remaining-weight editor. The response is the
/// same autonomous fragment returned by mutations, with only its editor
/// state changed.
async fn edit_remaining(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    match st.spools.view(SpoolId::new(id)).await {
        Ok(detail) => render_card(&st, &locale, StatusCode::OK, detail, None, true).await,
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(e) => internal_error(e),
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
/// `NotFound` 404s, anything else is a generic 500 via `internal_error`
/// (detail logged server-side, never sent to the client).
async fn finish_op(
    st: &AppState,
    locale: &str,
    id: SpoolId,
    result: Result<Spool, RepositoryError>,
) -> Response {
    match result {
        Ok(spool) => {
            if matches!(spool.status, SpoolStatus::Empty | SpoolStatus::Archived)
                && let Err(e) = st.printers.unload_spool(id.clone()).await
            {
                return internal_error(e);
            }
            render_card_for(st, locale, id, StatusCode::OK, None).await
        }
        Err(RepositoryError::NotFound(_)) => not_found(st, locale),
        Err(RepositoryError::Domain(e)) => {
            let key = op_error_key(&e);
            render_card_for(st, locale, id, StatusCode::UNPROCESSABLE_ENTITY, Some(key)).await
        }
        Err(e) => internal_error(e),
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
    error_key: Option<&'a str>,
    /// The full page (extends `base.html`) on a fresh `GET
    /// /spools/{id}/edit`, or just the shared wizard fragment when
    /// re-rendering a failed `PUT /spools/{id}` in place.
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
    let locations = match location_options(st).await {
        Ok(ls) => ls,
        Err(resp) => return resp,
    };
    let manufacturers = match manufacturer_options(st).await {
        Ok(ms) => ms,
        Err(resp) => return resp,
    };
    let mut ctx = Context::new();
    ctx.insert("id", opts.id);
    ctx.insert("materials", &materials);
    ctx.insert("locations", &locations);
    ctx.insert("manufacturers", &manufacturers);
    ctx.insert("condition", &form.condition);
    ctx.insert("material_id", &form.material_id);
    ctx.insert("location_id", &form.location_id);
    ctx.insert("manufacturer_id", &form.manufacturer_id);
    ctx.insert("manufacturer_name", &form.manufacturer_name);
    ctx.insert("colour_hex", &form.colour_hex);
    let derived_colour_name = Colour::from_hex(form.colour_hex.clone())
        .ok()
        .and_then(|colour| colour.name().map(str::to_string))
        .unwrap_or_default();
    ctx.insert("colour_name", &derived_colour_name);
    ctx.insert("diameter", &form.diameter);
    ctx.insert("net_weight", &form.net_weight);
    ctx.insert("remaining_weight", &form.remaining_weight);
    ctx.insert("price_paid", &form.price_paid);
    ctx.insert("notes", &form.notes);
    ctx.insert("purchased_at", &form.purchased_at);
    ctx.insert("opened_at", &form.opened_at);
    ctx.insert("error_key", &opts.error_key);
    ctx.insert("wizard_step", "details");
    ctx.insert("edit_mode", &true);
    ctx.insert(
        "net_weight_is_custom",
        &net_weight_is_custom(&form.net_weight),
    );
    if opts.full_page {
        ctx.insert("nav_spool_count", &st.nav_spool_count().await);
        ctx.insert("nav_printer_count", &st.nav_printer_count().await);
    }
    let template = if opts.full_page {
        "spools_edit.html"
    } else {
        "_spool_wizard_details.html"
    };
    match st.renderer.render(template, locale, theme_attr, ctx) {
        Ok(html) => (opts.status, Html(html)).into_response(),
        Err(e) => internal_error(e),
    }
}

fn edit_form(detail: &SpoolDetail) -> SpoolForm {
    SpoolForm {
        condition: if detail.status == SpoolStatus::Open {
            "opened".to_string()
        } else {
            "new".to_string()
        },
        material_id: detail.material_id.as_str().to_string(),
        colour_hex: detail
            .colour
            .as_ref()
            .map(|colour| colour.hex().to_string())
            .unwrap_or_default(),
        colour_name: detail
            .colour
            .as_ref()
            .and_then(Colour::name)
            .unwrap_or("")
            .to_string(),
        diameter: detail.diameter.as_str().to_string(),
        net_weight: detail.net_weight.value().to_string(),
        price_paid: detail.price_paid.to_string(),
        location_id: detail.location_id.clone().unwrap_or_default(),
        manufacturer_id: detail.manufacturer_id.clone().unwrap_or_default(),
        manufacturer_name: String::new(),
        remaining_weight: detail.remaining_weight.value().to_string(),
        notes: detail.notes.clone().unwrap_or_default(),
        purchased_at: detail
            .purchased_at
            .map(|date| date.to_string())
            .unwrap_or_default(),
        opened_at: detail
            .opened_at
            .map(|date| date.to_string())
            .unwrap_or_default(),
    }
}

fn render_edit_condition(
    st: &AppState,
    locale: &str,
    theme_attr: &str,
    id: &str,
    full_page: bool,
) -> Response {
    let mut ctx = Context::new();
    ctx.insert("page", "spools");
    ctx.insert("wizard_step", "condition");
    ctx.insert("edit_mode", &true);
    ctx.insert("id", id);
    let template = if full_page {
        "spools_edit.html"
    } else {
        "_spool_wizard_condition.html"
    };
    match st.renderer.render(template, locale, theme_attr, ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal_error(e),
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
        Err(e) => return internal_error(e),
    };

    let form = edit_form(&detail);
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

async fn edit_condition_step(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    match st.spools.view(SpoolId::new(id.clone())).await {
        Ok(_) => render_edit_condition(&st, &locale, "", &id, false),
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(e) => internal_error(e),
    }
}

async fn edit_details_step(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ConditionQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let condition = match SpoolCondition::parse(&query.condition) {
        Ok(SpoolCondition::New | SpoolCondition::Opened) => query.condition,
        _ => return render_edit_condition(&st, &locale, "", &id, false),
    };
    let detail = match st.spools.view(SpoolId::new(id.clone())).await {
        Ok(detail) => detail,
        Err(RepositoryError::NotFound(_)) => return not_found(&st, &locale),
        Err(e) => return internal_error(e),
    };
    let mut form = edit_form(&detail);
    form.condition = condition;
    render_edit_form(
        &st,
        &locale,
        "",
        &form,
        EditFormRender {
            status: StatusCode::OK,
            id: &id,
            error_key: None,
            full_page: false,
        },
    )
    .await
}

/// `PUT /spools/{id}`, issued by the edit form's own `hx-put` (a genuine
/// HTTP PUT via htmx's JS, same mechanism the materials table's inline
/// edits use — no method-override hack). Delegates all status/remaining
/// derivation to the edit use case.
async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<SpoolForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let spool_id = SpoolId::new(id.clone());

    let mut spool = match form.to_edit(spool_id) {
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

    spool.manufacturer_id = match resolve_manufacturer_id(&st, &form).await {
        Ok(id) => id,
        Err(ManufacturerResolutionError::InvalidName) => {
            return render_edit_form(
                &st,
                &locale,
                theme.data_attr(),
                &form,
                EditFormRender {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    id: &id,
                    error_key: Some("spools.new.error.manufacturer"),
                    full_page: false,
                },
            )
            .await;
        }
        Err(ManufacturerResolutionError::Response(response)) => return response,
    };

    match st.spools.edit(spool).await {
        // htmx's `HX-Redirect` header triggers a full client-side navigation
        // (`window.location`) rather than swapping this response into the
        // form's target — success leaves the edit form for the spool detail.
        Ok(updated) => {
            if updated.status == SpoolStatus::Empty
                && let Err(e) = st.printers.unload_spool(updated.id).await
            {
                return internal_error(e);
            }
            Response::builder()
                .status(StatusCode::OK)
                .header("HX-Redirect", format!("/spools/{id}"))
                .body(Body::empty())
                .unwrap()
        }
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
        // Same defensive-404 rationale as `create`'s `UnknownLocation` arm.
        Err(RepositoryError::UnknownLocation(_)) => not_found(&st, &locale),
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(e) => internal_error(e),
    }
}

/// The `POST /spools/{id}/location` form payload — the detail card's
/// reassign control. Blank `location_id` (the leading empty `<option>`) ⇒
/// unassign, same convention as `SpoolForm::location_id`.
#[derive(Debug, Deserialize, Default)]
pub struct LocationAssignForm {
    #[serde(default)]
    pub location_id: String,
}

/// `POST /spools/{id}/location` — reassigns (or, on a blank submission,
/// unassigns) the spool's Location via `SpoolsUseCases::assign_location`
/// (load -> mutate -> save happens inside the use case; this handler never
/// reloads-then-mutates itself). Re-renders the detail-card fragment on
/// success, same `outerHTML` swap pattern as the weight/archive ops.
/// Both an unknown spool id and an unknown location id (defensive — ids
/// come from rendered selects) 404, never a 500 or a "wrong slice" error.
async fn reassign_location(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<LocationAssignForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let spool_id = SpoolId::new(id);
    let location_id = parse_location_id(&form.location_id);
    match st
        .spools
        .assign_location(spool_id.clone(), location_id)
        .await
    {
        Ok(_) => render_card_for(&st, &locale, spool_id, StatusCode::OK, None).await,
        Err(RepositoryError::NotFound(_)) => not_found(&st, &locale),
        Err(RepositoryError::UnknownLocation(_)) => not_found(&st, &locale),
        Err(e) => internal_error(e),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/spools", get(list_page).post(create))
        .route("/spools/rows", get(rows))
        .route("/spools/new", get(new_page))
        .route("/spools/new/condition", get(condition_step))
        .route("/spools/new/details", get(details_step))
        .route("/spools/{id}/edit", get(edit_page))
        .route("/spools/{id}/edit/condition", get(edit_condition_step))
        .route("/spools/{id}/edit/details", get(edit_details_step))
        .route("/spools/{id}", get(detail_page).put(update))
        .route("/spools/{id}/card", get(detail_card))
        .route("/spools/{id}/remaining/edit", get(edit_remaining))
        .route(
            "/spools/{id}/remaining",
            post(set_remaining).put(set_remaining),
        )
        .route("/spools/{id}/consume", post(consume))
        .route("/spools/{id}/archive", post(archive))
        .route("/spools/{id}/restore", post(restore))
        .route("/spools/{id}/location", post(reassign_location))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;
    use domain::spools::SpoolType;

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
            location_name: Some("Drybox 1".into()),
            manufacturer_name: Some("Prusament".into()),
        }
    }

    fn material_option() -> MaterialOption {
        MaterialOption {
            id: "01HMAT".into(),
            name: "PLA".into(),
        }
    }

    fn location_option() -> LocationOption {
        LocationOption {
            id: "01HLOC".into(),
            name: "Shelf A".into(),
        }
    }

    fn manufacturer_option() -> ManufacturerOption {
        ManufacturerOption {
            id: "01HMFR".into(),
            name: "Prusament".into(),
        }
    }

    /// Inserts the count context every list/rows render needs, so individual
    /// tests only add what they assert on.
    fn insert_counts(ctx: &mut Context) {
        ctx.insert("filtered_count", &1usize);
        ctx.insert("total_count", &1usize);
        ctx.insert("low_stock_threshold_pct", &15u8);
    }

    fn render_list(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Sealed")]);
        ctx.insert("materials", &vec![material_option()]);
        ctx.insert("manufacturers", &vec![manufacturer_option()]);
        ctx.insert("locations", &vec![location_option()]);
        ctx.insert("selected_material", "");
        ctx.insert("selected_status", "");
        ctx.insert("selected_manufacturer", "");
        ctx.insert("selected_location", "");
        ctx.insert("search", "");
        ctx.insert("selected_sort", "created_desc");
        ctx.insert("stock_value", "37.50");
        insert_counts(&mut ctx);
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
    fn query_maps_search_manufacturer_and_location_facets() {
        let q = SpoolQuery {
            manufacturer_id: Some("01HMFR".into()),
            location_id: Some("01HLOC".into()),
            search: Some("  magenta  ".into()),
            ..Default::default()
        };
        let filter = q.to_filter();
        assert_eq!(filter.manufacturer_id, Some(ManufacturerId::new("01HMFR")));
        assert_eq!(filter.location_id, Some(LocationId::new("01HLOC")));
        assert_eq!(filter.search.as_deref(), Some("magenta")); // trimmed
    }

    #[test]
    fn query_blank_search_maps_to_none() {
        let q = SpoolQuery {
            search: Some("   ".into()),
            ..Default::default()
        };
        assert_eq!(q.to_filter().search, None);
    }

    #[test]
    fn list_page_renders_new_filters_and_counter() {
        let html = render_list("fr");
        assert!(html.contains(r#"name="search""#));
        assert!(html.contains(r#"name="manufacturer_id""#));
        assert!(html.contains(r#"name="location_id""#));
        assert!(html.contains("Prusament")); // manufacturer option
        assert!(html.contains("Shelf A")); // location option
        assert!(html.contains(r#"id="spools-filtered-count""#));
        assert!(html.contains("affichées")); // counter suffix (fr)
        assert!(!html.contains("spools.count.")); // no raw key leak
        assert!(!html.contains("spools.filter.")); // no raw key leak
    }

    #[test]
    fn list_page_keeps_search_visible_and_collapses_advanced_filters_by_default() {
        let html = render_list("en");
        let search = html.find(r#"id="spool-search""#).unwrap();
        let details = html
            .find(r#"<details class="spools-advanced-filters">"#)
            .unwrap();
        let details_end = html[details..].find("</details>").unwrap() + details;
        let material = html.find(r#"name="material_id""#).unwrap();

        assert!(search < details); // search stays outside the collapsed panel
        assert!(details < material && material < details_end);
        assert!(html.contains("Advanced filters"));
        assert!(!html.contains(r#"<details class="spools-advanced-filters" open"#));
    }

    #[test]
    fn list_page_renders_card_view_alongside_table() {
        let html = render_list("en");
        // Both views are rendered; a CSS radio toggle picks which is visible.
        assert!(html.contains(r#"id="spool-cards""#));
        assert!(html.contains("spool-card--sealed"));
        assert!(html.contains("Drybox 1")); // location shown as card storage
        assert!(html.contains(r#"id="spools-view-cards""#)); // view toggle radio
    }

    #[test]
    fn rows_fragment_updates_cards_out_of_band() {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spools", &vec![view("01HSP", "Open")]);
        ctx.insert("stock_value", "37.50");
        insert_counts(&mut ctx);
        let html = r.render("_spool_rows.html", "en", "", ctx).unwrap();
        // The card grid is re-rendered out-of-band so filtering keeps it in sync.
        assert!(html.contains(r#"id="spool-cards" hx-swap-oob="innerHTML""#));
        assert!(html.contains("spool-card--open"));
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
        insert_counts(&mut ctx);
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
        insert_counts(&mut ctx);
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
            total_length_m: 335.2,
            price_paid: "24.99".into(),
            current_value: "19.99".into(),
            status: "Open".into(),
            location_name: Some("Shelf A".into()),
            location_id: Some("01HLOC".into()),
            manufacturer_name: Some("Prusament".into()),
            notes: None,
            purchased_at: None,
            opened_at: None,
        }
    }

    fn render_detail(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spool", &detail_view("01HSP"));
        ctx.insert("locations", &vec![location_option()]);
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
        assert!(html.contains("24.99"));
        assert!(html.contains("19.99")); // current value at 80% remaining
        assert!(html.contains("335.2")); // total filament length
        assert!(html.contains("/spools/01HSP/edit"));
        assert!(html.contains(r#"class="spool-detail-hero""#));
        assert!(html.contains(r#"class="spool-detail-info-grid""#));
        assert!(html.contains("Consumption history"));
        assert!(html.contains("No notes for this spool."));
        assert!(html.contains(r#"hx-post="/spools/01HSP/archive""#));
        assert!(html.contains(r#"hx-get="/spools/01HSP/remaining/edit""#));
        assert!(html.contains("Shelf A")); // assigned location name shown
        assert!(html.contains("Prusament")); // manufacturer name shown
        assert!(!html.contains("spools.col."));
        assert!(!html.contains("spools.detail."));
        assert!(!html.contains("spools.status."));
        assert!(!html.contains("spools.location."));
    }

    #[test]
    fn detail_page_localises_to_french() {
        let html = render_detail("fr");
        assert!(html.contains("Bobines"));
        assert!(html.contains("Valeur actuelle"));
        assert!(html.contains("Historique de consommation"));
        assert!(html.contains("Aucune note pour cette bobine."));
        assert!(!html.contains("spools.col."));
        assert!(!html.contains("spools.detail."));
    }

    #[test]
    fn detail_page_renders_notes_and_dates_when_present() {
        let r = Renderer::new(Catalog::load("en"));
        let mut spool = detail_view("01HSP");
        spool.notes = Some("Opened for a prototype".into());
        spool.purchased_at = Some("2026-07-01".into());
        spool.opened_at = Some("2026-07-12".into());
        let mut ctx = Context::new();
        ctx.insert("spool", &spool);
        ctx.insert("locations", &vec![location_option()]);
        let html = r.render("spools_detail.html", "en", "", ctx).unwrap();
        assert!(html.contains("Opened for a prototype"));
        assert!(html.contains("2026-07-01"));
        assert!(html.contains("2026-07-12"));
        assert!(!html.contains("No notes for this spool."));
    }

    #[test]
    fn detail_fragment_renders_inline_weight_editor_with_put_and_cancel_get() {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("spool", &detail_view("01HSP"));
        ctx.insert("locations", &vec![location_option()]);
        ctx.insert("error_key", &Option::<&str>::None);
        ctx.insert("editing_remaining", &true);
        ctx.insert("is_fragment", &true);
        ctx.insert("low_stock_threshold_pct", &15u8);
        let html = r.render("_spool_detail_card.html", "en", "", ctx).unwrap();

        assert!(html.contains(r#"hx-put="/spools/01HSP/remaining""#));
        assert!(html.contains(r#"hx-get="/spools/01HSP/card""#));
        assert!(html.contains(r#"value="800.0""#));
        assert!(!html.contains("Adjust weight"));
        assert!(!html.contains("spools."));
    }

    #[test]
    fn detail_fragment_applies_configured_low_stock_gauge_state() {
        let r = Renderer::new(Catalog::load("en"));
        let mut spool = detail_view("01HSP");
        spool.remaining_pct = 12;
        let mut ctx = Context::new();
        ctx.insert("spool", &spool);
        ctx.insert("locations", &Vec::<LocationOption>::new());
        ctx.insert("error_key", &Option::<&str>::None);
        ctx.insert("editing_remaining", &false);
        ctx.insert("is_fragment", &false);
        ctx.insert("low_stock_threshold_pct", &15u8);
        let html = r.render("_spool_detail_card.html", "en", "", ctx).unwrap();

        assert!(html.contains("spool-detail-gauge gauge--low"));
        assert!(html.contains("detail-weight--low"));
    }

    #[test]
    fn query_maps_status_and_material_filter() {
        let q = SpoolQuery {
            material_id: Some("01HMAT".into()),
            status: Some("Open".into()),
            sort: Some("remaining_asc".into()),
            ..Default::default()
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
        ctx.insert("locations", &vec![location_option()]);
        ctx.insert("manufacturers", &vec![manufacturer_option()]);
        ctx.insert("wizard_step", "details");
        ctx.insert("condition", "new");
        ctx.insert("material_id", "");
        ctx.insert("location_id", "");
        ctx.insert("manufacturer_id", "");
        ctx.insert("manufacturer_name", "");
        ctx.insert("colour_hex", "");
        ctx.insert("colour_name", "");
        ctx.insert("diameter", "1.75");
        ctx.insert("net_weight", "");
        ctx.insert("remaining_weight", "");
        ctx.insert("price_paid", "");
        ctx.insert("notes", "");
        ctx.insert("purchased_at", "");
        ctx.insert("opened_at", "");
        ctx.insert("error_key", &Option::<&str>::None);
        r.render("spools_new.html", locale, "", ctx).unwrap()
    }

    fn render_edit_form_template(locale: &str) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("id", "01HSP");
        ctx.insert("materials", &vec![material_option()]);
        ctx.insert("locations", &vec![location_option()]);
        ctx.insert("manufacturers", &vec![manufacturer_option()]);
        ctx.insert("wizard_step", "details");
        ctx.insert("edit_mode", &true);
        ctx.insert("condition", "opened");
        ctx.insert("material_id", "01HMAT");
        ctx.insert("location_id", "01HLOC");
        ctx.insert("manufacturer_id", "01HMFR");
        ctx.insert("manufacturer_name", "");
        ctx.insert("colour_hex", "#C62828");
        ctx.insert("colour_name", "Red");
        ctx.insert("diameter", "2.85");
        ctx.insert("net_weight", "1234.5");
        ctx.insert("net_weight_is_custom", &true);
        ctx.insert("remaining_weight", "456.7");
        ctx.insert("price_paid", "31.20");
        ctx.insert("notes", "Opened for a prototype");
        ctx.insert("purchased_at", "2026-07-01");
        ctx.insert("opened_at", "2026-07-12");
        ctx.insert("error_key", &Option::<&str>::None);
        r.render("spools_edit.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn new_form_lists_material_option_no_raw_keys() {
        let html = render_new_form("en");
        assert!(html.contains("PLA"));
        assert!(html.contains(r#"value="01HMAT""#));
        assert!(!html.contains("spools.new.")); // no raw i18n key leaks
    }

    #[test]
    fn new_form_lists_location_option_and_blank_unassigned_choice() {
        let html = render_new_form("en");
        assert!(html.contains("Shelf A"));
        assert!(html.contains(r#"value="01HLOC""#));
        assert!(html.contains(r#"<option value="">"#)); // blank = unassigned
        assert!(!html.contains("spools.new.")); // no raw i18n key leaks
        assert!(!html.contains("spools.location."));
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
        ctx.insert("locations", &vec![location_option()]);
        ctx.insert("manufacturers", &vec![manufacturer_option()]);
        ctx.insert("wizard_step", "details");
        ctx.insert("condition", "new");
        ctx.insert("material_id", "");
        ctx.insert("location_id", "");
        ctx.insert("manufacturer_id", "");
        ctx.insert("manufacturer_name", "");
        ctx.insert("colour_hex", "not-a-hex");
        ctx.insert("colour_name", "");
        ctx.insert("diameter", "1.75");
        ctx.insert("net_weight", "1000");
        ctx.insert("remaining_weight", "");
        ctx.insert("price_paid", "24.99");
        ctx.insert("notes", "");
        ctx.insert("purchased_at", "");
        ctx.insert("opened_at", "");
        ctx.insert("error_key", &Some("spools.new.error.colour"));
        let html = r.render("spools_new.html", "en", "", ctx).unwrap();
        assert!(html.contains("must be a valid #RRGGBB hex code"));
        assert!(html.contains(r#"value="not-a-hex""#)); // submitted value echoed back
    }

    #[test]
    fn edit_form_reuses_wizard_details_and_prefills_all_supported_fields() {
        let html = render_edit_form_template("en");
        assert!(html.contains(r#"class="spool-wizard-form""#));
        assert!(html.contains(r#"hx-put="/spools/01HSP""#));
        assert!(html.contains(r#"value="opened""#));
        assert!(html.contains(r#"value="01HMAT" selected"#));
        assert!(html.contains(r#"value="01HMFR" selected"#));
        assert!(html.contains(r#"value="01HLOC" selected"#));
        assert!(html.contains(r##"value="#C62828""##));
        assert!(html.contains(r#"value="2.85" checked"#));
        assert!(html.contains(r#"value="custom" selected"#));
        assert!(html.contains(r#"value="1234.5""#));
        assert!(html.contains(r#"name="remaining_weight""#));
        assert!(html.contains(r#"value="456.7""#));
        assert!(html.contains(r#"value="31.20""#));
        assert!(html.contains(r#"name="purchased_at" value="2026-07-01""#));
        assert!(html.contains(r#"name="opened_at" value="2026-07-12""#));
        assert!(
            html.contains(r#"<textarea name="notes" rows="4">Opened for a prototype</textarea>"#)
        );
        assert!(html.contains("/spools/01HSP/edit/condition"));
        assert!(!html.contains("spools.edit."));
        assert!(!html.contains("spools.colour."));
    }

    #[test]
    fn edit_form_renders_in_non_default_french_locale() {
        let html = render_edit_form_template("fr");
        assert!(html.contains("Modifier la bobine"));
        assert!(html.contains("Entamée"));
        assert!(html.contains("Enregistrer les modifications"));
        assert!(html.contains("Poids restant"));
        assert!(!html.contains("spools.edit."));
        assert!(!html.contains("spools.condition."));
    }

    fn valid_form(material_id: &str) -> SpoolForm {
        SpoolForm {
            condition: "new".to_string(),
            material_id: material_id.to_string(),
            colour_hex: "#1A9E4B".to_string(),
            colour_name: "vert sapin".to_string(),
            diameter: "1.75".to_string(),
            net_weight: "1000".to_string(),
            price_paid: "24.99".to_string(),
            location_id: "".to_string(),
            manufacturer_id: "".to_string(),
            manufacturer_name: "".to_string(),
            remaining_weight: "".to_string(),
            notes: "".to_string(),
            purchased_at: "".to_string(),
            opened_at: "".to_string(),
        }
    }

    /// A `NewSpool` matching `valid_form`'s values — used by the handler
    /// tests below to seed a stub-backed spool via `SpoolsUseCases::add`.
    fn valid_new_spool(material_id: &str) -> NewSpool {
        valid_form(material_id).to_new().unwrap()
    }

    #[test]
    fn to_new_maps_valid_form_to_domain_values() {
        let mut form = valid_form("01HMAT");
        form.notes = "  Keep dry  ".into();
        form.purchased_at = "2026-07-01".into();
        form.opened_at = "2026-07-12".into();
        let new = form.to_new().unwrap();
        assert_eq!(new.material_id, MaterialId::new("01HMAT"));
        assert_eq!(new.colour.as_ref().unwrap().hex(), "#1A9E4B");
        assert_eq!(new.colour.as_ref().unwrap().name(), Some("green"));
        assert_eq!(new.diameter, Diameter::Mm1_75);
        assert_eq!(new.net_weight.value(), 1000.0);
        assert_eq!(
            new.price_paid,
            Money::from_decimal(Decimal::from_str_exact("24.99").unwrap()).unwrap()
        );
        assert_eq!(new.notes.as_deref(), Some("Keep dry"));
        assert_eq!(new.purchased_at.unwrap().to_string(), "2026-07-01");
        assert_eq!(new.opened_at.unwrap().to_string(), "2026-07-12");
    }

    #[test]
    fn to_new_rejects_invalid_optional_date() {
        let mut form = valid_form("01HMAT");
        form.purchased_at = "2026-02-30".into();
        assert_eq!(form.to_new(), Err("spools.new.error.date"));
    }

    #[test]
    fn to_new_derives_opened_and_refill_initial_values() {
        let mut opened = valid_form("01HMAT");
        opened.condition = "opened".into();
        opened.remaining_weight = "1200".into();
        let opened = opened.to_new().unwrap();
        assert_eq!(opened.initial_status(), SpoolStatus::Open);
        assert_eq!(opened.initial_remaining_weight().value(), 1000.0);
        assert_eq!(opened.spool_type(), SpoolType::Complete);

        let mut refill = valid_form("01HMAT");
        refill.condition = "refill".into();
        let refill = refill.to_new().unwrap();
        assert_eq!(refill.initial_status(), SpoolStatus::Sealed);
        assert_eq!(refill.spool_type(), SpoolType::Recharge);
    }

    #[test]
    fn to_new_accepts_an_unset_colour() {
        let mut form = valid_form("01HMAT");
        form.colour_hex.clear();
        assert_eq!(form.to_new().unwrap().colour, None);
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

    #[test]
    fn to_new_maps_selected_location_to_some() {
        let mut f = valid_form("01HMAT");
        f.location_id = "01HLOC".to_string();
        let new = f.to_new().unwrap();
        assert_eq!(new.location_id, Some(LocationId::new("01HLOC")));
    }

    #[test]
    fn to_new_maps_blank_location_to_none() {
        let mut f = valid_form("01HMAT");
        f.location_id = "".to_string();
        let new = f.to_new().unwrap();
        assert_eq!(new.location_id, None);
    }

    #[test]
    fn to_new_maps_whitespace_location_to_none() {
        let mut f = valid_form("01HMAT");
        f.location_id = "   ".to_string();
        let new = f.to_new().unwrap();
        assert_eq!(new.location_id, None);
    }

    #[test]
    fn to_edit_maps_selected_location_to_some() {
        let mut f = valid_form("01HMAT");
        f.location_id = "01HLOC".to_string();
        let spool = f.to_edit(SpoolId::new("01HSP")).unwrap();
        assert_eq!(spool.location_id, Some(LocationId::new("01HLOC")));
    }

    #[test]
    fn to_edit_maps_blank_location_to_none() {
        let mut f = valid_form("01HMAT");
        f.location_id = "".to_string();
        let spool = f.to_edit(SpoolId::new("01HSP")).unwrap();
        assert_eq!(spool.location_id, None);
    }

    #[test]
    fn to_edit_normalizes_custom_colour_and_accepts_no_colour() {
        let mut form = valid_form("01HMAT");
        form.colour_hex = "abc".to_string();
        let edit = form.to_edit(SpoolId::new("01HSP")).unwrap();
        assert_eq!(edit.colour.unwrap().hex(), "#AABBCC");

        form.colour_hex.clear();
        assert_eq!(form.to_edit(SpoolId::new("01HSP")).unwrap().colour, None);
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
        use domain::dashboard::stubs::StubDashboardRepository;
        use domain::dashboard::{DashboardService, DashboardUseCases};
        use domain::locations::stubs::StubLocationRepository;
        use domain::locations::{LocationName, LocationsService, LocationsUseCases, NewLocation};
        use domain::materials::stubs::StubMaterialRepository;
        use domain::materials::{
            Density, DryingParams, MaterialName, MaterialRepository, MaterialsService,
            MaterialsUseCases, NewMaterial, Sensitivity, Temperature,
        };
        use domain::spools::stubs::StubSpoolRepository;
        use domain::spools::{SpoolRepository, SpoolsService, SpoolsUseCases};
        use sqlx::PgPool;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct NoopPrinters(Arc<AtomicUsize>);

        #[async_trait::async_trait]
        impl domain::printers::PrintersUseCases for NoopPrinters {
            async fn list(
                &self,
            ) -> Result<Vec<domain::printers::Printer>, domain::printers::RepositoryError>
            {
                Ok(vec![])
            }
            async fn add(
                &self,
                _: domain::printers::NewPrinter,
            ) -> Result<domain::printers::Printer, domain::printers::RepositoryError> {
                unreachable!()
            }
            async fn edit(
                &self,
                _: domain::printers::Printer,
            ) -> Result<domain::printers::Printer, domain::printers::RepositoryError> {
                unreachable!()
            }
            async fn delete(
                &self,
                _: domain::shared::PrinterId,
            ) -> Result<(), domain::printers::RepositoryError> {
                Ok(())
            }
            async fn load_slot(
                &self,
                _: domain::shared::PrinterId,
                _: String,
                _: SpoolId,
            ) -> Result<(), domain::printers::RepositoryError> {
                Ok(())
            }
            async fn unload_slot(
                &self,
                _: domain::shared::PrinterId,
                _: String,
            ) -> Result<(), domain::printers::RepositoryError> {
                Ok(())
            }
            async fn unload_spool(
                &self,
                _: SpoolId,
            ) -> Result<(), domain::printers::RepositoryError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            async fn loadable_spools(
                &self,
                _: Option<SpoolId>,
            ) -> Result<Vec<domain::printers::LoadableSpool>, domain::printers::RepositoryError>
            {
                Ok(vec![])
            }
        }

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

            let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(Arc::new(
                StubDashboardRepository::new(),
            )));

            let manufacturers: Arc<dyn domain::manufacturers::ManufacturersUseCases> =
                Arc::new(domain::manufacturers::ManufacturersService::new(Arc::new(
                    domain::manufacturers::stubs::StubManufacturerRepository::new(),
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
            let mut state = AppState::new(
                    db,
                    &cfg,
                    materials,
                    spools,
                    locations,
                    manufacturers,
                    dashboard,
                    Arc::new(
                        domain::instance_configuration::InstanceConfigurationService::new(
                            Arc::new(
                                domain::instance_configuration::stubs::StubInstanceConfigurationRepository::new(),
                            ),
                        ),
                    ),
                    Arc::new(domain::instance_transfer::InstanceTransferService::new(
                        Arc::new(
                            domain::instance_transfer::stubs::StubInstanceTransferRepository::default(),
                        ),
                    )),
                );
            state.printers = Arc::new(NoopPrinters(Arc::new(AtomicUsize::new(0))));
            (state, seeded.id.as_str().to_string())
        }

        async fn body_of(res: Response) -> String {
            let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        }

        /// Seeds a Location via the real use case (not a bespoke stub
        /// bypass), returning its id — used by the location-assignment
        /// handler tests below to get a real, list-able option.
        async fn seed_location(st: &AppState, name: &str) -> String {
            st.locations
                .add(NewLocation {
                    name: LocationName::new(name).unwrap(),
                    note: None,
                })
                .await
                .unwrap()
                .id
                .as_str()
                .to_string()
        }

        #[tokio::test]
        async fn get_new_renders_form_with_material_option() {
            let (st, _material_id) = test_state().await;
            let res = new_page(State(st), HeaderMap::new()).await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("/spools/new/details?condition=new"));
            assert!(html.contains("/spools/new/details?condition=opened"));
            assert!(html.contains("/spools/new/details?condition=refill"));
        }

        #[tokio::test]
        async fn details_fragment_keeps_active_french_locale_and_condition_fields() {
            let (st, _) = test_state().await;
            let mut headers = HeaderMap::new();
            headers.insert("cookie", "lang=fr".parse().unwrap());
            let opened = details_step(
                State(st.clone()),
                headers.clone(),
                Query(ConditionQuery {
                    condition: "opened".into(),
                }),
            )
            .await;
            let html = body_of(opened).await;
            assert!(html.contains("Entamée"));
            assert!(html.contains("Poids restant"));
            assert!(html.contains("Changer"));
            assert!(!html.contains("spools.condition."));

            let new = details_step(
                State(st),
                headers,
                Query(ConditionQuery {
                    condition: "new".into(),
                }),
            )
            .await;
            assert!(!body_of(new).await.contains("Poids restant"));
        }

        #[tokio::test]
        async fn post_valid_form_adds_spool_and_redirects() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let res = create(State(st), HeaderMap::new(), Form(valid_form(&material_id))).await;
            assert_eq!(res.status(), StatusCode::SEE_OTHER);
            assert!(
                res.headers()
                    .get("location")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("/spools/")
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
        async fn get_edit_prefills_opened_custom_weight_and_french_locale() {
            let (st, material_id) = test_state().await;
            let mut form = valid_form(&material_id);
            form.condition = "opened".to_string();
            form.net_weight = "1234.5".to_string();
            form.remaining_weight = "456.7".to_string();
            form.colour_hex = "abc".to_string();
            let created = st.spools.add(form.to_new().unwrap()).await.unwrap();
            let mut headers = HeaderMap::new();
            headers.insert("cookie", "lang=fr".parse().unwrap());

            let res = edit_page(State(st), headers, Path(created.id.as_str().to_string())).await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("Modifier la bobine"));
            assert!(html.contains("Entamée"));
            assert!(html.contains(r##"value="#AABBCC""##));
            assert!(html.contains(r#"value="custom" selected"#));
            assert!(html.contains(r#"value="1234.5""#));
            assert!(html.contains(r#"value="456.7""#));
            assert!(!html.contains("spools.edit."));
        }

        #[tokio::test]
        async fn edit_change_returns_status_choices_then_reloads_shared_details() {
            let (st, material_id) = test_state().await;
            let created = st.spools.add(valid_new_spool(&material_id)).await.unwrap();
            let id = created.id.as_str().to_string();

            let choices =
                edit_condition_step(State(st.clone()), HeaderMap::new(), Path(id.clone())).await;
            let html = body_of(choices).await;
            assert!(html.contains(&format!("/spools/{id}/edit/details?condition=opened")));
            assert!(!html.contains("condition=refill"));

            let details = edit_details_step(
                State(st),
                HeaderMap::new(),
                Path(id.clone()),
                Query(ConditionQuery {
                    condition: "opened".to_string(),
                }),
            )
            .await;
            let html = body_of(details).await;
            assert!(html.contains(r#"class="spool-wizard-form""#));
            assert!(html.contains(&format!(r#"hx-put="/spools/{id}""#)));
            assert!(html.contains(r#"name="remaining_weight""#));
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
        async fn put_opened_changes_colour_and_derives_remaining_and_status() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let mut form = valid_form(&material_id);
            form.colour_hex = "#00FF00".to_string();
            form.colour_name = "vert".to_string();
            form.condition = "opened".to_string();
            form.remaining_weight = "800".to_string();
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
                format!("/spools/{}", created.id.as_str())
            );

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.colour.as_ref().unwrap().hex(), "#00FF00");
            assert_eq!(detail.status, SpoolStatus::Open);
            assert_eq!(detail.remaining_weight.value(), 800.0);
            assert_eq!(detail.net_weight.value(), 1000.0);
        }

        #[tokio::test]
        async fn put_lowering_net_below_remaining_clamps_remaining() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let mut form = valid_form(&material_id);
            form.net_weight = "500".to_string();
            form.condition = "opened".to_string();
            form.remaining_weight = "800".to_string();
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
            assert_eq!(detail.colour.as_ref().unwrap().hex(), "#1A9E4B"); // unchanged
        }

        // --- Task 12: GET /spools/{id} (detail view).

        #[tokio::test]
        async fn get_detail_renders_spool_fields_and_derived_values() {
            let (st, material_id) = test_state().await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            // Draw the spool down so remaining ratio/length differ from net.
            spools
                .set_remaining(created.id.clone(), Grams::new(800.0).unwrap())
                .await
                .unwrap();

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
            assert!(html.contains("#1A9E4B")); // derived colour name
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
            let remaining_put = format!(r#"hx-put="/spools/{id}/remaining""#);
            let remaining_edit = format!(r#"hx-get="/spools/{id}/remaining/edit""#);
            let consume_post = format!(r#"hx-post="/spools/{id}/consume""#);

            let archived = archive(State(st.clone()), HeaderMap::new(), Path(id.clone())).await;
            assert_eq!(archived.status(), StatusCode::OK);
            let html = body_of(archived).await;
            assert!(html.contains(&restore_post)); // Restore control shown
            assert!(!html.contains(&remaining_put)); // weight forms hidden
            assert!(!html.contains(&remaining_edit));
            assert!(!html.contains(&consume_post));

            let restored = restore(State(st), HeaderMap::new(), Path(id)).await;
            assert_eq!(restored.status(), StatusCode::OK);
            let html = body_of(restored).await;
            assert!(html.contains(&remaining_edit)); // inline weight control shown again
            assert!(html.contains(&consume_post));
            assert!(!html.contains(&restore_post));
        }

        #[tokio::test]
        async fn empty_and_archive_handlers_orchestrate_printer_unload() {
            let (mut st, material_id) = test_state().await;
            let unloads = Arc::new(AtomicUsize::new(0));
            st.printers = Arc::new(NoopPrinters(unloads.clone()));

            let first = st.spools.add(valid_new_spool(&material_id)).await.unwrap();
            let response = set_remaining(
                State(st.clone()),
                HeaderMap::new(),
                Path(first.id.as_str().to_string()),
                Form(RemainingForm {
                    remaining: "0".into(),
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);

            let second = st.spools.add(valid_new_spool(&material_id)).await.unwrap();
            let response = archive(
                State(st),
                HeaderMap::new(),
                Path(second.id.as_str().to_string()),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(unloads.load(Ordering::SeqCst), 2);
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

        // --- Task 9: web location assignment (add/edit selects + detail
        // reassign).

        #[tokio::test]
        async fn post_valid_form_with_location_persists_location_id() {
            let (st, material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let spools = st.spools.clone();
            let mut form = valid_form(&material_id);
            form.location_id = location_id.clone();

            let res = create(State(st), HeaderMap::new(), Form(form)).await;
            assert_eq!(res.status(), StatusCode::SEE_OTHER);

            let created = spools
                .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
                .await
                .unwrap();
            assert_eq!(created.len(), 1);
            let detail = spools.view(created[0].id.clone()).await.unwrap();
            assert_eq!(detail.location_id, Some(location_id));
        }

        #[tokio::test]
        async fn post_valid_form_blank_location_persists_unassigned() {
            let (st, material_id) = test_state().await;
            seed_location(&st, "Shelf A").await; // a real location exists but isn't selected
            let spools = st.spools.clone();
            let form = valid_form(&material_id); // location_id: "" (default)

            let res = create(State(st), HeaderMap::new(), Form(form)).await;
            assert_eq!(res.status(), StatusCode::SEE_OTHER);

            let created = spools
                .list(SpoolFilter::default(), SpoolSort::CreatedDesc)
                .await
                .unwrap();
            let detail = spools.view(created[0].id.clone()).await.unwrap();
            assert_eq!(detail.location_id, None);
        }

        #[tokio::test]
        async fn get_new_lists_location_option() {
            let (st, _material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let res = details_step(
                State(st),
                HeaderMap::new(),
                Query(ConditionQuery {
                    condition: "new".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains("Shelf A"));
            assert!(html.contains(&format!(r#"value="{location_id}""#)));
        }

        #[tokio::test]
        async fn get_edit_prefills_location_from_stored_spool() {
            let (st, material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            spools
                .assign_location(
                    created.id.clone(),
                    Some(LocationId::new(location_id.clone())),
                )
                .await
                .unwrap();

            let res = edit_page(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(html.contains(&format!(r#"value="{location_id}" selected"#)));
        }

        #[tokio::test]
        async fn put_without_changing_location_preserves_it() {
            let (st, material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            spools
                .assign_location(
                    created.id.clone(),
                    Some(LocationId::new(location_id.clone())),
                )
                .await
                .unwrap();

            let mut form = valid_form(&material_id);
            form.location_id = location_id.clone(); // resubmit the same location
            let res = update(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(form),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.location_id, Some(location_id));
        }

        #[tokio::test]
        async fn put_blank_location_unassigns() {
            let (st, material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            spools
                .assign_location(
                    created.id.clone(),
                    Some(LocationId::new(location_id.clone())),
                )
                .await
                .unwrap();

            let form = valid_form(&material_id); // location_id: "" (default) => unassign
            let res = update(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(form),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.location_id, None);
        }

        #[tokio::test]
        async fn post_reassign_location_sets_location_and_returns_fragment() {
            let (st, material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();

            let res = reassign_location(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(LocationAssignForm {
                    location_id: location_id.clone(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            assert!(!html.contains("<html")); // fragment only, no page shell
            assert!(html.contains("Shelf A"));

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.location_id, Some(location_id));
        }

        #[tokio::test]
        async fn post_reassign_location_blank_unassigns_and_returns_fragment() {
            let (st, material_id) = test_state().await;
            let location_id = seed_location(&st, "Shelf A").await;
            let spools = st.spools.clone();
            let created = spools.add(valid_new_spool(&material_id)).await.unwrap();
            spools
                .assign_location(
                    created.id.clone(),
                    Some(LocationId::new(location_id.clone())),
                )
                .await
                .unwrap();

            let res = reassign_location(
                State(st),
                HeaderMap::new(),
                Path(created.id.as_str().to_string()),
                Form(LocationAssignForm {
                    location_id: "".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK);
            let html = body_of(res).await;
            // The location option is still listed (it still exists — a
            // spool just isn't assigned to it anymore), but must no longer
            // be the selected one, and the display falls back to the
            // "unassigned" label rather than the location's name.
            assert!(html.contains(&format!(r#"value="{location_id}""#)));
            assert!(!html.contains(&format!(r#"value="{location_id}" selected"#)));
            assert!(html.contains("Unassigned"));

            let detail = spools.view(created.id.clone()).await.unwrap();
            assert_eq!(detail.location_id, None);
        }

        #[tokio::test]
        async fn post_reassign_location_unknown_spool_id_returns_404() {
            let (st, _material_id) = test_state().await;
            let res = reassign_location(
                State(st),
                HeaderMap::new(),
                Path("does-not-exist".to_string()),
                Form(LocationAssignForm {
                    location_id: "".to_string(),
                }),
            )
            .await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
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
            assert!(html.contains("Ajuster le poids"));
            assert!(html.contains("Opérations de stock"));
            assert!(!html.contains("spools.")); // no raw i18n key leaks
        }
    }
}
