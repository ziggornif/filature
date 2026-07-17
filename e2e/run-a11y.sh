#!/bin/sh
set -eu
E2E_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$E2E_DIR/.." && pwd)
COMPOSE_PROJECT_NAME=${FILATURE_E2E_COMPOSE_PROJECT:-filature-a11y-e2e}
APP_PORT=${FILATURE_E2E_PORT:-18081}
E2E_PASSWORD=${FILATURE_E2E_PASSWORD:-filature-a11y}
cleanup() { docker compose -p "$COMPOSE_PROJECT_NAME" -f "$REPO_DIR/docker-compose.yml" down -v --remove-orphans; }
trap cleanup EXIT INT TERM
cd "$REPO_DIR"
export POSTGRES_PASSWORD=filature-a11y-postgres AUTH_USERNAME=${FILATURE_E2E_USERNAME:-a11y} APP_BIND=127.0.0.1 APP_PORT
# Compose validates required substitutions even for `build` and the hashing
# subcommand. The placeholder is never used to start the application.
export AUTH_PASSWORD_HASH=not-used-before-startup
docker compose -p "$COMPOSE_PROJECT_NAME" build app
AUTH_PASSWORD_HASH=$(docker compose -p "$COMPOSE_PROJECT_NAME" run --rm --no-deps app hash-password "$E2E_PASSWORD" | tail -n 1)
export AUTH_PASSWORD_HASH
docker compose -p "$COMPOSE_PROJECT_NAME" up -d --wait
export FILATURE_E2E_BASE_URL="http://127.0.0.1:$APP_PORT" FILATURE_E2E_USERNAME="$AUTH_USERNAME" FILATURE_E2E_PASSWORD="$E2E_PASSWORD"
cd "$E2E_DIR"
npx playwright test
