# Deployment — Docker Compose on a VM

Filature ships as a single self-contained binary: templates, static assets,
i18n catalogs and DB migrations are all embedded, and migrations run
automatically on boot. A deploy is therefore just **the app container + a
PostgreSQL container**. HTTPS is expected to be terminated by a reverse proxy
running elsewhere, which forwards to this VM's published app port.

## Prerequisites

- A VM with Docker Engine + the Compose plugin (`docker compose`).
- Network reachability from your reverse-proxy host to this VM's app port.

## First deploy

```sh
git clone <this-repo> && cd filature
cp .env.example .env
# edit .env — at minimum set a real POSTGRES_PASSWORD
docker compose up -d --build
```

The app waits for Postgres to be healthy, runs the migrations, seeds the
material + manufacturer referentials, then listens on the published port
(default `8080`). Check it:

```sh
docker compose ps
docker compose logs -f app
curl -sf http://localhost:8080/ >/dev/null && echo ok
```

Point your external reverse proxy at `http://<vm-ip>:8080`.

## Configuration

All config is environment variables (no `filature.toml` in the image). The
Compose file wires these from `.env`:

| `.env` var | Purpose | Default |
|---|---|---|
| `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` | DB credentials (used by both services) | `filature` / — / `filature` |
| `APP_PORT` | Host port the app is published on | `8080` |
| `APP_BIND` | Host interface to bind the published port to | `0.0.0.0` |
| `FILATURE_DEFAULT_LOCALE` | UI default locale (`fr` \| `en`) | `fr` |

The app itself reads `FILATURE_SERVER__BIND`, `FILATURE_DATABASE__URL` and
`FILATURE_I18N__DEFAULT_LOCALE` (set for you in `docker-compose.yml`). Any
`FILATURE_<SECTION>__<KEY>` env var overrides config.

## Security note

The app has **no built-in authentication** — anyone who can reach the published
port has full access. Keep it off the public internet: bind it to a private
interface (`APP_BIND=<private-ip>` in `.env`) and/or restrict it with a
firewall so only your reverse-proxy host can reach it. The reverse proxy is
where you add TLS and, if needed, access control.

## Operations

```sh
# Update to a new version
git pull && docker compose up -d --build

# Logs / status
docker compose logs -f app
docker compose ps

# Backup the database (volume `pgdata`)
docker compose exec db pg_dump -U filature filature > backup-$(date +%F).sql

# Restore
cat backup.sql | docker compose exec -T db psql -U filature -d filature

# Stop (data survives in the named volume) / full teardown incl. data
docker compose down
docker compose down -v   # ⚠ deletes the pgdata volume
```
