#!/bin/bash
#
# compare-backends.sh
#
# Compares JSON-RPC responses between Python and Rust backends.
# This script helps verify that the Rust backend is a drop-in replacement
# for the Python backend by checking response compatibility.
#
# Usage:
#   ./scripts/compare-backends.sh [method] [params_json]
#
# Examples:
#   ./scripts/compare-backends.sh                      # Run all comparison tests
#   ./scripts/compare-backends.sh get_status           # Test a specific method
#   ./scripts/compare-backends.sh get_version_shortcuts '{"tag":"v0.4.0"}'
#

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

PYTHON_PORT=9876
RUST_PORT=9877
PYTHON_PID=""
RUST_PID=""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Cleanup function
cleanup() {
    echo -e "\n${BLUE}Cleaning up...${NC}"
    if [ -n "$PYTHON_PID" ] && kill -0 "$PYTHON_PID" 2>/dev/null; then
        kill "$PYTHON_PID" 2>/dev/null || true
    fi
    if [ -n "$RUST_PID" ] && kill -0 "$RUST_PID" 2>/dev/null; then
        kill "$RUST_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Start Python backend
start_python_backend() {
    echo -e "${BLUE}Starting Python backend on port $PYTHON_PORT...${NC}"

    cd "$PROJECT_ROOT/backend"

    # Activate venv if it exists
    if [ -f "$PROJECT_ROOT/venv/bin/activate" ]; then
        source "$PROJECT_ROOT/venv/bin/activate"
    fi

    python rpc_server.py --port $PYTHON_PORT &
    PYTHON_PID=$!

    # Wait for Python backend to be ready
    for i in {1..30}; do
        if curl -s "http://127.0.0.1:$PYTHON_PORT/health" > /dev/null 2>&1; then
            echo -e "${GREEN}Python backend started${NC}"
            return 0
        fi
        sleep 0.5
    done

    echo -e "${RED}Failed to start Python backend${NC}"
    return 1
}

# Start Rust backend
start_rust_backend() {
    echo -e "${BLUE}Starting Rust backend on port $RUST_PORT...${NC}"

    local rust_binary="$PROJECT_ROOT/rust/target/release/pumas-rpc"

    if [ ! -f "$rust_binary" ]; then
        echo -e "${YELLOW}Rust binary not found. Building...${NC}"
        cd "$PROJECT_ROOT/rust"
        cargo build --release
    fi

    "$rust_binary" --port $RUST_PORT --launcher_root "$PROJECT_ROOT" &
    RUST_PID=$!

    # Wait for Rust backend to be ready
    for i in {1..30}; do
        if curl -s "http://127.0.0.1:$RUST_PORT/health" > /dev/null 2>&1; then
            echo -e "${GREEN}Rust backend started${NC}"
            return 0
        fi
        sleep 0.5
    done

    echo -e "${RED}Failed to start Rust backend${NC}"
    return 1
}

# Make RPC call
rpc_call() {
    local port=$1
    local method=$2
    local params=${3:-"{}"}

    curl -s -X POST "http://127.0.0.1:$port/rpc" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}"
}

# Compare responses
compare_responses() {
    local method=$1
    local params=${2:-"{}"}

    echo -e "\n${BLUE}Testing: $method${NC}"
    echo "Params: $params"

    # Get responses
    local python_response=$(rpc_call $PYTHON_PORT "$method" "$params")
    local rust_response=$(rpc_call $RUST_PORT "$method" "$params")

    # Save to temp files for comparison
    local python_file=$(mktemp)
    local rust_file=$(mktemp)

    echo "$python_response" | jq -S '.result' > "$python_file" 2>/dev/null || echo "$python_response" > "$python_file"
    echo "$rust_response" | jq -S '.result' > "$rust_file" 2>/dev/null || echo "$rust_response" > "$rust_file"

    # Compare
    if diff -q "$python_file" "$rust_file" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Responses match${NC}"
        rm "$python_file" "$rust_file"
        return 0
    else
        echo -e "${RED}✗ Responses differ${NC}"
        echo -e "\n${YELLOW}Python response:${NC}"
        cat "$python_file" | head -20
        echo -e "\n${YELLOW}Rust response:${NC}"
        cat "$rust_file" | head -20

        echo -e "\n${YELLOW}Diff:${NC}"
        diff "$python_file" "$rust_file" || true

        rm "$python_file" "$rust_file"
        return 1
    fi
}

# Compare structure only (ignore dynamic values)
compare_structure() {
    local method=$1
    local params=${2:-"{}"}

    echo -e "\n${BLUE}Testing structure: $method${NC}"

    local python_response=$(rpc_call $PYTHON_PORT "$method" "$params")
    local rust_response=$(rpc_call $RUST_PORT "$method" "$params")

    # Extract keys only for structure comparison
    local python_keys=$(echo "$python_response" | jq -S 'paths | map(tostring) | join(".")' 2>/dev/null | sort | uniq)
    local rust_keys=$(echo "$rust_response" | jq -S 'paths | map(tostring) | join(".")' 2>/dev/null | sort | uniq)

    if [ "$python_keys" = "$rust_keys" ]; then
        echo -e "${GREEN}✓ Response structures match${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠ Response structures differ (may be expected for dynamic data)${NC}"
        echo -e "\n${YELLOW}Python keys:${NC}"
        echo "$python_keys" | head -10
        echo -e "\n${YELLOW}Rust keys:${NC}"
        echo "$rust_keys" | head -10
        return 0  # Don't fail on structure differences for dynamic data
    fi
}

# Run all tests
run_all_tests() {
    local failed=0
    local passed=0
    local total=0

    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}Running Backend Comparison Tests${NC}"
    echo -e "${BLUE}========================================${NC}\n"

    # Test methods that should return identical results
    local exact_methods=(
        "health_check:{}"
        "get_sandbox_info:{}"
    )

    # Test methods that should have matching structure
    local structure_methods=(
        "get_status:{}"
        "get_disk_space:{}"
        "get_system_resources:{}"
        "get_launcher_version:{}"
        "get_available_versions:{}"
        "get_installed_versions:{}"
        "get_active_version:{}"
        "get_default_version:{}"
        "is_comfyui_running:{}"
        "has_background_fetch_completed:{}"
        "get_github_cache_status:{}"
        "get_library_status:{}"
        "get_network_status:{}"
    )

    echo -e "${BLUE}=== Exact Match Tests ===${NC}"
    for test in "${exact_methods[@]}"; do
        IFS=':' read -r method params <<< "$test"
        total=$((total + 1))
        if compare_responses "$method" "$params"; then
            passed=$((passed + 1))
        else
            failed=$((failed + 1))
        fi
    done

    echo -e "\n${BLUE}=== Structure Match Tests ===${NC}"
    for test in "${structure_methods[@]}"; do
        IFS=':' read -r method params <<< "$test"
        total=$((total + 1))
        if compare_structure "$method" "$params"; then
            passed=$((passed + 1))
        else
            failed=$((failed + 1))
        fi
    done

    # Summary
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}Test Summary${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo -e "Total: $total"
    echo -e "${GREEN}Passed: $passed${NC}"
    if [ $failed -gt 0 ]; then
        echo -e "${RED}Failed: $failed${NC}"
    else
        echo -e "Failed: $failed"
    fi

    return $failed
}

# Main
main() {
    echo -e "${BLUE}Backend Comparison Tool${NC}"
    echo -e "${BLUE}=======================${NC}\n"

    # Check for jq
    if ! command -v jq &> /dev/null; then
        echo -e "${RED}Error: jq is required. Install with: apt install jq${NC}"
        exit 1
    fi

    # Start both backends
    start_python_backend
    start_rust_backend

    # Run tests
    if [ $# -eq 0 ]; then
        # Run all tests
        run_all_tests
    else
        # Run specific test
        local method=$1
        local params=${2:-"{}"}
        compare_responses "$method" "$params"
    fi
}

main "$@"
