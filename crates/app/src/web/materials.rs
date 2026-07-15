//! The driving (Axum) adapter for the materials slice: an htmx editable
//! table (`GET /materials`) whose rows are edited/created in place via
//! row-fragment responses (`POST /materials`, `PUT /materials/{id}`).

use crate::web::router::{form_error, internal_error, resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, put},
};
use domain::materials::{
    Density, DryingParams, Material, MaterialId, MaterialName, NewMaterial, RepositoryError,
    Sensitivity, Temperature,
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Template-shaped view of a `Material`: plain strings/numbers plus the
/// derived `threshold_pct` (the domain never exposes a "view model").
#[derive(Serialize)]
pub struct MaterialView {
    pub id: String,
    pub name: String,
    pub density: f64,
    pub drying_temp_c: u16,
    pub drying_time_h: u16,
    pub sensitivity: String, // "Low" | "Medium" | "High" — drives the <select>
    pub threshold_pct: u8,   // derived from sensitivity
    pub nozzle_c: u16,
    pub bed_c: u16,
}

impl From<Material> for MaterialView {
    fn from(m: Material) -> Self {
        Self {
            id: m.id.as_str().to_string(),
            name: m.name.as_str().to_string(),
            density: m.density.value(),
            drying_temp_c: m.drying.temp.value(),
            drying_time_h: m.drying.time_h,
            threshold_pct: m.sensitivity.humidity_threshold_pct(),
            sensitivity: m.sensitivity.as_str().to_string(),
            nozzle_c: m.nozzle.value(),
            bed_c: m.bed.value(),
        }
    }
}

/// The htmx form payload for both create (`POST /materials`) and edit
/// (`PUT /materials/{id}`) — field names must match the `<input name=...>`
/// attributes in `_material_row.html`.
#[derive(Deserialize)]
pub struct MaterialForm {
    pub name: String,
    pub density: f64,
    pub drying_temp_c: u16,
    pub drying_time_h: u16,
    pub sensitivity: String,
    pub nozzle_c: u16,
    pub bed_c: u16,
}

impl MaterialForm {
    /// Maps the raw form into a domain `NewMaterial`, rejecting invalid
    /// density/sensitivity with a plain client-facing error message (the
    /// caller turns this into a 422) rather than panicking or 500-ing.
    fn to_new(&self) -> Result<NewMaterial, String> {
        Ok(NewMaterial {
            name: MaterialName::new(self.name.clone()).map_err(|e| e.to_string())?,
            density: Density::new(self.density).map_err(|e| e.to_string())?,
            drying: DryingParams {
                temp: Temperature::new(self.drying_temp_c),
                time_h: self.drying_time_h,
            },
            sensitivity: Sensitivity::parse(&self.sensitivity).map_err(|e| e.to_string())?,
            nozzle: Temperature::new(self.nozzle_c),
            bed: Temperature::new(self.bed_c),
        })
    }
}

fn render_row(st: &AppState, locale: &str, m: Material) -> Response {
    let view: MaterialView = m.into();
    let mut ctx = Context::new();
    ctx.insert("m", &view);
    match st.renderer.render("_material_row.html", locale, "", ctx) {
        // A trailing OOB clear empties any stale error left in `#materials-msg`
        // by a prior failed submit, so a successful add/edit dismisses it
        // (TD-009).
        Ok(html) => Html(format!(
            "{html}<div id=\"materials-msg\" hx-swap-oob=\"innerHTML\"></div>"
        ))
        .into_response(),
        Err(e) => internal_error(e),
    }
}

async fn list_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.materials.list().await {
        Ok(items) => {
            let views: Vec<MaterialView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("materials", &views);
            ctx.insert("page", "materials");
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &st.nav_printer_count().await);
            match st
                .renderer
                .render("materials.html", &locale, theme.data_attr(), ctx)
            {
                Ok(html) => Html(html).into_response(),
                Err(e) => internal_error(e),
            }
        }
        Err(e) => internal_error(e),
    }
}

async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(f): Form<MaterialForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let new = match f.to_new() {
        Ok(n) => n,
        Err(_) => {
            let msg = st.renderer.t(&locale, "materials.error.invalid");
            return form_error(&st, &locale, StatusCode::UNPROCESSABLE_ENTITY, &msg);
        }
    };
    match st.materials.add(new).await {
        Ok(m) => render_row(&st, &locale, m),
        Err(RepositoryError::Duplicate(name)) => {
            let msg = st
                .renderer
                .t(&locale, "materials.error.duplicate")
                .replace("{name}", &name);
            form_error(&st, &locale, StatusCode::CONFLICT, &msg)
        }
        Err(e) => internal_error(e),
    }
}

async fn edit(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(f): Form<MaterialForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let new = match f.to_new() {
        Ok(n) => n,
        Err(_) => {
            let msg = st.renderer.t(&locale, "materials.error.invalid");
            return form_error(&st, &locale, StatusCode::UNPROCESSABLE_ENTITY, &msg);
        }
    };
    let material = Material {
        id: MaterialId::new(id),
        name: new.name,
        density: new.density,
        drying: new.drying,
        sensitivity: new.sensitivity,
        nozzle: new.nozzle,
        bed: new.bed,
    };
    match st.materials.edit(material).await {
        Ok(m) => render_row(&st, &locale, m),
        Err(RepositoryError::Duplicate(name)) => {
            let msg = st
                .renderer
                .t(&locale, "materials.error.duplicate")
                .replace("{name}", &name);
            form_error(&st, &locale, StatusCode::CONFLICT, &msg)
        }
        Err(RepositoryError::NotFound(_)) => {
            let msg = st.renderer.t(&locale, "materials.error.not_found");
            form_error(&st, &locale, StatusCode::NOT_FOUND, &msg)
        }
        Err(e) => internal_error(e),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/materials", get(list_page).post(create))
        .route("/materials/{id}", put(edit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;

    fn view() -> MaterialView {
        MaterialView {
            id: "01HZID".into(),
            name: "PLA".into(),
            density: 1.24,
            drying_temp_c: 45,
            drying_time_h: 6,
            sensitivity: "Low".into(),
            threshold_pct: 40,
            nozzle_c: 210,
            bed_c: 60,
        }
    }

    fn render(locale: &str) -> String {
        render_materials(locale, &[view()])
    }

    fn render_materials(locale: &str, materials: &[MaterialView]) -> String {
        let r = Renderer::new(Catalog::load("en"));
        let mut ctx = Context::new();
        ctx.insert("materials", materials);
        r.render("materials.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn table_shows_material_and_threshold_no_raw_keys() {
        let html = render("en");
        assert!(html.contains("PLA"));
        assert!(html.contains("40")); // derived threshold
        assert!(!html.contains("materials.col.")); // no raw i18n key leaks
    }

    #[test]
    fn table_localises_to_french() {
        let html = render("fr");
        assert!(html.contains("Matériau") || html.contains("Densité"));
        assert!(!html.contains("materials.col."));
    }

    #[test]
    fn page_explains_that_material_settings_are_indicative() {
        let en = render("en");
        assert!(en.contains(
            "Reference guide — indicative drying and printing settings. Adjust according to the brand."
        ));
        assert!(en.contains("These settings are starting points, not stock data."));
        assert!(en.contains(r#"class="materials-info" role="note""#));
        assert!(en.contains(r#"class="materials-info-icon" aria-hidden="true""#));

        let fr = render("fr");
        assert!(fr.contains("Référentiel — réglages indicatifs de séchage et d"));
        assert!(fr.contains("impression. À ajuster selon la marque."));
        assert!(fr.contains("Ces réglages sont des valeurs de départ, pas des données de stock."));
        assert!(!fr.contains("materials.subtitle"));
        assert!(!fr.contains("materials.notice"));
    }

    #[test]
    fn page_wires_the_error_feedback_slot() {
        // TD-009 wiring guard: the message slot, the response-targets extension,
        // and the per-control error routing must all be present so a 422/409
        // reaches the DOM instead of failing silently.
        let html = render("en");
        assert!(html.contains(r#"id="materials-msg""#));
        assert!(html.contains(r#"hx-ext="response-targets""#));
        assert!(html.contains(r##"hx-target-error="#materials-msg""##)); // add form + row inputs
    }

    #[test]
    fn table_matches_the_editable_materials_handoff() {
        let fr = render("fr");

        assert_eq!(fr.matches("<th>Séchage</th>").count(), 1);
        assert!(!fr.contains("Séchage —"));
        assert!(fr.contains(r#"class="material-drying""#));
        assert!(fr.contains(r#"name="drying_temp_c""#));
        assert!(fr.contains(r#"name="drying_time_h""#));
        assert!(fr.contains("°C ·"));
        assert!(fr.contains("h</span>"));

        assert!(fr.contains(r#"class="material-name-badge referential-name-badge""#));
        assert!(fr.contains(r#"class="material-sensitivity-pill""#));
        assert!(fr.contains("material-row--low"));
        assert!(fr.contains(r#"hx-put="/materials/01HZID""#));
        assert!(fr.contains("<th>Seuil %HR</th>"));
        assert!(fr.contains("<th>Buse</th>"));
        assert!(fr.contains("<th>Plateau</th>"));
        assert!(fr.contains("Modifier une valeur met à jour aussitôt les longueurs restantes et les seuils d’alerte d’humidité."));

        let en = render("en");
        assert!(en.contains("<th>Drying</th>"));
        assert!(en.contains("<th>RH threshold</th>"));
        assert!(en.contains("<th>Nozzle</th>"));
        assert!(en.contains("<th>Bed</th>"));
        assert!(en.contains(
            "Editing a value immediately updates remaining lengths and humidity alert thresholds."
        ));
        assert!(!en.contains("materials.footer"));
    }

    #[test]
    fn each_sensitivity_renders_its_semantic_pill_hook() {
        let mut low = view();
        low.id = "LOW".into();
        let mut medium = view();
        medium.id = "MEDIUM".into();
        medium.sensitivity = "Medium".into();
        let mut high = view();
        high.id = "HIGH".into();
        high.sensitivity = "High".into();

        let html = render_materials("fr", &[low, medium, high]);
        assert!(html.contains("material-row--low"));
        assert!(html.contains("material-row--medium"));
        assert!(html.contains("material-row--high"));
        assert_eq!(html.matches("material-sensitivity-pill").count(), 3);
        assert!(html.contains("Faible"));
        assert!(html.contains("Moyenne"));
        assert!(html.contains("Élevée"));
    }
}
