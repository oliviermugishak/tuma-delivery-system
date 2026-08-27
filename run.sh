#!/usr/bin/env bash
# Tuma dev runner — one script, one command per thing you run day-to-day.
# New commands land here as slices are built.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Tuma dev runner

Usage: ./run.sh <command> [args]

Commands:
  mobile    Run the Flutter app (flutter run; extra args pass through, e.g. ./run.sh mobile -d linux)
  api       Run the Rust API server (cargo run; needs dev Postgres up, see tuma-server/docker-compose.yml)
  openapi   Export the OpenAPI spec to tuma-platform/openapi.json (for platform client codegen)
  help      Show this help
EOF
}

cmd_mobile() {
  cd "$ROOT_DIR/tuma-app"
  exec flutter run "$@"
}

cmd_api() {
  cd "$ROOT_DIR/tuma-server"
  export APP_ENV="${APP_ENV:-local}"
  exec cargo run "$@"
}

cmd_openapi() {
  cd "$ROOT_DIR/tuma-server"
  exec cargo run --quiet --bin export_openapi -- "$ROOT_DIR/tuma-platform/openapi.json"
}

command="${1:-help}"
shift || true

case "$command" in
  mobile) cmd_mobile "$@" ;;
  api) cmd_api "$@" ;;
  openapi) cmd_openapi "$@" ;;
  help | -h | --help) usage ;;
  *)
    echo "Unknown command: $command" >&2
    usage >&2
    exit 1
    ;;
esac
