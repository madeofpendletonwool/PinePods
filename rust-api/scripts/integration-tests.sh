#!/usr/bin/env bash
# Run the rust-api integration suite (tests/integration) against real databases.
#
#   scripts/integration-tests.sh                 # Postgres, MariaDB and SQLite, all tests
#   scripts/integration-tests.sh --db postgres   # one backend only (postgres | mariadb | sqlite)
#   scripts/integration-tests.sh -- queue        # extra args go to `cargo test` (name filter, etc.)
#   scripts/integration-tests.sh --gpodder mock -- gpodder
#                                                # gpodder protocol tests against rust | mock
#                                                # (default rust)
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

DBS=(postgresql mariadb sqlite)
CARGO_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --db)
      case "${2:-}" in
        postgres|postgresql) DBS=(postgresql) ;;
        mariadb|mysql) DBS=(mariadb) ;;
        sqlite) DBS=(sqlite) ;;
        all) DBS=(postgresql mariadb sqlite) ;;
        *) echo "unknown --db '${2:-}' (postgres | mariadb | sqlite | all)" >&2; exit 2 ;;
      esac
      shift 2 ;;
    --gpodder)
      case "${2:-}" in
        rust|mock) export PINEPODS_TEST_GPODDER="$2" ;;
        *) echo "unknown --gpodder '${2:-}' (rust | mock)" >&2; exit 2 ;;
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

# Backup tests run the real dump tools. Where they aren't installed, run the ones in the test
# containers instead, translating addresses and output files.
install_db_tool_shims() {
  local dir="$API_DIR/target/db-tools" compose_file="$API_DIR/tests/docker-compose.yml"
  mkdir -p "$dir"
  local tool service binary
  for tool in pg_dump psql mysqldump mysql; do
    command -v "$tool" >/dev/null && continue
    case "$tool" in
      pg_dump|psql) service=postgres binary=$tool ;;
      mysqldump) service=mariadb binary=mariadb-dump ;;
      mysql) service=mariadb binary=mariadb ;;
    esac
    cat > "$dir/$tool" <<SHIM
#!/usr/bin/env bash
args=(); out=""; prev=""
for a in "\$@"; do
  case "\$prev" in
    -h|--host) args+=("127.0.0.1"); prev=""; continue ;;
    -p|-P|--port) if [[ "$service" == postgres ]]; then args+=("5432"); else args+=("3306"); fi; prev=""; continue ;;
    -f|--result-file) out="\$a"; args=("\${args[@]:0:\${#args[@]}-1}"); prev=""; continue ;;
  esac
  case "\$a" in
    --ssl-verify-server-cert=0|--default-auth=*) prev=""; continue ;;
  esac
  args+=("\$a"); prev="\$a"
done
run=(docker compose -f "$compose_file" exec -T -e "PGPASSWORD=\${PGPASSWORD:-}" "$service" "$binary" "\${args[@]}")
case "$tool" in
  pg_dump|mysqldump) exec </dev/null ;;
esac
if [[ -n "\$out" ]]; then "\${run[@]}" > "\$out"; else "\${run[@]}"; fi
SHIM
    chmod +x "$dir/$tool"
  done
  export PATH="$PATH:$dir"
}
if [[ -z "${PINEPODS_TEST_EXTERNAL_SERVICES:-}" ]]; then
  install_db_tool_shims
fi

cd "$API_DIR"
status=0
for db in "${DBS[@]}"; do
  echo "=== integration tests: $db ==="
  PINEPODS_TEST_DB_TYPE="$db" cargo test --test integration ${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"} || status=$?
done
exit $status
