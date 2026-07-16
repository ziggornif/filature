use crate::web::router::{internal_error, resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Router,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use domain::printers::{
    BAMBU_MODELS, FeedMode, LoadableSpool, Module, NewPrinter, PRUSA_MODELS, Printer, PrinterBrand,
    PrinterName, RepositoryError,
};
use domain::shared::{PrinterId, SpoolId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tera::Context;

#[derive(Serialize)]
pub struct SlotGroupView {
    pub label_key: String,
    pub count: usize,
    pub multi: bool,
    pub slots: Vec<SlotView>,
}
#[derive(Serialize)]
pub struct SlotView {
    pub key: String,
    pub loaded: Option<LoadedSpoolView>,
    pub options: Vec<LoadableSpoolView>,
}
#[derive(Serialize)]
pub struct LoadedSpoolView {
    pub id: String,
    pub brand: String,
    pub colour_name: String,
    pub colour_hex: String,
    pub material_name: String,
    pub remaining_pct: u8,
    pub remaining_weight: f64,
    pub net_weight: f64,
    pub status_key: String,
}
#[derive(Serialize)]
pub struct LoadableSpoolView {
    pub id: String,
    pub label: String,
}
#[derive(Serialize)]
pub struct PrinterView {
    pub id: String,
    pub name: String,
    pub model: String,
    pub brand: String,
    pub liner: String,
    pub groups: Vec<SlotGroupView>,
    pub slot_summary: String,
}
impl PrinterView {
    async fn build(p: Printer, st: &AppState) -> Result<Self, RepositoryError> {
        let mut grouped: BTreeMap<String, Vec<SlotView>> = BTreeMap::new();
        let mut order = Vec::new();
        for slot in p.slots {
            if !grouped.contains_key(&slot.group_label) {
                order.push(slot.group_label.clone());
            }
            let current = slot.loaded_spool.as_ref().map(|s| s.id.clone());
            let options = st
                .printers
                .loadable_spools(current)
                .await?
                .into_iter()
                .map(LoadableSpoolView::from)
                .collect();
            let loaded = slot.loaded_spool.map(|s| {
                let remaining_pct = s.remaining_pct();
                LoadedSpoolView {
                    id: s.id.as_str().into(),
                    brand: s.manufacturer_name.unwrap_or_else(|| "—".into()),
                    colour_name: s.colour_name.unwrap_or_else(|| "—".into()),
                    colour_hex: s.colour_hex.unwrap_or_else(|| "transparent".into()),
                    material_name: s.material_name,
                    remaining_pct,
                    remaining_weight: s.remaining_weight,
                    net_weight: s.net_weight,
                    status_key: format!("spools.status.{}", s.status.to_lowercase()),
                }
            });
            grouped.entry(slot.group_label).or_default().push(SlotView {
                key: slot.key,
                loaded,
                options,
            });
        }
        let groups: Vec<_> = order
            .into_iter()
            .map(|label| {
                let slots = grouped.remove(&label).unwrap_or_default();
                SlotGroupView {
                    label_key: format!("printers.group.{label}"),
                    count: slots.len(),
                    multi: slots.len() > 1,
                    slots,
                }
            })
            .collect();
        let slot_summary = groups
            .iter()
            .map(|g| format!("{} ({})", g.label_key, g.slots.len()))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Self {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            model: p.model,
            brand: p.brand.as_str().into(),
            liner: p.brand.liner().into(),
            groups,
            slot_summary,
        })
    }
}

impl From<Printer> for PrinterView {
    fn from(p: Printer) -> Self {
        let mut grouped: BTreeMap<String, usize> = BTreeMap::new();
        let mut order = Vec::new();
        for slot in &p.slots {
            if !grouped.contains_key(&slot.group_label) {
                order.push(slot.group_label.clone());
            }
            *grouped.entry(slot.group_label.clone()).or_default() += 1;
        }
        let groups = order
            .into_iter()
            .map(|label| {
                let count = grouped[&label];
                SlotGroupView {
                    label_key: format!("printers.group.{label}"),
                    count,
                    multi: count > 1,
                    slots: Vec::new(),
                }
            })
            .collect();
        Self {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            model: p.model,
            brand: p.brand.as_str().into(),
            liner: p.brand.liner().into(),
            groups,
            slot_summary: String::new(),
        }
    }
}

impl From<LoadableSpool> for LoadableSpoolView {
    fn from(s: LoadableSpool) -> Self {
        let brand = s.manufacturer_name.unwrap_or_else(|| "—".into());
        let colour = s.colour_name.unwrap_or_else(|| "—".into());
        Self {
            id: s.id.as_str().into(),
            label: format!("{brand} · {colour} ({})", s.material_name),
        }
    }
}

#[derive(Default, Deserialize)]
struct FromQuery {
    #[serde(default)]
    from: String,
}
#[derive(Deserialize)]
pub struct PrinterForm {
    name: String,
    brand: String,
    model: String,
    #[serde(default = "default_heads")]
    heads: u8,
    module: String,
    #[serde(default)]
    module_count: Option<u8>,
    #[serde(default)]
    ams_units: u8,
    // Comma-separated per-head feed modes (e.g. "ams_fed,direct"). A single
    // string field is used because urlencoded form bodies cannot deserialize
    // repeated keys into a sequence.
    #[serde(default)]
    feed_modes: String,
    #[serde(default)]
    from: String,
}
fn default_heads() -> u8 {
    1
}
type PrinterDomain = (
    PrinterName,
    PrinterBrand,
    String,
    u8,
    Module,
    u8,
    Vec<FeedMode>,
);
impl PrinterForm {
    fn domain(&self) -> Result<PrinterDomain, String> {
        let name = PrinterName::new(&self.name).map_err(|e| e.to_string())?;
        let brand = PrinterBrand::parse(&self.brand).map_err(|e| e.to_string())?;
        let model = match brand {
            PrinterBrand::Other => {
                let m = self.model.trim();
                if m.is_empty() {
                    "Autre".into()
                } else {
                    m.into()
                }
            }
            _ => self.model.clone(),
        };
        if (brand == PrinterBrand::BambuLab && !BAMBU_MODELS.contains(&model.as_str()))
            || (brand == PrinterBrand::Prusa && !PRUSA_MODELS.contains(&model.as_str()))
        {
            return Err("invalid model".into());
        }
        let module = match self.module.as_str() {
            "none" => Module::None,
            "mmu" => Module::Mmu,
            "multi_slot" => Module::MultiSlot {
                slots: self.module_count.unwrap_or(4),
            },
            _ => return Err("invalid module".into()),
        };
        let module =
            Module::validate(brand, &model, self.heads, module).map_err(|e| e.to_string())?;
        let feed_modes = if brand == PrinterBrand::BambuLab {
            let parsed = self
                .feed_modes
                .split(',')
                .filter(|m| !m.is_empty())
                .map(|m| FeedMode::parse(m).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            if parsed.len() != usize::from(self.heads) {
                return Err("invalid feed modes".into());
            }
            parsed
        } else {
            vec![FeedMode::Direct; usize::from(self.heads)]
        };
        domain::printers::derive_slots(
            brand,
            &model,
            self.heads,
            &module,
            self.ams_units,
            &feed_modes,
        )
        .map_err(|e| e.to_string())?;
        Ok((
            name,
            brand,
            model,
            self.heads,
            module,
            self.ams_units,
            feed_modes,
        ))
    }
    fn destination(&self) -> &'static str {
        if self.from == "settings" {
            "/settings/printers"
        } else {
            "/printers"
        }
    }
}

async fn page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.printers.list().await {
        Ok(items) => {
            let mut views = Vec::with_capacity(items.len());
            for item in items {
                match PrinterView::build(item, &st).await {
                    Ok(view) => views.push(view),
                    Err(e) => return internal_error(e),
                }
            }
            let loaded_spools_count = views
                .iter()
                .flat_map(|p| &p.groups)
                .flat_map(|g| &g.slots)
                .filter(|s| s.loaded.is_some())
                .count();
            let mut ctx = Context::new();
            ctx.insert("page", "printers");
            ctx.insert("printers", &views);
            ctx.insert("printer_count", &views.len());
            ctx.insert("loaded_spools_count", &loaded_spools_count);
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &views.len());
            match st
                .renderer
                .render("printers.html", &locale, theme.data_attr(), ctx)
            {
                Ok(h) => Html(h).into_response(),
                Err(e) => internal_error(e),
            }
        }
        Err(e) => internal_error(e),
    }
}

async fn loading_fragment(st: &AppState, headers: &HeaderMap) -> Response {
    let locale = resolve_locale(headers, st);
    let theme = resolve_theme(headers);
    let items = match st.printers.list().await {
        Ok(items) => items,
        Err(e) => return internal_error(e),
    };
    let mut views = Vec::with_capacity(items.len());
    for item in items {
        match PrinterView::build(item, st).await {
            Ok(view) => views.push(view),
            Err(e) => return internal_error(e),
        }
    }
    let loaded_spools_count = views
        .iter()
        .flat_map(|p| &p.groups)
        .flat_map(|g| &g.slots)
        .filter(|s| s.loaded.is_some())
        .count();
    let mut ctx = Context::new();
    ctx.insert("printers", &views);
    ctx.insert("loaded_spools_count", &loaded_spools_count);
    match st
        .renderer
        .render("_printer_loading.html", &locale, theme.data_attr(), ctx)
    {
        Ok(h) => Html(h).into_response(),
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
struct SlotForm {
    #[serde(default)]
    spool_id: String,
}

async fn set_slot(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((printer_id, slot_key)): Path<(String, String)>,
    Form(form): Form<SlotForm>,
) -> Response {
    let printer_id = PrinterId::new(printer_id);
    let result = if form.spool_id.is_empty() {
        st.printers.unload_slot(printer_id, slot_key).await
    } else {
        st.printers
            .load_slot(printer_id, slot_key, SpoolId::new(form.spool_id))
            .await
    };
    match result {
        Ok(()) => loading_fragment(&st, &headers).await,
        Err(
            RepositoryError::NotFound(_)
            | RepositoryError::SlotNotFound { .. }
            | RepositoryError::UnknownSpool(_),
        ) => StatusCode::NOT_FOUND.into_response(),
        Err(RepositoryError::Domain(_)) => (
            StatusCode::CONFLICT,
            Html(st.renderer.t(
                &resolve_locale(&headers, &st),
                "printers.error.spool_not_loadable",
            )),
        )
            .into_response(),
        Err(RepositoryError::AlreadyLoaded(_)) => (
            StatusCode::CONFLICT,
            Html(st.renderer.t(
                &resolve_locale(&headers, &st),
                "printers.error.already_loaded",
            )),
        )
            .into_response(),
        Err(e) => internal_error(e),
    }
}
async fn form_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<FromQuery>,
) -> Response {
    render_form(&st, &headers, None, q.from).await
}
async fn edit_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<FromQuery>,
) -> Response {
    match st.printers.list().await {
        Ok(items) => match items.into_iter().find(|p| p.id.as_str() == id) {
            Some(p) => render_form(&st, &headers, Some(p), q.from).await,
            None => StatusCode::NOT_FOUND.into_response(),
        },
        Err(e) => internal_error(e),
    }
}
async fn render_form(
    st: &AppState,
    headers: &HeaderMap,
    printer: Option<Printer>,
    from: String,
) -> Response {
    let locale = resolve_locale(headers, st);
    let theme = resolve_theme(headers);
    let mut ctx = Context::new();
    ctx.insert("page", "printers");
    ctx.insert("printer", &printer.map(PrinterFormView::from));
    ctx.insert(
        "from",
        &if from == "settings" {
            "settings"
        } else {
            "printers"
        },
    );
    ctx.insert("bambu_models", &BAMBU_MODELS);
    ctx.insert("prusa_models", &PRUSA_MODELS);
    ctx.insert(
        "bambu_models_json",
        &serde_json::to_string(BAMBU_MODELS).unwrap(),
    );
    ctx.insert(
        "prusa_models_json",
        &serde_json::to_string(PRUSA_MODELS).unwrap(),
    );
    ctx.insert("nav_spool_count", &st.nav_spool_count().await);
    ctx.insert("nav_printer_count", &st.nav_printer_count().await);
    match st
        .renderer
        .render("printer_form.html", &locale, theme.data_attr(), ctx)
    {
        Ok(h) => Html(h).into_response(),
        Err(e) => internal_error(e),
    }
}
#[derive(Serialize)]
struct PrinterFormView {
    id: String,
    name: String,
    brand: String,
    model: String,
    heads: u8,
    module: String,
    module_count: Option<u8>,
    ams_units: u8,
    feed_modes: Vec<String>,
    feed_modes_json: String,
}
impl From<Printer> for PrinterFormView {
    fn from(p: Printer) -> Self {
        Self {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            brand: p.brand.as_str().into(),
            model: p.model,
            heads: p.heads,
            module: p.module.kind().into(),
            module_count: p.module.count(),
            ams_units: p.ams_units,
            feed_modes: p.feed_modes.iter().map(|m| m.as_str().into()).collect(),
            feed_modes_json: serde_json::to_string(
                &p.feed_modes.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[]".into()),
        }
    }
}
async fn create(State(st): State<AppState>, Form(f): Form<PrinterForm>) -> Response {
    let dest = f.destination();
    let (name, brand, model, heads, module, ams_units, feed_modes) = match f.domain() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Html(st.renderer.t(&st.default_locale, "printers.error.invalid")),
            )
                .into_response();
        }
    };
    match st
        .printers
        .add(NewPrinter {
            name,
            brand,
            model,
            heads,
            module,
            ams_units,
            feed_modes,
        })
        .await
    {
        Ok(_) => Redirect::to(dest).into_response(),
        Err(e) => internal_error(e),
    }
}
async fn update(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<PrinterForm>,
) -> Response {
    let dest = f.destination();
    let (name, brand, model, heads, module, ams_units, feed_modes) = match f.domain() {
        Ok(v) => v,
        Err(_) => return StatusCode::UNPROCESSABLE_ENTITY.into_response(),
    };
    let printer = Printer {
        id: PrinterId::new(id),
        name,
        brand,
        model,
        heads,
        module,
        ams_units,
        feed_modes,
        slots: vec![],
    };
    match st.printers.edit(printer).await {
        Ok(_) => Redirect::to(dest).into_response(),
        Err(RepositoryError::NotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => internal_error(e),
    }
}
async fn delete(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    match st.printers.delete(PrinterId::new(id)).await {
        Ok(()) => (StatusCode::OK, Html("")).into_response(),
        Err(RepositoryError::NotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => internal_error(e),
    }
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/printers", get(page).post(create))
        .route("/printers/new", get(form_page))
        .route("/printers/{id}/edit", get(edit_page))
        .route("/printers/{id}", axum::routing::post(update).delete(delete))
        .route(
            "/printers/{printer_id}/slots/{slot_key}",
            axum::routing::post(set_slot),
        )
}

#[cfg(test)]
mod tests {
    use super::PrinterForm;
    use domain::printers::FeedMode;

    // Reproduces the axum::Form path: urlencoded bodies cannot deserialize
    // repeated keys into a sequence, so feed_modes travels as one CSV field.
    fn form(body: &str) -> PrinterForm {
        serde_urlencoded::from_str(body).expect("urlencoded body deserializes")
    }

    #[test]
    fn single_head_bambu_feed_mode_round_trips() {
        let (_, _, _, heads, _, ams_units, feed_modes) = form(
            "name=Shop&brand=bambu&model=P1S&heads=1&module=none&ams_units=1&feed_modes=ams_fed",
        )
        .domain()
        .unwrap();
        assert_eq!(heads, 1);
        assert_eq!(ams_units, 1);
        assert_eq!(feed_modes, vec![FeedMode::AmsFed]);
    }

    #[test]
    fn dual_head_bambu_parses_csv_feed_modes_in_order() {
        let (_, _, _, _, _, _, feed_modes) = form(
            "name=H2D&brand=bambu&model=H2D&heads=2&module=none&ams_units=1&feed_modes=direct,ams_fed",
        )
        .domain()
        .unwrap();
        assert_eq!(feed_modes, vec![FeedMode::Direct, FeedMode::AmsFed]);
    }

    #[test]
    fn non_bambu_ignores_absent_feed_modes() {
        let (_, _, _, _, _, ams_units, feed_modes) =
            form("name=MK&brand=prusa&model=MK4S&heads=1&module=none")
                .domain()
                .unwrap();
        assert_eq!(ams_units, 0);
        assert_eq!(feed_modes, vec![FeedMode::Direct]);
    }

    #[test]
    fn feed_modes_count_must_match_heads() {
        assert!(
            form(
                "name=H2D&brand=bambu&model=H2D&heads=2&module=none&ams_units=1&feed_modes=direct"
            )
            .domain()
            .is_err()
        );
    }
}
