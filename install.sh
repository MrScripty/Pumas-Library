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
        if ! command -v node &> /dev/null; then
            PACKAGES_TO_INSTALL+=("nodejs")
        fi

        if ! command -v npm &> /dev/null; then
            PACKAGES_TO_INSTALL+=("npm")
        fi

        # GTK and WebKitGTK
        if ! dpkg -l 2>/dev/null | grep -q "^ii  libgtk-3-0"; then
            PACKAGES_TO_INSTALL+=("libgtk-3-0")
        fi

        if ! dpkg -l 2>/dev/null | grep -q "^ii  libwebkit2gtk-4.1-0" && ! dpkg -l 2>/dev/null | grep -q "^ii  libwebkit2gtk-4.0-37"; then
            PACKAGES_TO_INSTALL+=("libwebkit2gtk-4.1-0")
        fi

        if ! dpkg -l 2>/dev/null | grep -q "^ii  gir1.2-webkit2-4.1" && ! dpkg -l 2>/dev/null | grep -q "^ii  gir1.2-webkit2-4.0"; then
            PACKAGES_TO_INSTALL+=("gir1.2-webkit2-4.1")
        fi

        if ! dpkg -l 2>/dev/null | grep -q "^ii  python3-gi"; then
            PACKAGES_TO_INSTALL+=("python3-gi")
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
echo "Installing Python dependencies from requirements.txt..."
if [ -f "$SCRIPT_DIR/requirements.txt" ]; then
    pip install -r "$SCRIPT_DIR/requirements.txt"
    echo -e "${GREEN}✓ Python dependencies installed${NC}"
else
    echo -e "${YELLOW}Warning: requirements.txt not found${NC}"
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
        rm -rf node_modules package-lock.json
        npm install
    else
        echo "Using existing node_modules"
    fi
else
    echo "Installing npm packages..."
    npm install
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

# Color codes for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

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

    help|--help|-h)
        echo "Linux ComfyUI Launcher"
        echo ""
        echo "Usage:"
        echo "  ./launcher          Run the application"
        echo "  ./launcher dev      Run with developer console enabled"
        echo "  ./launcher build    Build the frontend (npm run build)"
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
echo -e "  ${YELLOW}./launcher build${NC}    - Build the frontend"
echo -e "  ${YELLOW}./launcher help${NC}     - Show help"
echo ""
echo -e "${BLUE}Optional: Add to your PATH${NC}"
echo -e "  ${YELLOW}ln -s $LAUNCHER_SCRIPT ~/.local/bin/comfyui-launcher${NC}"
echo -e "  Then run from anywhere: ${YELLOW}comfyui-launcher${NC}"
echo ""
echo -e "${BLUE}For developers:${NC}"
echo -e "  See ${CYAN}BUILD_README.md${NC} for development setup and build instructions"
echo ""
