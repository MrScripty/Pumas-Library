#!/usr/bin/env bash
#
# Linux ComfyUI Launcher - Installation Script
# One-command installation for end users
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
VENV_DIR="$SCRIPT_DIR/venv"
LAUNCHER_SCRIPT="$SCRIPT_DIR/launcher"

echo ""
echo -e "${CYAN}╔════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║  Linux ComfyUI Launcher - Installer   ║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════╝${NC}"
echo ""

# Step 1: Check system dependencies
echo -e "${BLUE}[1/5] Checking system dependencies...${NC}"
echo ""

if [ -f "$SCRIPT_DIR/scripts/system-check.sh" ]; then
    if bash "$SCRIPT_DIR/scripts/system-check.sh"; then
        echo ""
    else
        echo ""
        echo -e "${YELLOW}Missing dependencies detected.${NC}"
        read -p "Install missing dependencies now? (requires sudo) [Y/n]: " -n 1 -r
        echo

        if [[ $REPLY =~ ^[Nn]$ ]]; then
            echo -e "${RED}Installation cancelled. Please install dependencies manually.${NC}"
            exit 1
        fi

        echo -e "${BLUE}Installing system dependencies...${NC}"

        # Determine Python command and packages needed
        PYTHON_CMD=""
        PYTHON_VENV_PKG=""

        if command -v python3.12 &> /dev/null; then
            PYTHON_CMD="python3.12"
            PYTHON_VENV_PKG="python3.12-venv"
        elif command -v python3 &> /dev/null; then
            PYTHON_CMD="python3"
            PYTHON_VENV_PKG="python3-venv"
        fi

        # Collect packages to install
        PACKAGES_TO_INSTALL=()

        # Python
        if ! command -v python3.12 &> /dev/null && ! command -v python3 &> /dev/null; then
            PACKAGES_TO_INSTALL+=("python3.12")
            PYTHON_CMD="python3.12"
            PYTHON_VENV_PKG="python3.12-venv"
        fi

        # Python venv
        if ! dpkg -l 2>/dev/null | grep -q "^ii  python3.12-venv" && ! dpkg -l 2>/dev/null | grep -q "^ii  python3-venv"; then
            PACKAGES_TO_INSTALL+=("$PYTHON_VENV_PKG")
        fi

        # Node.js and npm
        NODE_TOO_OLD=0
        NODE_VERSION=""
        NODE_MAJOR=""

        if command -v node &> /dev/null; then
            NODE_VERSION=$(node --version 2>&1)
            NODE_MAJOR=$(echo "$NODE_VERSION" | sed 's/^v//' | cut -d. -f1)
            if [ -n "$NODE_MAJOR" ] && [ "$NODE_MAJOR" -lt 24 ]; then
                NODE_TOO_OLD=1
                echo -e "${YELLOW}Node.js $NODE_VERSION found (< 24 required)${NC}"
            fi
        fi

        if ! command -v node &> /dev/null || [ "$NODE_TOO_OLD" -eq 1 ]; then
            PACKAGES_TO_INSTALL+=("nodejs")
        fi

        if ! command -v npm &> /dev/null; then
            PACKAGES_TO_INSTALL+=("npm")
        fi

        # GTK and WebKitGTK
        if ! dpkg -l 2>/dev/null | grep -q "^ii  libgtk-3-0"; then
            PACKAGES_TO_INSTALL+=("libgtk-3-0")
        fi

        WEBKIT_PKG=""
        if dpkg -l 2>/dev/null | grep -q "^ii  libwebkit2gtk-4.1-0"; then
            :
        elif dpkg -l 2>/dev/null | grep -q "^ii  libwebkit2gtk-4.0-37"; then
            :
        elif apt-cache show libwebkit2gtk-4.1-0 >/dev/null 2>&1; then
            WEBKIT_PKG="libwebkit2gtk-4.1-0"
        elif apt-cache show libwebkit2gtk-4.0-37 >/dev/null 2>&1; then
            WEBKIT_PKG="libwebkit2gtk-4.0-37"
        else
            echo -e "${YELLOW}Warning: WebKitGTK package not found in apt cache. You may need to enable 'universe'.${NC}"
        fi

        if [ -n "$WEBKIT_PKG" ]; then
            PACKAGES_TO_INSTALL+=("$WEBKIT_PKG")
        fi

        WEBKIT_GIR_PKG=""
        if dpkg -l 2>/dev/null | grep -q "^ii  gir1.2-webkit2-4.1"; then
            :
        elif dpkg -l 2>/dev/null | grep -q "^ii  gir1.2-webkit2-4.0"; then
            :
        elif apt-cache show gir1.2-webkit2-4.1 >/dev/null 2>&1; then
            WEBKIT_GIR_PKG="gir1.2-webkit2-4.1"
        elif apt-cache show gir1.2-webkit2-4.0 >/dev/null 2>&1; then
            WEBKIT_GIR_PKG="gir1.2-webkit2-4.0"
        else
            echo -e "${YELLOW}Warning: WebKitGTK GIR package not found in apt cache. You may need to enable 'universe'.${NC}"
        fi

        if [ -n "$WEBKIT_GIR_PKG" ]; then
            PACKAGES_TO_INSTALL+=("$WEBKIT_GIR_PKG")
        fi

        if ! dpkg -l 2>/dev/null | grep -q "^ii  python3-gi"; then
            PACKAGES_TO_INSTALL+=("python3-gi")
        fi

        if ! dpkg -l 2>/dev/null | grep -q "^ii  python3-gi-cairo"; then
            PACKAGES_TO_INSTALL+=("python3-gi-cairo")
        fi

        if ! dpkg -l 2>/dev/null | grep -q "^ii  gir1.2-gtk-3.0"; then
            PACKAGES_TO_INSTALL+=("gir1.2-gtk-3.0")
        fi

        if [ ${#PACKAGES_TO_INSTALL[@]} -gt 0 ]; then
            echo -e "${YELLOW}Installing: ${PACKAGES_TO_INSTALL[*]}${NC}"
            sudo apt update
            sudo apt install -y "${PACKAGES_TO_INSTALL[@]}"
            echo -e "${GREEN}✓ System dependencies installed${NC}"
        fi

        echo ""
    fi
else
    echo -e "${YELLOW}Warning: system-check.sh not found, skipping dependency check${NC}"
fi

# Verify Node.js version (24 LTS required)
if ! command -v node &> /dev/null; then
    echo -e "${RED}Error: Node.js 24 LTS not found${NC}"
    echo -e "${YELLOW}Install Node.js 24 LTS via nvm or NodeSource, then re-run ./install.sh${NC}"
    exit 1
fi

NODE_VERSION=$(node --version 2>&1)
NODE_MAJOR=$(echo "$NODE_VERSION" | sed 's/^v//' | cut -d. -f1)

if [ -z "$NODE_MAJOR" ] || [ "$NODE_MAJOR" -lt 24 ]; then
    echo -e "${RED}Error: Node.js 24 LTS required (found $NODE_VERSION)${NC}"
    echo -e "${YELLOW}Install Node.js 24 LTS via nvm or NodeSource, then re-run ./install.sh${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Using Node.js: $NODE_VERSION${NC}"
echo ""

# Determine Python command
if command -v python3.12 &> /dev/null; then
    PYTHON_CMD="python3.12"
elif command -v python3 &> /dev/null; then
    PYTHON_CMD="python3"
else
    echo -e "${RED}Error: Python 3.12+ not found${NC}"
    exit 1
fi

PYTHON_VERSION=$($PYTHON_CMD --version 2>&1 | awk '{print $2}')
echo -e "${GREEN}✓ Using Python: $PYTHON_VERSION${NC}"
echo ""

# Step 2: Create Python virtual environment
echo -e "${BLUE}[2/5] Setting up Python virtual environment...${NC}"

if [ -d "$VENV_DIR" ]; then
    echo -e "${YELLOW}Virtual environment already exists at: $VENV_DIR${NC}"
    read -p "Recreate virtual environment? [y/N]: " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Removing old virtual environment..."
        rm -rf "$VENV_DIR"
    else
        echo "Using existing virtual environment"
    fi
fi

if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment with --system-site-packages (for GTK access)..."
    $PYTHON_CMD -m venv --system-site-packages "$VENV_DIR"
    echo -e "${GREEN}✓ Virtual environment created${NC}"
fi

# Activate virtual environment
source "$VENV_DIR/bin/activate"

# Upgrade pip
echo "Upgrading pip..."
pip install --upgrade pip > /dev/null

# Install Python dependencies
echo "Installing Python dependencies..."
if [ -f "$SCRIPT_DIR/requirements-lock.txt" ]; then
    echo "Using locked dependencies (requirements-lock.txt)..."
    pip install -r "$SCRIPT_DIR/requirements-lock.txt"
    echo -e "${GREEN}✓ Python dependencies installed (pinned versions)${NC}"
elif [ -f "$SCRIPT_DIR/requirements.txt" ]; then
    echo -e "${YELLOW}Warning: requirements-lock.txt not found, using requirements.txt${NC}"
    pip install -r "$SCRIPT_DIR/requirements.txt"
    echo -e "${GREEN}✓ Python dependencies installed${NC}"
else
    echo -e "${RED}Error: No requirements file found${NC}"
    exit 1
fi

echo ""

# Step 3: Install frontend dependencies
echo -e "${BLUE}[3/5] Installing frontend dependencies...${NC}"

if [ ! -d "$FRONTEND_DIR" ]; then
    echo -e "${RED}Error: Frontend directory not found at $FRONTEND_DIR${NC}"
    exit 1
fi

cd "$FRONTEND_DIR"

if [ -d "node_modules" ]; then
    echo -e "${YELLOW}node_modules already exists${NC}"
    read -p "Reinstall npm packages? [y/N]: " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf node_modules
        npm ci
    else
        echo "Using existing node_modules"
    fi
else
    echo "Installing npm packages with locked dependencies..."
    if [ -f "package-lock.json" ]; then
        npm ci
    else
        echo -e "${YELLOW}Warning: package-lock.json not found, using 'npm install'${NC}"
        npm install
    fi
fi

echo -e "${GREEN}✓ Frontend dependencies installed${NC}"
echo ""

# Step 4: Build frontend
echo -e "${BLUE}[4/5] Building frontend...${NC}"

npm run build

if [ ! -d "dist" ]; then
    echo -e "${RED}Error: Frontend build failed - dist directory not created${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Frontend built successfully${NC}"
echo ""

# Step 5: Create launcher script
echo -e "${BLUE}[5/5] Creating launcher script...${NC}"

cd "$SCRIPT_DIR"

cat > "$LAUNCHER_SCRIPT" << 'EOF'
#!/bin/bash
# Linux ComfyUI Launcher - Wrapper Script
# Supports running the app and building the frontend

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_ACTIVATE="$SCRIPT_DIR/venv/bin/activate"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
VENV_PRE_COMMIT="$SCRIPT_DIR/venv/bin/pre-commit"
VENV_CYCLONEDX="$SCRIPT_DIR/venv/bin/cyclonedx-py"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

install_dev_tools() {
    if [ ! -f "$SCRIPT_DIR/requirements-dev.txt" ]; then
        echo "Error: requirements-dev.txt not found at $SCRIPT_DIR"
        exit 1
    fi

    source "$VENV_ACTIVATE"
    cd "$SCRIPT_DIR"
    echo -e "${BLUE}Installing dev tooling...${NC}"
    pip install -r "$SCRIPT_DIR/requirements-dev.txt"

    if command -v pre-commit >/dev/null 2>&1 && [ -d "$SCRIPT_DIR/.git" ]; then
        pre-commit install --install-hooks
    fi

    echo -e "${GREEN}✓ Dev tooling installed${NC}"
}

if [ ! -f "$VENV_ACTIVATE" ]; then
    echo "Error: Virtual environment not found at $SCRIPT_DIR/venv"
    echo "Please run ./install.sh first"
    exit 1
fi

# Handle commands
case "$1" in
    build)
        echo -e "${BLUE}Building frontend...${NC}"
        cd "$FRONTEND_DIR"
        npm run build
        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✓ Frontend built successfully${NC}"
            echo "Run './launcher' to start the app with the new build"
        else
            echo "Error: Frontend build failed"
            exit 1
        fi
        ;;

    dev)
        # Run with developer console enabled
        echo -e "${YELLOW}Starting with developer console enabled${NC}"
        source "$VENV_ACTIVATE"
        cd "$SCRIPT_DIR"
        export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
        python "$SCRIPT_DIR/backend/main.py" --dev
        ;;

    dev-install)
        install_dev_tools
        ;;

    test)
        if [ ! -x "$VENV_PRE_COMMIT" ]; then
            echo -e "${YELLOW}Dev tooling not installed.${NC}"
            echo "Run './launcher dev-install' to install requirements-dev.txt and pre-commit hooks."
            exit 1
        fi
        echo -e "${BLUE}Running all git hook checks via pre-commit...${NC}"
        source "$VENV_ACTIVATE"
        cd "$SCRIPT_DIR"

        # Run pre-commit hooks on all files (single source of truth)
        pre-commit run --all-files

        if [ $? -eq 0 ]; then
            echo ""
            echo -e "${GREEN}✓ All checks passed${NC}"
        else
            echo ""
            echo -e "${RED}✗ Some checks failed${NC}"
            echo ""
            echo "To auto-fix formatting issues, run:"
            echo "  pre-commit run --all-files"
            echo ""
            echo "Or fix specific issues:"
            echo "  python -m black ."
            echo "  python -m isort ."
            exit 1
        fi
        ;;

    sbom)
        if [ ! -x "$VENV_CYCLONEDX" ]; then
            echo -e "${YELLOW}Dev tooling not installed.${NC}"
            echo "Run './launcher dev-install' to install requirements-dev.txt."
            exit 1
        fi
        echo -e "${BLUE}Generating Software Bill of Materials (SBOM)...${NC}"
        source "$VENV_ACTIVATE"
        cd "$SCRIPT_DIR"

        # Run the SBOM generation script
        bash "$SCRIPT_DIR/scripts/dev/generate-sbom.sh"

        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✓ SBOM generated successfully${NC}"
            echo "  - Python SBOM: docs/sbom/sbom-python.json"
            echo "  - Frontend SBOM: docs/sbom/sbom-frontend.json"
        else
            echo "Error: SBOM generation failed"
            exit 1
        fi
        ;;

    help|--help|-h)
        echo "Linux ComfyUI Launcher"
        echo ""
        echo "Usage:"
        echo "  ./launcher          Run the application"
        echo "  ./launcher dev      Run with developer console enabled"
        echo "  ./launcher dev-install  Install dev tooling (requirements-dev.txt)"
        echo "  ./launcher build    Build the frontend (npm run build)"
        echo "  ./launcher test     Run all pre-commit hooks (formatting, linting, tests, type checking)"
        echo "  ./launcher sbom     Generate Software Bill of Materials"
        echo "  ./launcher help     Show this help message"
        ;;

    "")
        # No arguments - run the app (no debug console)
        source "$VENV_ACTIVATE"
        cd "$SCRIPT_DIR"
        export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
        python "$SCRIPT_DIR/backend/main.py"
        ;;

    *)
        echo "Unknown command: $1"
        echo "Run './launcher help' for usage information"
        exit 1
        ;;
esac
EOF

chmod +x "$LAUNCHER_SCRIPT"

echo -e "${GREEN}✓ Launcher script created at: $LAUNCHER_SCRIPT${NC}"
echo ""

# Summary
echo -e "${CYAN}╔════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║         Installation Complete!         ║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}The Linux ComfyUI Launcher is now installed.${NC}"
echo ""
echo -e "${BLUE}Usage:${NC}"
echo -e "  ${YELLOW}./launcher${NC}          - Run the application"
echo -e "  ${YELLOW}./launcher dev${NC}      - Run with developer console (F12)"
echo -e "  ${YELLOW}./launcher dev-install${NC}  - Install dev tooling"
echo -e "  ${YELLOW}./launcher build${NC}    - Build the frontend"
echo -e "  ${YELLOW}./launcher test${NC}     - Run pre-commit hooks"
echo -e "  ${YELLOW}./launcher sbom${NC}     - Generate SBOMs"
echo -e "  ${YELLOW}./launcher help${NC}     - Show help"
echo ""
echo -e "${BLUE}Optional: Add to your PATH${NC}"
echo -e "  ${YELLOW}ln -s $LAUNCHER_SCRIPT ~/.local/bin/comfyui-launcher${NC}"
echo -e "  Then run from anywhere: ${YELLOW}comfyui-launcher${NC}"
echo ""
echo -e "${BLUE}For developers:${NC}"
echo -e "  See ${CYAN}BUILD_README.md${NC} for development setup and build instructions"
echo ""
