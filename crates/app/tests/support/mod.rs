//! Shared Postgres testcontainer setup for the integration (`it_*`) and e2e
//! (`e2e_*`) test binaries (ADR-0003 — test the engine we ship). Not a test
//! file itself: it lives in a subdirectory so Cargo doesn't compile it as its
//! own integration-test binary; each test file pulls it in via `mod support;`.

use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

static POSTGRES: OnceCell<(ContainerAsync<Postgres>, Arc<str>)> = OnceCell::const_new();

/// Boots a throwaway Postgres container the first time it's called in this
/// test binary, then reuses it for every subsequent call — so a test file
/// with several `#[tokio::test]`s only pays the container-startup cost once.
/// Returns a ready `postgres://` connection URL.
pub async fn postgres_url() -> Arc<str> {
    let (_container, url) = POSTGRES
        .get_or_init(|| async {
            let container = Postgres::default()
                .start()
                .await
                .expect("postgres testcontainer starts");
            let host = container
                .get_host()
                .await
                .expect("postgres testcontainer host");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("postgres testcontainer port");
            let url: Arc<str> =
                format!("postgres://postgres:postgres@{host}:{port}/postgres").into();
            (container, url)
        })
        .await;
    url.clone()
}
