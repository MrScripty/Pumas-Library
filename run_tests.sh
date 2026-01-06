#!/bin/bash
# Test Runner Script for Backend Unit Tests
# Run all Phase 1 tests and generate coverage report

set -e  # Exit on error

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

echo "========================================"
echo "Backend Unit Test Runner - Phase 1"
echo "========================================"
echo ""

# Activate virtual environment
if [ -d "venv" ]; then
    echo "✓ Activating virtual environment..."
    source venv/bin/activate
else
    echo "✗ Virtual environment not found!"
    echo "  Please create one: python3 -m venv venv"
    exit 1
fi

# Install test dependencies if needed
echo ""
echo "Checking test dependencies..."
pip install -q pytest pytest-cov pytest-mock freezegun 2>/dev/null || true

echo ""
echo "========================================"
echo "Running Phase 1 Unit Tests"
echo "========================================"
echo ""

# Test 1: Dependency Manager (NEW)
echo "[ 1/4 ] Testing dependency_manager.py..."
python -m pytest backend/tests/test_dependency_manager.py -v --tb=short

# Test 2: Patch Manager (NEW)
echo ""
echo "[ 2/4 ] Testing patch_manager.py..."
python -m pytest backend/tests/test_patch_manager.py -v --tb=short

# Test 3: Process Manager Extended
echo ""
echo "[ 3/4 ] Testing process_manager_extended.py..."
python -m pytest backend/tests/test_process_manager_extended.py -v --tb=short

# Test 4: System Utils Extended
echo ""
echo "[ 4/4 ] Testing system_utils_extended.py..."
python -m pytest backend/tests/test_system_utils_extended.py -v --tb=short

echo ""
echo "========================================"
echo "Running All Tests with Coverage"
echo "========================================"
echo ""

python -m pytest backend/tests/ \
    --cov=backend/api \
    --cov-report=term-missing \
    --cov-report=html \
    -v

echo ""
echo "========================================"
echo "✓ All Tests Complete!"
echo "========================================"
echo ""
echo "Coverage report generated:"
echo "  - Terminal: See above"
echo "  - HTML: htmlcov/index.html"
echo ""
echo "Next steps:"
echo "  1. Review coverage report: open htmlcov/index.html"
echo "  2. Merge extended test files into main test files"
echo "  3. Proceed with Phase 2 (JavaScriptAPI tests)"
echo ""
