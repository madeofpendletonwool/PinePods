#!/bin/bash
#
# upgrade-postgres.sh — Upgrade a Pinepods PostgreSQL data directory to a new
# major version (e.g. 17 -> 18) using the drop-in pgautoupgrade image.
#
# A major PostgreSQL version uses an incompatible on-disk format, so postgres:18
# refuses to start against a data directory created by postgres:17. This script:
#   1. Takes a safety pg_dumpall backup of your existing data.
#   2. Runs a one-shot pgautoupgrade container that upgrades the directory in place.
#   3. Tells you to switch your compose `db` image to the new version.
#
# This is one-way: once upgraded you cannot run the OLD major version against the
# same directory again. Keep the backup this script makes.
#
set -euo pipefail

# ---- Defaults (match the shipped Pinepods compose files) --------------------
VOLUME=""                                   # host path OR docker named volume holding pgdata
DB_NAME="pinepods_database"
DB_USER="postgres"
DB_PASSWORD="myS3curepass"
PGDATA="/var/lib/postgresql/data/pgdata"
OLD_IMAGE="postgres:17"                      # image used for the safety dump (current version)
TARGET_IMAGE="pgautoupgrade/pgautoupgrade:18-trixie"  # Debian variant to match postgres:18
NEW_IMAGE="postgres:18"                      # what you switch your compose to afterwards
BACKUP_DIR="./backups"
DB_CONTAINER="db"                            # name of your running db container (to verify it's stopped)
ASSUME_YES="no"

usage() {
    cat <<EOF
Upgrade a Pinepods PostgreSQL data directory to a new major version.

Usage: $0 --volume <pgdata-volume> [options]

Required:
  --volume <v>        Host path (e.g. /home/user/pinepods/pgdata) OR docker named
                      volume (e.g. pinepods_pgdata) that holds your PostgreSQL data.
                      This is the LEFT side of the db service's data volume mapping.

Options (defaults match the shipped compose files):
  --db <name>         Database name              (default: $DB_NAME)
  --user <name>       Database user              (default: $DB_USER)
  --password <pass>   Database password          (default: $DB_PASSWORD)
  --pgdata <path>     PGDATA inside container    (default: $PGDATA)
  --old-image <img>   Current postgres image     (default: $OLD_IMAGE)
  --target-image <img> pgautoupgrade image       (default: $TARGET_IMAGE)
  --new-image <img>   Image to switch to after   (default: $NEW_IMAGE)
  --backup-dir <dir>  Where to write the dump    (default: $BACKUP_DIR)
  --container <name>  db container name to check is stopped (default: $DB_CONTAINER)
  -y, --yes           Don't prompt for confirmation
  -h, --help          Show this help

Example:
  $0 --volume /home/user/pinepods/pgdata --password myS3curepass

Before running: stop your stack with 'docker compose down'.
EOF
}

# ---- Parse args -------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --volume)       VOLUME="$2"; shift 2 ;;
        --db)           DB_NAME="$2"; shift 2 ;;
        --user)         DB_USER="$2"; shift 2 ;;
        --password)     DB_PASSWORD="$2"; shift 2 ;;
        --pgdata)       PGDATA="$2"; shift 2 ;;
        --old-image)    OLD_IMAGE="$2"; shift 2 ;;
        --target-image) TARGET_IMAGE="$2"; shift 2 ;;
        --new-image)    NEW_IMAGE="$2"; shift 2 ;;
        --backup-dir)   BACKUP_DIR="$2"; shift 2 ;;
        --container)    DB_CONTAINER="$2"; shift 2 ;;
        -y|--yes)       ASSUME_YES="yes"; shift ;;
        -h|--help)      usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; echo; usage; exit 1 ;;
    esac
done

if [[ -z "$VOLUME" ]]; then
    echo "ERROR: --volume is required." >&2
    echo
    usage
    exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
    echo "ERROR: docker is not installed or not on PATH." >&2
    exit 1
fi

# ---- Pre-flight: db container must be stopped -------------------------------
if docker ps --format '{{.Names}}' | grep -qx "$DB_CONTAINER"; then
    echo "ERROR: container '$DB_CONTAINER' is still running." >&2
    echo "Stop your stack first:  docker compose down" >&2
    exit 1
fi

TMP_DUMP_CTR="pinepods-pg-dump-tmp"
UPGRADE_CTR="pinepods-pg-upgrade"

cleanup() {
    docker rm -f "$TMP_DUMP_CTR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "==> Upgrade plan"
echo "    volume:        $VOLUME"
echo "    pgdata:        $PGDATA"
echo "    database:      $DB_NAME (user: $DB_USER)"
echo "    safety dump:   $OLD_IMAGE -> $BACKUP_DIR"
echo "    upgrade image: $TARGET_IMAGE"
echo "    after upgrade: switch your compose 'db' image to $NEW_IMAGE"
echo
echo "    This modifies the data directory in place and is NOT reversible."
echo
if [[ "$ASSUME_YES" != "yes" ]]; then
    read -r -p "Continue? [y/N] " reply
    case "$reply" in
        y|Y|yes|YES) ;;
        *) echo "Aborted."; exit 1 ;;
    esac
fi

# ---- Step 1: safety backup via a temporary old-version server ---------------
mkdir -p "$BACKUP_DIR"
TS="$(date +%Y%m%d-%H%M%S)"
BACKUP_FILE="$BACKUP_DIR/pre-pg18-$TS.sql"

echo
echo "==> [1/3] Taking safety backup with $OLD_IMAGE ..."
docker rm -f "$TMP_DUMP_CTR" >/dev/null 2>&1 || true
docker run -d --name "$TMP_DUMP_CTR" \
    -e POSTGRES_PASSWORD="$DB_PASSWORD" \
    -e PGDATA="$PGDATA" \
    -v "$VOLUME":/var/lib/postgresql/data \
    "$OLD_IMAGE" >/dev/null

echo "    waiting for the temporary server to become ready ..."
ready="no"
for _ in $(seq 1 60); do
    if docker exec "$TMP_DUMP_CTR" pg_isready -U "$DB_USER" >/dev/null 2>&1; then
        ready="yes"
        break
    fi
    sleep 1
done

if [[ "$ready" != "yes" ]]; then
    echo "ERROR: temporary $OLD_IMAGE server did not become ready. Logs:" >&2
    docker logs "$TMP_DUMP_CTR" >&2 || true
    exit 1
fi

echo "    dumping all databases to $BACKUP_FILE ..."
if ! docker exec -e PGPASSWORD="$DB_PASSWORD" "$TMP_DUMP_CTR" \
        pg_dumpall -U "$DB_USER" > "$BACKUP_FILE"; then
    echo "ERROR: pg_dumpall failed. Aborting before any changes." >&2
    exit 1
fi

if [[ ! -s "$BACKUP_FILE" ]]; then
    echo "ERROR: backup file is empty. Aborting before any changes." >&2
    exit 1
fi
echo "    backup OK: $BACKUP_FILE ($(wc -c < "$BACKUP_FILE") bytes)"

echo "    stopping temporary server ..."
docker rm -f "$TMP_DUMP_CTR" >/dev/null 2>&1 || true

# ---- Step 2: one-shot in-place upgrade -------------------------------------
echo
echo "==> [2/3] Upgrading data directory in place with $TARGET_IMAGE ..."
docker rm -f "$UPGRADE_CTR" >/dev/null 2>&1 || true
if ! docker run --rm --name "$UPGRADE_CTR" \
        -e POSTGRES_DB="$DB_NAME" \
        -e POSTGRES_USER="$DB_USER" \
        -e POSTGRES_PASSWORD="$DB_PASSWORD" \
        -e PGDATA="$PGDATA" \
        -e PGAUTO_ONESHOT=yes \
        -v "$VOLUME":/var/lib/postgresql/data \
        "$TARGET_IMAGE"; then
    echo >&2
    echo "ERROR: pgautoupgrade failed. Your data directory may be unchanged or partially" >&2
    echo "upgraded. Restore from the backup at: $BACKUP_FILE" >&2
    exit 1
fi

# ---- Step 3: next steps -----------------------------------------------------
echo
echo "==> [3/3] Upgrade complete."
cat <<EOF

Your PostgreSQL data directory is now upgraded.

Next steps:
  1. Edit your compose file and set the db service image to:
         image: $NEW_IMAGE
     (remove any PGAUTO_ONESHOT line if you added one)
  2. Start the stack:
         docker compose up -d
  3. The new image likely ships a newer glibc, so the db will warn about a
     "collation version mismatch". Rebuild indexes on your database, then clear
     the version flag on every database that warns:
         docker compose exec db psql -U $DB_USER -d $DB_NAME \\
           -c "REINDEX DATABASE $DB_NAME;"
         docker compose exec db psql -U $DB_USER -d $DB_NAME \\
           -c "ALTER DATABASE $DB_NAME REFRESH COLLATION VERSION;"
         docker compose exec db psql -U $DB_USER -d postgres \\
           -c "ALTER DATABASE postgres REFRESH COLLATION VERSION;"
         docker compose exec db psql -U $DB_USER -d template1 \\
           -c "ALTER DATABASE template1 REFRESH COLLATION VERSION;"

Safety backup kept at: $BACKUP_FILE
Delete it only after you've confirmed Pinepods works on the new version.
EOF
