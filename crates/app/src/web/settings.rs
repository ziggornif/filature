//! Driving adapter for the instance-wide settings screen.

use crate::instance_transfer::{decode, encode};
use crate::web::locations::LocationView;
use crate::web::manufacturers::ManufacturerView;
use crate::web::printers::PrinterView;
use crate::web::router::{form_error, internal_error, resolve_locale, resolve_theme};
use crate::web::state::AppState;
use axum::{
    Form, Router,
    extract::{DefaultBodyLimit, Multipart, Query, State, multipart::MultipartRejection},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use domain::instance_configuration::InstanceConfiguration;
use domain::instance_transfer::TransferError;
use domain::shared::LowStockThreshold;
use serde::Deserialize;
use tera::Context;

const MAX_IMPORT_BYTES: usize = 1024 * 1024;
const MAX_MULTIPART_BYTES: usize = MAX_IMPORT_BYTES + 64 * 1024;

#[derive(Deserialize)]
struct LowStockThresholdForm {
    low_stock_threshold: String,
}

#[derive(Default, Deserialize)]
struct SettingsQuery {
    #[serde(default)]
    imported: Option<String>,
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

async fn general_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.instance_configuration.get().await {
        Ok(configuration) => {
            let mut ctx = setting_context(configuration, false);
            ctx.insert("page", "settings");
            ctx.insert("active_tab", "general");
            ctx.insert("imported", &false);
            ctx.insert("import_error", "");
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &st.nav_printer_count().await);
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

async fn manufacturers_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.manufacturers.list_with_spool_counts().await {
        Ok(items) => {
            let manufacturers: Vec<ManufacturerView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("page", "settings");
            ctx.insert("active_tab", "manufacturers");
            ctx.insert("manufacturers", &manufacturers);
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &st.nav_printer_count().await);
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

async fn locations_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.locations.list_with_spool_counts().await {
        Ok(items) => {
            let locations: Vec<LocationView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("page", "settings");
            ctx.insert("active_tab", "locations");
            ctx.insert("locations", &locations);
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &st.nav_printer_count().await);
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

async fn printers_page(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    match st.printers.list().await {
        Ok(items) => {
            let printers: Vec<PrinterView> = items.into_iter().map(Into::into).collect();
            let mut ctx = Context::new();
            ctx.insert("page", "settings");
            ctx.insert("active_tab", "printers");
            ctx.insert("printers", &printers);
            ctx.insert("nav_spool_count", &st.nav_spool_count().await);
            ctx.insert("nav_printer_count", &printers.len());
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

async fn backup_page(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SettingsQuery>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let mut ctx = Context::new();
    ctx.insert("page", "settings");
    ctx.insert("active_tab", "backup");
    ctx.insert("imported", &(query.imported.as_deref() == Some("1")));
    ctx.insert("import_error", "");
    ctx.insert("nav_spool_count", &st.nav_spool_count().await);
    ctx.insert("nav_printer_count", &st.nav_printer_count().await);
    match st
        .renderer
        .render("settings.html", &locale, theme.data_attr(), ctx)
    {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn export_instance(State(st): State<AppState>) -> Response {
    let document = match st.instance_transfer.export().await {
        Ok(document) => document,
        Err(error) => return internal_error(error),
    };
    match encode(&document) {
        Ok(json) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/json; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=filature-instance.json",
                ),
            ],
            json,
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn import_error(st: &AppState, locale: &str, theme_attr: &str, key: &str) -> Response {
    let message = st.renderer.t(locale, key);
    let configuration = match st.instance_configuration.get().await {
        Ok(configuration) => configuration,
        Err(error) => return internal_error(error),
    };
    let mut context = setting_context(configuration, false);
    context.insert("page", "settings");
    context.insert("active_tab", "backup");
    context.insert("imported", &false);
    context.insert("import_error", &message);
    context.insert("nav_spool_count", &st.nav_spool_count().await);
    context.insert("nav_printer_count", &st.nav_printer_count().await);
    match st
        .renderer
        .render("settings.html", locale, theme_attr, context)
    {
        Ok(html) => (StatusCode::UNPROCESSABLE_ENTITY, Html(html)).into_response(),
        Err(error) => internal_error(error),
    }
}

async fn import_instance(
    State(st): State<AppState>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let locale = resolve_locale(&headers, &st);
    let theme = resolve_theme(&headers);
    let mut multipart = match multipart {
        Ok(multipart) => multipart,
        Err(_) => {
            return import_error(
                &st,
                &locale,
                theme.data_attr(),
                "settings.transfer.error.too_large",
            )
            .await;
        }
    };
    let mut confirmed = false;
    let mut backup = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => {
                return import_error(
                    &st,
                    &locale,
                    theme.data_attr(),
                    "settings.transfer.error.invalid",
                )
                .await;
            }
        };
        match field.name() {
            Some("confirm_replace") => match field.text().await {
                Ok(value) => confirmed = value == "yes",
                Err(_) => {
                    return import_error(
                        &st,
                        &locale,
                        theme.data_attr(),
                        "settings.transfer.error.invalid",
                    )
                    .await;
                }
            },
            Some("backup") if backup.is_none() => match field.bytes().await {
                Ok(bytes) if bytes.len() <= MAX_IMPORT_BYTES => backup = Some(bytes),
                Ok(_) => {
                    return import_error(
                        &st,
                        &locale,
                        theme.data_attr(),
                        "settings.transfer.error.too_large",
                    )
                    .await;
                }
                Err(_) => {
                    return import_error(
                        &st,
                        &locale,
                        theme.data_attr(),
                        "settings.transfer.error.invalid",
                    )
                    .await;
                }
            },
            _ => {
                return import_error(
                    &st,
                    &locale,
                    theme.data_attr(),
                    "settings.transfer.error.invalid",
                )
                .await;
            }
        }
    }

    if !confirmed {
        return import_error(
            &st,
            &locale,
            theme.data_attr(),
            "settings.transfer.error.confirmation",
        )
        .await;
    }
    let Some(backup) = backup else {
        return import_error(
            &st,
            &locale,
            theme.data_attr(),
            "settings.transfer.error.invalid",
        )
        .await;
    };
    let document = match decode(&backup) {
        Ok(document) => document,
        Err(_) => {
            return import_error(
                &st,
                &locale,
                theme.data_attr(),
                "settings.transfer.error.invalid",
            )
            .await;
        }
    };

    match st.instance_transfer.import(document).await {
        Ok(()) => Redirect::to("/settings/backup?imported=1").into_response(),
        Err(TransferError::UnsupportedFormat(_) | TransferError::UnsupportedVersion(_)) => {
            import_error(
                &st,
                &locale,
                theme.data_attr(),
                "settings.transfer.error.incompatible",
            )
            .await
        }
        Err(TransferError::Invalid(_)) => {
            import_error(
                &st,
                &locale,
                theme.data_attr(),
                "settings.transfer.error.invalid",
            )
            .await
        }
        Err(TransferError::Backend(error)) => internal_error(error),
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
    Router::new()
        .route("/settings", get(general_page))
        .route("/settings/manufacturers", get(manufacturers_page))
        .route("/settings/locations", get(locations_page))
        .route("/settings/printers", get(printers_page))
        .route("/settings/backup", get(backup_page))
        .route(
            "/settings/low-stock-threshold",
            axum::routing::post(update_low_stock_threshold),
        )
        .route("/settings/export", get(export_instance))
        .route(
            "/settings/import",
            axum::routing::post(import_instance).layer(DefaultBodyLimit::max(MAX_MULTIPART_BYTES)),
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
        ctx.insert("active_tab", "general");
        ctx.insert("imported", &false);
        ctx.insert("import_error", "");
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
        assert!(html.contains(r#"class="settings-tabs" hx-boost="true""#));
        assert!(html.contains(r#"href="/settings/manufacturers""#));
        assert!(html.contains(r#"href="/settings/locations""#));
        assert!(html.contains(r#"href="/settings/backup""#));
        assert!(!html.contains(r#"href="/manufacturers""#));
        assert!(!html.contains(r#"href="/locations""#));
        assert!(!html.contains("settings.low_stock."));
        assert!(!html.contains("settings.transfer."));
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

        fn test_state() -> (
            AppState,
            Arc<dyn InstanceConfigurationUseCases>,
            Arc<domain::instance_transfer::stubs::StubInstanceTransferRepository>,
        ) {
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
            let instance_transfer_repository = Arc::new(
                domain::instance_transfer::stubs::StubInstanceTransferRepository::default(),
            );
            let state = AppState::new(
                db,
                &cfg,
                materials,
                spools,
                locations,
                manufacturers,
                dashboard,
                instance_configuration.clone(),
                Arc::new(domain::instance_transfer::InstanceTransferService::new(
                    instance_transfer_repository.clone(),
                )),
            );
            (state, instance_configuration, instance_transfer_repository)
        }

        async fn body_of(response: Response) -> String {
            let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        }

        #[tokio::test]
        async fn settings_tabs_are_full_pages_and_legacy_gets_are_not_found() {
            use axum::{body::Body, http::Request};
            use tower::ServiceExt;

            let (state, _, _) = test_state();
            for (path, active_href) in [
                ("/settings", "/settings"),
                ("/settings/manufacturers", "/settings/manufacturers"),
                ("/settings/locations", "/settings/locations"),
                ("/settings/backup", "/settings/backup"),
            ] {
                let response = crate::web::router::router(state.clone())
                    .oneshot(Request::get(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK, "{path}");
                let html = body_of(response).await;
                assert!(html.contains(r#"class="settings-tabs" hx-boost="true""#));
                assert!(html.contains(&format!(
                    r#"href="{active_href}" class="active" aria-current="page""#
                )));
            }

            for path in ["/manufacturers", "/locations"] {
                let response = crate::web::router::router(state.clone())
                    .oneshot(Request::get(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            }
        }

        #[tokio::test]
        async fn update_persists_threshold_and_returns_localised_htmx_fragment() {
            let (state, configuration, _) = test_state();
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
            let (state, _, _) = test_state();
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

        fn multipart_body(json: &[u8], confirmed: bool) -> (String, Vec<u8>) {
            let boundary = "filature-test-boundary";
            let mut body = Vec::new();
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"backup\"; filename=\"backup.json\"\r\nContent-Type: application/json\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(json);
            body.extend_from_slice(b"\r\n");
            if confirmed {
                body.extend_from_slice(
                    format!(
                        "--{boundary}\r\nContent-Disposition: form-data; name=\"confirm_replace\"\r\n\r\nyes\r\n"
                    )
                    .as_bytes(),
                );
            }
            body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            (boundary.into(), body)
        }

        #[tokio::test]
        async fn export_returns_a_versioned_json_attachment() {
            let (state, _, _) = test_state();
            let response = export_instance(State(state)).await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/json; charset=utf-8"
            );
            assert!(
                response.headers()[header::CONTENT_DISPOSITION]
                    .to_str()
                    .unwrap()
                    .contains("attachment")
            );
            let json: serde_json::Value = serde_json::from_str(&body_of(response).await).unwrap();
            assert_eq!(json["format"], domain::instance_transfer::FORMAT);
            assert_eq!(json["version"], domain::instance_transfer::VERSION);
        }

        #[tokio::test]
        async fn import_requires_explicit_confirmation_before_replacing() {
            use axum::{body::Body, http::Request};
            use tower::ServiceExt;

            let (state, _, repository) = test_state();
            let document = state.instance_transfer.export().await.unwrap();
            let json = crate::instance_transfer::encode(&document).unwrap();
            let (boundary, body) = multipart_body(&json, false);
            let response = routes()
                .with_state(state)
                .oneshot(
                    Request::post("/settings/import")
                        .header(
                            header::CONTENT_TYPE,
                            format!("multipart/form-data; boundary={boundary}"),
                        )
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(repository.replace_count(), 0);
        }

        #[tokio::test]
        async fn incompatible_import_is_rejected_without_replacing() {
            use axum::{body::Body, http::Request};
            use tower::ServiceExt;

            let (state, _, repository) = test_state();
            let mut document = state.instance_transfer.export().await.unwrap();
            document.version += 1;
            let json = crate::instance_transfer::encode(&document).unwrap();
            let (boundary, body) = multipart_body(&json, true);
            let response = routes()
                .with_state(state)
                .oneshot(
                    Request::post("/settings/import")
                        .header(
                            header::CONTENT_TYPE,
                            format!("multipart/form-data; boundary={boundary}"),
                        )
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(repository.replace_count(), 0);
        }

        #[tokio::test]
        async fn valid_confirmed_import_replaces_and_redirects() {
            use axum::{body::Body, http::Request};
            use tower::ServiceExt;

            let (state, _, repository) = test_state();
            let document = state.instance_transfer.export().await.unwrap();
            let json = crate::instance_transfer::encode(&document).unwrap();
            let (boundary, body) = multipart_body(&json, true);
            let response = routes()
                .with_state(state)
                .oneshot(
                    Request::post("/settings/import")
                        .header(
                            header::CONTENT_TYPE,
                            format!("multipart/form-data; boundary={boundary}"),
                        )
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(
                response.headers()[header::LOCATION],
                "/settings/backup?imported=1"
            );
            assert_eq!(repository.replace_count(), 1);
        }

        #[tokio::test]
        async fn import_rejects_a_file_over_the_limit_without_replacing() {
            use axum::{body::Body, http::Request};
            use tower::ServiceExt;

            let (state, _, repository) = test_state();
            let oversized = vec![b' '; MAX_IMPORT_BYTES + 1];
            let (boundary, body) = multipart_body(&oversized, true);
            let response = routes()
                .with_state(state)
                .oneshot(
                    Request::post("/settings/import")
                        .header(
                            header::CONTENT_TYPE,
                            format!("multipart/form-data; boundary={boundary}"),
                        )
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            assert!(
                body_of(response)
                    .await
                    .contains("The backup exceeds the 1 MiB limit.")
            );
            assert_eq!(repository.replace_count(), 0);
        }
    }
}
