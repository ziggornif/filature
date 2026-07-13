//! Driving adapter for the instance-wide settings screen.

use crate::web::router::{form_error, internal_error, resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Form, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use domain::instance_configuration::InstanceConfiguration;
use domain::shared::LowStockThreshold;
use serde::Deserialize;
use tera::Context;

#[derive(Deserialize)]
struct LowStockThresholdForm {
    low_stock_threshold: String,
}

impl LowStockThresholdForm {
    fn threshold(&self) -> Option<LowStockThreshold> {
        self.low_stock_threshold
            .parse::<i64>()
            .ok()
            .and_then(|value| LowStockThreshold::new(value).ok())
    }
}

fn setting_context(configuration: InstanceConfiguration, saved: bool) -> Context {
    let mut ctx = Context::new();
    ctx.insert(
        "low_stock_threshold_pct",
        &configuration.low_stock_threshold.percent(),
    );
    ctx.insert("saved", &saved);
    ctx
}

fn render_setting(st: &AppState, locale: &str, configuration: InstanceConfiguration) -> Response {
    match st.renderer.render(
        "_low_stock_threshold_setting.html",
        locale,
        "",
        setting_context(configuration, true),
    ) {
        Ok(html) => (
            StatusCode::OK,
            Html(format!(
                "{html}<div id=\"settings-msg\" hx-swap-oob=\"innerHTML\"></div>"
            )),
        )
            .into_response(),
        Err(e) => internal_error(e),
    }
}

async fn page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.instance_configuration.get().await {
        Ok(configuration) => {
            let mut ctx = setting_context(configuration, false);
            ctx.insert("page", "settings");
            match st
                .renderer
                .render("settings.html", &locale, theme.data_attr(), ctx)
            {
                Ok(html) => Html(html).into_response(),
                Err(e) => internal_error(e),
            }
        }
        Err(e) => internal_error(e),
    }
}

async fn update_low_stock_threshold(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<LowStockThresholdForm>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let Some(threshold) = form.threshold() else {
        let message = st.renderer.t(&locale, "settings.low_stock.error.range");
        return form_error(&st, &locale, StatusCode::UNPROCESSABLE_ENTITY, &message);
    };

    match st
        .instance_configuration
        .update_low_stock_threshold(threshold)
        .await
    {
        Ok(configuration) => render_setting(&st, &locale, configuration),
        Err(e) => internal_error(e),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/settings", get(page)).route(
        "/settings/low-stock-threshold",
        axum::routing::post(update_low_stock_threshold),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::i18n::Catalog;
    use crate::web::templates::Renderer;

    fn render(locale: &str, threshold: i64, saved: bool) -> String {
        let renderer = Renderer::new(Catalog::load("en"));
        let configuration = InstanceConfiguration {
            low_stock_threshold: LowStockThreshold::new(threshold).unwrap(),
        };
        let mut ctx = setting_context(configuration, saved);
        ctx.insert("page", "settings");
        renderer.render("settings.html", locale, "", ctx).unwrap()
    }

    #[test]
    fn settings_page_renders_value_and_htmx_contract() {
        let html = render("en", 15, false);
        assert!(html.contains(r#"href="/settings" class="active""#));
        assert!(html.contains(r#"value="15""#));
        assert!(html.contains(r#"min="0""#));
        assert!(html.contains(r#"max="100""#));
        assert!(html.contains(r#"hx-post="/settings/low-stock-threshold""#));
        assert!(!html.contains("settings.low_stock."));
        assert!(!html.contains("nav.settings"));
    }

    #[test]
    fn settings_page_renders_non_default_french_locale() {
        let html = render("fr", 27, true);
        assert!(html.contains("Paramètres"));
        assert!(html.contains("Seuil de stock bas"));
        assert!(html.contains("Enregistré."));
        assert!(html.contains(r#"lang="fr""#));
        assert!(!html.contains("settings.low_stock."));
    }

    #[test]
    fn form_rejects_values_outside_range_and_non_numbers() {
        for value in ["-1", "101", "not-a-number", ""] {
            let form = LowStockThresholdForm {
                low_stock_threshold: value.to_string(),
            };
            assert!(form.threshold().is_none(), "{value} must be rejected");
        }
    }

    mod handlers {
        use super::*;
        use crate::config::{Config, DatabaseConfig, I18nConfig, ServerConfig};
        use axum::body::to_bytes;
        use domain::dashboard::{DashboardService, DashboardUseCases};
        use domain::instance_configuration::{
            InstanceConfigurationService, InstanceConfigurationUseCases,
        };
        use domain::locations::{LocationsService, LocationsUseCases};
        use domain::manufacturers::{ManufacturersService, ManufacturersUseCases};
        use domain::materials::{MaterialsService, MaterialsUseCases};
        use domain::spools::{SpoolsService, SpoolsUseCases};
        use sqlx::PgPool;
        use std::sync::Arc;

        fn test_state() -> (AppState, Arc<dyn InstanceConfigurationUseCases>) {
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
            let materials: Arc<dyn MaterialsUseCases> = Arc::new(MaterialsService::new(Arc::new(
                domain::materials::stubs::StubMaterialRepository::new(),
            )));
            let spools: Arc<dyn SpoolsUseCases> = Arc::new(SpoolsService::new(Arc::new(
                domain::spools::stubs::StubSpoolRepository::new(),
            )));
            let locations: Arc<dyn LocationsUseCases> = Arc::new(LocationsService::new(Arc::new(
                domain::locations::stubs::StubLocationRepository::new(),
            )));
            let manufacturers: Arc<dyn ManufacturersUseCases> =
                Arc::new(ManufacturersService::new(Arc::new(
                    domain::manufacturers::stubs::StubManufacturerRepository::new(),
                )));
            let dashboard: Arc<dyn DashboardUseCases> = Arc::new(DashboardService::new(Arc::new(
                domain::dashboard::stubs::StubDashboardRepository::new(),
            )));
            let instance_configuration: Arc<dyn InstanceConfigurationUseCases> =
                Arc::new(InstanceConfigurationService::new(Arc::new(
                    domain::instance_configuration::stubs::StubInstanceConfigurationRepository::new(
                    ),
                )));
            let state = AppState::new(
                db,
                &cfg,
                materials,
                spools,
                locations,
                manufacturers,
                dashboard,
                instance_configuration.clone(),
            );
            (state, instance_configuration)
        }

        async fn body_of(response: Response) -> String {
            let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        }

        #[tokio::test]
        async fn update_persists_threshold_and_returns_localised_htmx_fragment() {
            let (state, configuration) = test_state();
            let mut headers = HeaderMap::new();
            headers.insert(axum::http::header::COOKIE, "lang=fr".parse().unwrap());

            let response = update_low_stock_threshold(
                State(state),
                headers,
                Form(LowStockThresholdForm {
                    low_stock_threshold: "27".into(),
                }),
            )
            .await;

            assert_eq!(response.status(), StatusCode::OK);
            let html = body_of(response).await;
            assert!(html.contains(r#"value="27""#));
            assert!(html.contains("Enregistré."));
            assert_eq!(
                configuration
                    .get()
                    .await
                    .unwrap()
                    .low_stock_threshold
                    .percent(),
                27
            );
        }

        #[tokio::test]
        async fn invalid_update_returns_clear_localised_422() {
            let (state, _) = test_state();
            let mut headers = HeaderMap::new();
            headers.insert(axum::http::header::COOKIE, "lang=fr".parse().unwrap());

            let response = update_low_stock_threshold(
                State(state),
                headers,
                Form(LowStockThresholdForm {
                    low_stock_threshold: "101".into(),
                }),
            )
            .await;

            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert!(
                body_of(response)
                    .await
                    .contains("Saisissez un pourcentage entier compris entre 0 et 100.")
            );
        }
    }
}
