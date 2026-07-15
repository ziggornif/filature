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
    BAMBU_MODELS, Module, NewPrinter, PRUSA_MODELS, Printer, PrinterBrand, PrinterName,
    RepositoryError,
};
use domain::shared::PrinterId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tera::Context;

#[derive(Serialize)]
pub struct SlotGroupView {
    pub label_key: String,
    pub count: usize,
    pub multi: bool,
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
        let groups: Vec<_> = order
            .into_iter()
            .map(|label| {
                let count = grouped[&label];
                SlotGroupView {
                    label_key: format!("printers.group.{label}"),
                    count,
                    multi: count > 1,
                }
            })
            .collect();
        let slot_summary = groups
            .iter()
            .map(|g| format!("{} ({})", g.label_key, g.count))
            .collect::<Vec<_>>()
            .join(", ");
        Self {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            model: p.model,
            brand: p.brand.as_str().into(),
            liner: p.brand.liner().into(),
            groups,
            slot_summary,
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
    module: String,
    #[serde(default)]
    module_count: Option<u8>,
    #[serde(default)]
    from: String,
}
impl PrinterForm {
    fn domain(&self) -> Result<(PrinterName, PrinterBrand, String, Module), String> {
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
            "ams" => Module::Ams,
            "mmu" => Module::Mmu,
            "indx" => Module::Indx,
            "tool_changer" => Module::ToolChanger {
                heads: self.module_count.unwrap_or(2),
            },
            "multi_colour" => Module::MultiColour {
                slots: self.module_count.unwrap_or(4),
            },
            _ => return Err("invalid module".into()),
        };
        let module = Module::validate(brand, &model, module).map_err(|e| e.to_string())?;
        Ok((name, brand, model, module))
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
            let views: Vec<PrinterView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("page", "printers");
            ctx.insert("printers", &views);
            ctx.insert("printer_count", &views.len());
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
    module: String,
    module_count: Option<u8>,
}
impl From<Printer> for PrinterFormView {
    fn from(p: Printer) -> Self {
        Self {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            brand: p.brand.as_str().into(),
            model: p.model,
            module: p.module.kind().into(),
            module_count: p.module.count(),
        }
    }
}
async fn create(State(st): State<AppState>, Form(f): Form<PrinterForm>) -> Response {
    let dest = f.destination();
    let (name, brand, model, module) = match f.domain() {
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
            module,
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
    let (name, brand, model, module) = match f.domain() {
        Ok(v) => v,
        Err(_) => return StatusCode::UNPROCESSABLE_ENTITY.into_response(),
    };
    let printer = Printer {
        id: PrinterId::new(id),
        name,
        brand,
        model,
        module,
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
}
