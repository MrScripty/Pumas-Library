#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)

usage() {
  cat <<'USAGE'
Usage: scripts/dev/list-audit-files.sh [--tracked-only] [--null]

Prints source-controlled and non-ignored local files that should be included in
standards audits. Generated and runtime-heavy paths are excluded explicitly so
local build output does not pollute line-count, search, or ownership scans.
USAGE
}

tracked_only=false
null_output=false

for arg in "$@"; do
  case "${arg}" in
    --tracked-only)
      tracked_only=true
      ;;
    --null|-z)
      null_output=true
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

git_args=(-c)
if [[ "${tracked_only}" == false ]]; then
  git_args+=(-o --exclude-standard)
fi
if [[ "${null_output}" == true ]]; then
  git_args+=(-z)
fi

pathspecs=(
  ':!:bindings/csharp/generated/**'
  ':!:bindings/kotlin/**'
  ':!:bindings/python/**'
  ':!:bindings/ruby/**'
  ':!:bindings/swift/**'
  ':!:electron/release/**'
  ':!:frontend/dist/**'
  ':!:launcher-data/**'
  ':!:node_modules/**'
  ':!:rust/target/**'
  ':!:torch-server/.venv/**'
)

cd "${REPO_ROOT}"
git ls-files "${git_args[@]}" -- "${pathspecs[@]}"
