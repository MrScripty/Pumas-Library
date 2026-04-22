#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/dev/check-commit-message.sh <commit-message-file>

Validates the first non-comment line of a commit message against the project
conventional commit format:

  <type>(<scope>): <description>

Allowed types: feat, fix, refactor, chore, docs, style, test, perf, ci
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ "$#" -ne 1 || ! -f "$1" ]]; then
  usage >&2
  exit 2
fi

message_file="$1"
subject=$(
  awk '!/^[[:space:]]*(#|$)/ { print; exit }' "${message_file}" |
    sed 's/[[:space:]]*$//'
)

if [[ -z "${subject}" ]]; then
  printf 'Commit message subject is empty.\n' >&2
  usage >&2
  exit 1
fi

candidate="${subject}"

for prefix in "fixup! " "squash! " "amend! "; do
  if [[ "${candidate}" == "${prefix}"* ]]; then
    candidate="${candidate#"${prefix}"}"
    break
  fi
done

if (( ${#candidate} > 72 )); then
  printf 'Commit message subject exceeds 72 characters:\n  %s\n' "${subject}" >&2
  exit 1
fi

pattern='^(feat|fix|refactor|chore|docs|style|test|perf|ci)(\([a-z0-9][a-z0-9-]*\))?!?: [[:lower:][:digit:]][[:print:]]*$'

if [[ ! "${candidate}" =~ ${pattern} ]]; then
  cat >&2 <<'ERROR'
Commit message subject must use conventional commit format:
  <type>(<scope>): <description>

Allowed types: feat, fix, refactor, chore, docs, style, test, perf, ci
Scope is optional, lowercase, and hyphenated when needed.
Description should be imperative and start lowercase.
ERROR
  printf '\nReceived:\n  %s\n' "${subject}" >&2
  exit 1
fi
