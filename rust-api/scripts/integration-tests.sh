#!/usr/bin/env bash
# Run the rust-api integration suite (tests/integration) against real databases.
#
#   scripts/integration-tests.sh                 # Postgres + MariaDB, all tests
#   scripts/integration-tests.sh --db postgres   # one backend only (postgres | mariadb)
#   scripts/integration-tests.sh -- queue        # extra args go to `cargo test` (name filter, etc.)
#   scripts/integration-tests.sh down            # stop the throwaway containers
#
# Containers come from tests/docker-compose.yml (off-default ports, tmpfs data). A Python venv
# under target/ holds the deps needed to run the real schema migrations into a template DB.
set -euo pipefail

API_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE=(docker compose -f "$API_DIR/tests/docker-compose.yml")

if [[ "${1:-}" == "down" ]]; then
  "${COMPOSE[@]}" down -v
  exit 0
fi

DBS=(postgresql mariadb)
CARGO_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --db)
      case "${2:-}" in
        postgres|postgresql) DBS=(postgresql) ;;
        mariadb|mysql) DBS=(mariadb) ;;
        all) DBS=(postgresql mariadb) ;;
        *) echo "unknown --db '${2:-}' (postgres | mariadb | all)" >&2; exit 2 ;;
      esac
      shift 2 ;;
    --) shift; CARGO_ARGS+=("$@"); break ;;
    *) CARGO_ARGS+=("$1"); shift ;;
  esac
done

# Containers are only started if the caller hasn't pointed us at their own (CI uses services).
if [[ -z "${PINEPODS_TEST_EXTERNAL_SERVICES:-}" ]]; then
  "${COMPOSE[@]}" up -d --wait
fi

# Python venv for the migration runner, rebuilt only when requirements change.
if [[ -z "${PINEPODS_TEST_PYTHON:-}" ]]; then
  VENV="$API_DIR/target/integration-venv"
  STAMP="$VENV/.requirements.sha"
  WANT="$(shasum "$API_DIR/tests/requirements.txt" | cut -d' ' -f1)"
  if [[ ! -x "$VENV/bin/python" || "$(cat "$STAMP" 2>/dev/null)" != "$WANT" ]]; then
    python3 -m venv "$VENV"
    "$VENV/bin/pip" install -q --upgrade pip
    "$VENV/bin/pip" install -q -r "$API_DIR/tests/requirements.txt"
    echo "$WANT" > "$STAMP"
  fi
  export PINEPODS_TEST_PYTHON="$VENV/bin/python"
fi

cd "$API_DIR"
status=0
for db in "${DBS[@]}"; do
  echo "=== integration tests: $db ==="
  PINEPODS_TEST_DB_TYPE="$db" cargo test --test integration ${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"} || status=$?
done
exit $status
