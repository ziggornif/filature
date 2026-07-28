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

## Instance de démo

L'instance destinée aux captures utilise PostgreSQL mais ne contacte aucune
imprimante. Démarrez PostgreSQL, définissez une clé AES de 32 octets encodée en
base64, puis lancez l'application avec le stub explicite :

```sh
export FILATURE_CREDENTIALS_KEY="$(openssl rand -base64 32)"
export FILATURE_MACHINE_STUB=tools/demo-machines.json
cargo run -p filature --features demo-stub
```

Dans un autre terminal, avec la même `FILATURE_CREDENTIALS_KEY`, importez le jeu
de données et créez les huit Machine Links :

```sh
cargo build -p filature --features demo-stub
tools/seed-demo.sh --yes
```

⚠️ `seed-demo.sh` **remplace tout le contenu** de l'instance visée (bobines,
imprimantes, rangements, Machine Links). Il refuse de tourner sans `--yes`, et
refuse toute `FILATURE_URL` qui ne pointe pas sur la machine locale — ne
contournez pas ces garde-fous pour le lancer contre une instance réelle.

Le script accepte `FILATURE_URL`, `FILATURE_USER`, `FILATURE_PASSWORD`,
`DATABASE_URL` et `FILATURE_BIN`. `FILATURE_DEMO` doit rester non définie :
ce mode public masque précisément Machine Link et Farm Activity.

## Tests

Run the complete Rust test suite with:

```sh
cargo test
```

SQLx uses the checked-in `.sqlx/` offline cache; CI builds with `SQLX_OFFLINE=true`. Keep that cache synchronized whenever queries change. Integration tests use testcontainers to start PostgreSQL. `tools/test.sh` also manages the testcontainers reaper when running the repository test workflow.

### End-to-end accessibility

The Playwright and axe-core suite runs against a disposable Docker Compose project. It binds Filature to `127.0.0.1:18081`, imports the bundled demo dataset (`e2e/fixtures/demo-instance.json`), checks every authenticated screen in light and dark mode, then removes its dedicated containers and volume.

```sh
cd e2e
npm install
npx playwright install chromium
npm run test:local
```

Set `FILATURE_E2E_PORT` to override the port. All critical/serious WCAG failures are blocking by default, including color contrast. Contrast details are attached to the Playwright report when the test fails.

CI should initially run `npm run test:local` as an advisory job and always publish the Playwright report. Use the repository's `ci-setup` skill when wiring it; this slice intentionally does not add CI configuration.

## Architecture

Filature uses hexagonal architecture organized into vertical use-case slices. Read [docs/architecture.md](docs/architecture.md), the [architecture decision records](docs/adr/), and [docs/glossary.md](docs/glossary.md) before making structural or domain-language changes.

All user-interface text must go through the i18n catalogs—never hardcode UI strings. See [ADR-0001](docs/adr/0001-language-and-i18n.md).

## Workflow

- Create focused commits using Conventional Commit messages such as `feat(spools): …` or `fix(auth): …`.
- Open pull requests against `main`.
- Keep `.sqlx/` metadata and both i18n catalogs synchronized with code changes.
- Ensure every CI job passes before requesting review.

The briefs in [docs/specs/](docs/specs/) define the slices and acceptance criteria. The craft harness in `.claude/harness` documents the discovery, design, delegation, review, and delivery process used to build them.
