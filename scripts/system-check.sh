#!/usr/bin/env bash
#
# System Dependency Checker for Linux ComfyUI Launcher
# Checks for required system packages and provides installation guidance
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track missing dependencies
MISSING_DEPS=()
MISSING_APT_PACKAGES=()

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}System Dependency Check${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Function to check command existence
check_command() {
    local cmd=$1
    local package=$2
    local apt_package=$3

    if command -v "$cmd" &> /dev/null; then
        echo -e "${GREEN}✓${NC} $package found: $(command -v "$cmd")"
        return 0
    else
        echo -e "${RED}✗${NC} $package not found"
        MISSING_DEPS+=("$package")
        if [ -n "$apt_package" ]; then
            MISSING_APT_PACKAGES+=("$apt_package")
        fi
        return 1
    fi
}

# Function to check Python version
check_python_version() {
    echo -e "${BLUE}Checking Python 3.12+...${NC}"

    if command -v python3.12 &> /dev/null; then
        local version=$(python3.12 --version 2>&1 | awk '{print $2}')
        echo -e "${GREEN}✓${NC} Python 3.12 found: $version"
        return 0
    elif command -v python3 &> /dev/null; then
        local version=$(python3 --version 2>&1 | awk '{print $2}')
        local major=$(echo "$version" | cut -d. -f1)
        local minor=$(echo "$version" | cut -d. -f2)

        if [ "$major" -eq 3 ] && [ "$minor" -ge 12 ]; then
            echo -e "${GREEN}✓${NC} Python $version found (>= 3.12)"
            return 0
        else
            echo -e "${RED}✗${NC} Python $version found (< 3.12 required)"
            MISSING_DEPS+=("Python 3.12+")
            MISSING_APT_PACKAGES+=("python3.12")
            return 1
        fi
    else
        echo -e "${RED}✗${NC} Python 3.12+ not found"
        MISSING_DEPS+=("Python 3.12+")
        MISSING_APT_PACKAGES+=("python3.12")
        return 1
    fi
}

# Function to check for dpkg package
check_dpkg_package() {
    local package=$1
    local display_name=$2

    if dpkg -l 2>/dev/null | grep -q "^ii  $package"; then
        echo -e "${GREEN}✓${NC} $display_name installed"
        return 0
    else
        echo -e "${RED}✗${NC} $display_name not installed"
        MISSING_DEPS+=("$display_name")
        MISSING_APT_PACKAGES+=("$package")
        return 1
    fi
}

echo -e "${BLUE}[1/4] Checking Python...${NC}"
check_python_version

# Check for python3.12-venv
if command -v python3.12 &> /dev/null || command -v python3 &> /dev/null; then
    check_dpkg_package "python3.12-venv" "Python 3.12 venv module" || \
    check_dpkg_package "python3-venv" "Python venv module" || true
fi

echo ""
echo -e "${BLUE}[2/4] Checking Node.js and npm...${NC}"
check_command "node" "Node.js" "nodejs"
check_command "npm" "npm" "npm"

if command -v node &> /dev/null; then
    local node_version=$(node --version)
    echo -e "  Node.js version: $node_version"
fi

echo ""
echo -e "${BLUE}[3/4] Checking GTK3 and WebKitGTK...${NC}"
check_dpkg_package "libgtk-3-0" "GTK3"

# Check for WebKitGTK (try both 4.0 and 4.1 versions)
if ! check_dpkg_package "libwebkit2gtk-4.1-0" "WebKitGTK 4.1"; then
    check_dpkg_package "libwebkit2gtk-4.0-37" "WebKitGTK 4.0" || true
fi

if ! check_dpkg_package "gir1.2-webkit2-4.1" "WebKit2 GObject Introspection 4.1"; then
    check_dpkg_package "gir1.2-webkit2-4.0" "WebKit2 GObject Introspection 4.0" || true
fi

echo ""
echo -e "${BLUE}[4/4] Checking Python GTK bindings...${NC}"
check_dpkg_package "python3-gi" "Python GObject Introspection"
check_dpkg_package "gir1.2-gtk-3.0" "GTK 3.0 GObject Introspection"

echo ""
echo -e "${BLUE}========================================${NC}"

# Summary
if [ ${#MISSING_DEPS[@]} -eq 0 ]; then
    echo -e "${GREEN}All dependencies are installed!${NC}"
    echo -e "${BLUE}========================================${NC}"
    exit 0
else
    echo -e "${YELLOW}Missing ${#MISSING_DEPS[@]} dependencies:${NC}"
    for dep in "${MISSING_DEPS[@]}"; do
        echo -e "  ${RED}•${NC} $dep"
    done
    echo ""

    if [ ${#MISSING_APT_PACKAGES[@]} -gt 0 ]; then
        echo -e "${BLUE}To install missing dependencies, run:${NC}"
        echo -e "${YELLOW}sudo apt update && sudo apt install -y ${MISSING_APT_PACKAGES[*]}${NC}"
    fi

    echo -e "${BLUE}========================================${NC}"
    exit 1
fi
