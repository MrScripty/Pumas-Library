#!/usr/bin/env bash
# Generate SBOMs for Python and frontend dependencies.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$ROOT_DIR/docs/sbom"

mkdir -p "$OUTPUT_DIR"

if [ ! -x "$ROOT_DIR/venv/bin/cyclonedx-py" ]; then
    echo "Error: venv cyclonedx-py not found. Install via ./venv/bin/python -m pip install cyclonedx-bom"
    exit 1
fi

"$ROOT_DIR/venv/bin/cyclonedx-py" requirements "$ROOT_DIR/requirements-lock.txt" \
    --output-file "$OUTPUT_DIR/sbom-python.json" \
    --output-reproducible

echo "Python SBOM written to $OUTPUT_DIR/sbom-python.json"

export NVM_DIR="$HOME/.nvm"
# shellcheck source=/dev/null
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"

if ! command -v node >/dev/null 2>&1; then
    echo "Error: Node.js not found. Install Node.js 24 LTS via nvm."
    exit 1
fi

NODE_VERSION=$(node --version)
NODE_MAJOR=$(echo "$NODE_VERSION" | sed 's/^v//' | cut -d. -f1)
if [ -n "$NODE_MAJOR" ] && [ "$NODE_MAJOR" -lt 24 ]; then
    echo "Error: Node.js $NODE_VERSION found (< 24 required)."
    exit 1
fi

cd "$ROOT_DIR/frontend"

npx @cyclonedx/cyclonedx-npm --output-file "$OUTPUT_DIR/sbom-frontend.json"

echo "Frontend SBOM written to $OUTPUT_DIR/sbom-frontend.json"
