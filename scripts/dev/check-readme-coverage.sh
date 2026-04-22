#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)

usage() {
  cat <<'USAGE'
Usage: scripts/dev/check-readme-coverage.sh

Checks standards-controlled source and support directories for README.md files.
Generated dependency, cache, and build output directories are skipped.
USAGE
}

for arg in "$@"; do
  case "${arg}" in
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

roots=(
  "frontend/src"
  "electron/src"
  "rust"
  "torch-server"
  "scripts"
  "bindings"
  "launcher-data/plugins"
)

prune_paths=(
  "*/.pytest_cache"
  "*/__pycache__"
  "*/.venv"
  "*/dist"
  "*/node_modules"
  "*/release"
  "*/target"
)

missing=()

cd "${REPO_ROOT}"

for root in "${roots[@]}"; do
  if [[ ! -d "${root}" ]]; then
    continue
  fi

  find_args=("${root}" "(")
  for index in "${!prune_paths[@]}"; do
    if [[ "${index}" != "0" ]]; then
      find_args+=("-o")
    fi
    find_args+=("-path" "${prune_paths[$index]}")
  done
  find_args+=(")" "-prune" "-o" "-type" "d" "-print0")

  while IFS= read -r -d '' directory; do
    if [[ ! -f "${directory}/README.md" ]]; then
      missing+=("${directory}")
    fi
  done < <(find "${find_args[@]}")
done

if (( ${#missing[@]} > 0 )); then
  printf 'Missing README.md for standards-controlled directories:\n' >&2
  printf '  %s\n' "${missing[@]}" >&2
  exit 1
fi

printf 'README coverage check passed for standards-controlled directories.\n'
