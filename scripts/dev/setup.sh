#!/usr/bin/env bash
#
# ComfyUI Setup Launcher - Development Setup Script
# Sets up development environment for building the application
#

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$SCRIPT_DIR/frontend"
VENV_DIR="$SCRIPT_DIR/venv"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}ComfyUI Setup Launcher - Dev Setup${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check Python 3.12+
if command -v python3.13 &> /dev/null; then
    PYTHON_CMD="python3.13"
    echo -e "${GREEN}Found Python 3.13${NC}"
elif command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version 2>&1 | awk '{print $2}')
    MAJOR=$(echo "$PYTHON_VERSION" | cut -d. -f1)
    MINOR=$(echo "$PYTHON_VERSION" | cut -d. -f2)

    if [ "$MAJOR" -eq 3 ] && [ "$MINOR" -ge 12 ]; then
        PYTHON_CMD="python3"
        echo -e "${GREEN}Found Python $PYTHON_VERSION${NC}"
    else
        echo -e "${RED}Error: Python 3.12+ required, found $PYTHON_VERSION${NC}"
        exit 1
    fi
else
    echo -e "${RED}Error: Python 3.12+ not found${NC}"
    echo "Please install Python 3.12 or higher"
    exit 1
fi

# Check Node.js
if ! command -v node &> /dev/null; then
    echo -e "${RED}Error: Node.js not found${NC}"
    echo "Please install Node.js (v18 or higher recommended)"
    exit 1
fi

NODE_VERSION=$(node --version)
echo -e "${GREEN}Found Node.js $NODE_VERSION${NC}"
echo ""

# Step 1: Set up Python virtual environment
echo -e "${BLUE}[1/3] Setting up Python virtual environment...${NC}"

if [ -d "$VENV_DIR" ]; then
    echo -e "${YELLOW}Virtual environment already exists${NC}"
    echo "Removing old venv to recreate with system-site-packages..."
    rm -rf "$VENV_DIR"
fi

echo "Creating virtual environment with --system-site-packages (for GTK access)..."
$PYTHON_CMD -m venv --system-site-packages "$VENV_DIR"
echo -e "${GREEN}Virtual environment created!${NC}"

# Activate virtual environment
source "$VENV_DIR/bin/activate"

# Upgrade pip
echo "Upgrading pip..."
pip install --upgrade pip > /dev/null

# Install Python dependencies
echo "Installing Python dependencies from requirements.txt..."
pip install -r "$SCRIPT_DIR/requirements.txt"

echo -e "${GREEN}Python dependencies installed!${NC}"
echo ""

# Step 2: Install system dependencies for PyWebView (GTK)
echo -e "${BLUE}[2/3] Checking system dependencies...${NC}"

MISSING_DEPS=()

# Check for required GTK packages
if ! dpkg -l | grep -q libgtk-3-0; then
    MISSING_DEPS+=("libgtk-3-0")
fi

if ! dpkg -l | grep -q libwebkit2gtk-4.0-37 && ! dpkg -l | grep -q libwebkit2gtk-4.1-0; then
    MISSING_DEPS+=("libwebkit2gtk-4.1-0")
fi

if ! dpkg -l | grep -q gir1.2-webkit2-4.0 && ! dpkg -l | grep -q gir1.2-webkit2-4.1; then
    MISSING_DEPS+=("gir1.2-webkit2-4.1")
fi

# Check for Python GTK bindings
if ! dpkg -l | grep -q python3-gi; then
    MISSING_DEPS+=("python3-gi")
fi

if ! dpkg -l | grep -q gir1.2-gtk-3.0; then
    MISSING_DEPS+=("gir1.2-gtk-3.0")
fi

if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    echo -e "${YELLOW}Missing system dependencies: ${MISSING_DEPS[*]}${NC}"
    echo "Installing system dependencies (requires sudo)..."
    sudo apt update
    sudo apt install -y "${MISSING_DEPS[@]}"
    echo -e "${GREEN}System dependencies installed!${NC}"
else
    echo -e "${GREEN}All system dependencies already installed${NC}"
fi

echo ""

# Step 3: Install frontend dependencies
echo -e "${BLUE}[3/3] Installing frontend dependencies...${NC}"

cd "$FRONTEND_DIR"

if [ -d "node_modules" ]; then
    echo -e "${YELLOW}node_modules already exists${NC}"
    read -p "Reinstall? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf node_modules package-lock.json
        npm install
    else
        echo "Skipping npm install"
    fi
else
    echo "Installing npm packages..."
    npm install
fi

echo -e "${GREEN}Frontend dependencies installed!${NC}"
echo ""

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}Development Environment Ready!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Next steps:"
echo ""
echo "1. To run in development mode:"
echo -e "   ${YELLOW}# Terminal 1: Start frontend dev server${NC}"
echo -e "   ${YELLOW}cd frontend && npm run dev${NC}"
echo ""
echo -e "   ${YELLOW}# Terminal 2: Run Python app (connects to dev server)${NC}"
echo -e "   ${YELLOW}source venv/bin/activate${NC}"
echo -e "   ${YELLOW}python backend/main.py${NC}"
echo ""
echo "2. To build for production:"
echo -e "   ${YELLOW}./build.sh${NC}"
echo ""
echo "3. To run the production build:"
echo -e "   ${YELLOW}./dist/comfyui-setup${NC}"
echo ""
