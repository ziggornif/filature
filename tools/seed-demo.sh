#!/usr/bin/env bash
#
# ⚠️  DESTRUCTIF — Importe le dataset de capture puis recrée ses Machine Links
# simulés. L'import est un REMPLACEMENT COMPLET (`confirm_replace=yes`) : toutes
# les bobines, imprimantes, rangements et Machine Links de l'instance visée sont
# écrasés. À réserver à une instance de démo jetable.
#
# Deux garde-fous, parce qu'une inversion de `FILATURE_URL` suffirait sinon à
# détruire une instance réelle :
#   1. l'instance visée doit être locale (localhost / 127.0.0.1 / ::1) ;
#   2. `--yes` doit être passé explicitement.
#
# Usage : tools/seed-demo.sh --yes
# Prérequis : instance compilée avec `demo-stub`, curl, psql et clé credentials.
set -euo pipefail

FILATURE_URL="${FILATURE_URL:-http://127.0.0.1:8080}"
FILATURE_USER="${FILATURE_USER:-demo}"
FILATURE_PASSWORD="${FILATURE_PASSWORD:-demo}"
DATABASE_URL="${DATABASE_URL:-postgres://filature:filature@127.0.0.1:5432/filature}"
FILATURE_BIN="${FILATURE_BIN:-target/debug/filature}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
COOKIE_JAR="$(mktemp)"
trap 'rm -f -- "$COOKIE_JAR"' EXIT

if [[ "${1:-}" != "--yes" ]]; then
  echo "Ce script REMPLACE tout le contenu de l'instance $FILATURE_URL." >&2
  echo "Relancer avec --yes pour confirmer : tools/seed-demo.sh --yes" >&2
  exit 2
fi
# L'hôte est extrait de l'URL (schéma et port retirés) : un `FILATURE_URL`
# pointant ailleurs que sur la machine locale est refusé, pas juste averti.
seed_host="${FILATURE_URL#*://}"
seed_host="${seed_host%%/*}"
seed_host="${seed_host%%:[0-9]*}"
case "$seed_host" in
  localhost | 127.0.0.1 | '[::1]' | ::1) ;;
  *)
    echo "Refus : $FILATURE_URL n'est pas une instance locale (hôte '$seed_host')." >&2
    echo "Ce script est destructif et réservé à une instance de démo locale." >&2
    exit 2
    ;;
esac

if [[ -z "${FILATURE_CREDENTIALS_KEY:-}" ]]; then
  echo "FILATURE_CREDENTIALS_KEY doit être définie." >&2
  exit 2
fi
if [[ ! -x "$FILATURE_BIN" ]]; then
  echo "Binaire introuvable: $FILATURE_BIN (compiler avec --features demo-stub)." >&2
  exit 2
fi

curl --fail --silent --show-error \
  --cookie-jar "$COOKIE_JAR" \
  --data-urlencode "username=$FILATURE_USER" \
  --data-urlencode "password=$FILATURE_PASSWORD" \
  "$FILATURE_URL/login" >/dev/null
curl --fail --silent --show-error \
  --cookie "$COOKIE_JAR" \
  --form "confirm_replace=yes" \
  --form "backup=@$SCRIPT_DIR/demo-instance.json;type=application/json" \
  "$FILATURE_URL/settings/import" >/dev/null

printer_ids=(
  01KXPB0RJFEK6A2FZTP900VT9T 01KXPB0RM6NTG2JM083TC7W7FK
  01KXPB0RP0AW0FN5H6KZ3HV047 01KXPB0RQF4JSM2J0DXWJ735YF
  01KXPB0RRSEWBJ6MMVDZDVYN7W 01KXPB0RT0ZFWRWHR3H89E586A
  01KXPB0RV93GKGAHCM73K67XM3 01KXPB0RWFG622WZS7KTZK1VYV
)
for printer_id in "${printer_ids[@]}"; do
  # `psql -c` n'interpole PAS les variables `--set` : la requête passe par
  # l'entrée standard, seul chemin où la substitution `:'var'` s'applique.
  found="$(psql "$DATABASE_URL" -XAt --set=printer_id="$printer_id" \
    <<<"SELECT count(*) FROM printers WHERE id = :'printer_id'")"
  if [[ "$found" != "1" ]]; then
    echo "Imprimante attendue absente après import: $printer_id" >&2
    exit 1
  fi
done

bambu_credential="$("$FILATURE_BIN" encrypt-credential 12345678)"
prusa_credential="$("$FILATURE_BIN" encrypt-credential demo-api-key)"
psql "$DATABASE_URL" -X --set=ON_ERROR_STOP=1 \
  --set=bambu_credential="$bambu_credential" \
  --set=prusa_credential="$prusa_credential" <<'SQL'
INSERT INTO machine_links (printer_id, kind, endpoint, credential) VALUES
('01KXPB0RJFEK6A2FZTP900VT9T','bambu','{"host":"192.168.1.31","serial":"01P00A3B0500123"}', :'bambu_credential'),
('01KXPB0RM6NTG2JM083TC7W7FK','bambu','{"host":"192.168.1.32","serial":"01H00C7D0900456"}', :'bambu_credential'),
('01KXPB0RP0AW0FN5H6KZ3HV047','bambu','{"host":"192.168.1.33","serial":"01X00E1F0300789"}', :'bambu_credential'),
('01KXPB0RQF4JSM2J0DXWJ735YF','bambu','{"host":"192.168.1.34","serial":"01H00C2A0700321"}', :'bambu_credential'),
('01KXPB0RRSEWBJ6MMVDZDVYN7W','prusalink','192.168.1.42', :'prusa_credential'),
('01KXPB0RT0ZFWRWHR3H89E586A','prusalink','192.168.1.43', :'prusa_credential'),
('01KXPB0RV93GKGAHCM73K67XM3','prusalink','192.168.1.44', :'prusa_credential'),
('01KXPB0RWFG622WZS7KTZK1VYV','moonraker','http://192.168.1.51:7125', NULL)
ON CONFLICT (printer_id) DO UPDATE SET
  kind = EXCLUDED.kind,
  endpoint = EXCLUDED.endpoint,
  credential = EXCLUDED.credential;
SQL
