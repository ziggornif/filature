# Contributing to Filature

Filature is a self-hosted filament stock manager for 3D-printing workshops. Start with the [public documentation site](https://ziggornif.github.io/filature/) for a product overview.

## Prerequisites

- Rust edition 2024, version 1.85 or newer. Container images currently pin Rust 1.97.
- Docker and Docker Compose for PostgreSQL and database-backed tests.
- The craft-harness submodule: run `git submodule update --init` after cloning.

## Run locally

The simplest development setup is:

```sh
cp .env.example .env
docker compose up
```

Alternatively, run your own PostgreSQL instance, set `FILATURE_DATABASE__URL`, and start the Rust application with Cargo. Templates, static assets, translations, and migrations are embedded in the binary, so editing an embedded static asset requires a rebuild.

## Tests

Run the complete Rust test suite with:

```sh
cargo test
```

SQLx uses the checked-in `.sqlx/` offline cache; CI builds with `SQLX_OFFLINE=true`. Keep that cache synchronized whenever queries change. Integration tests use testcontainers to start PostgreSQL. `tools/test.sh` also manages the testcontainers reaper when running the repository test workflow.

## Architecture

Filature uses hexagonal architecture organized into vertical use-case slices. Read [docs/architecture.md](docs/architecture.md), the [architecture decision records](docs/adr/), and [docs/glossary.md](docs/glossary.md) before making structural or domain-language changes.

All user-interface text must go through the i18n catalogs—never hardcode UI strings. See [ADR-0001](docs/adr/0001-language-and-i18n.md).

## Workflow

- Create focused commits using Conventional Commit messages such as `feat(spools): …` or `fix(auth): …`.
- Open pull requests against `main`.
- Keep `.sqlx/` metadata and both i18n catalogs synchronized with code changes.
- Ensure every CI job passes before requesting review.

The briefs in [docs/specs/](docs/specs/) define the slices and acceptance criteria. The craft harness in `.claude/harness` documents the discovery, design, delegation, review, and delivery process used to build them.
