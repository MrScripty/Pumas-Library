#!/usr/bin/env bash
#
# ComfyUI Setup Launcher - Build Script
# Builds React frontend and packages application with PyInstaller
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
BACKEND_DIR="$SCRIPT_DIR/backend"
BUILD_DIR="$SCRIPT_DIR/build"
DIST_DIR="$SCRIPT_DIR/dist"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}ComfyUI Setup Launcher - Build Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo -e "${RED}Error: Node.js is not installed${NC}"
    echo "Please install Node.js to build the frontend"
    exit 1
fi

# Check if Python 3.13 is available
if ! command -v python3.13 &> /dev/null; then
    echo -e "${YELLOW}Warning: python3.13 not found, trying python3...${NC}"
    PYTHON_CMD="python3"
else
    PYTHON_CMD="python3.13"
fi

# Verify Python version
PYTHON_VERSION=$($PYTHON_CMD --version 2>&1 | awk '{print $2}')
echo -e "${GREEN}Using Python: $PYTHON_VERSION${NC}"
echo ""

# Step 1: Build React Frontend
echo -e "${BLUE}[1/3] Building React frontend...${NC}"
cd "$FRONTEND_DIR"

if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}Installing npm dependencies...${NC}"
    npm install
fi

echo "Running Vite build..."
npm run build

if [ ! -d "dist" ]; then
    echo -e "${RED}Error: Frontend build failed - dist directory not created${NC}"
    exit 1
fi

echo -e "${GREEN}Frontend build complete!${NC}"
echo ""

# Step 2: Prepare Build Environment
echo -e "${BLUE}[2/3] Preparing build environment...${NC}"
cd "$SCRIPT_DIR"

# Check if virtual environment exists
if [ ! -d "venv" ]; then
    echo -e "${YELLOW}Virtual environment not found.${NC}"
    echo "Please run ./dev-setup.sh first to set up the development environment"
    exit 1
fi

# Activate virtual environment
source venv/bin/activate

# Verify PyInstaller is installed
if ! command -v pyinstaller &> /dev/null; then
    echo -e "${RED}Error: PyInstaller not found in virtual environment${NC}"
    echo "Please run ./dev-setup.sh to install dependencies"
    exit 1
fi

echo -e "${GREEN}Build environment ready!${NC}"
echo ""

# Step 3: Package with PyInstaller
echo -e "${BLUE}[3/3] Packaging application with PyInstaller...${NC}"

# Clean previous builds
if [ -d "$BUILD_DIR" ]; then
    echo "Cleaning previous build directory..."
    rm -rf "$BUILD_DIR"
fi

if [ -d "$DIST_DIR" ]; then
    echo "Cleaning previous dist directory..."
    rm -rf "$DIST_DIR"
fi

# Run PyInstaller
echo "Running PyInstaller..."
pyinstaller --clean comfyui-setup.spec

if [ ! -f "$DIST_DIR/comfyui-setup" ]; then
    echo -e "${RED}Error: PyInstaller build failed - executable not created${NC}"
    exit 1
fi

echo -e "${GREEN}Application packaged successfully!${NC}"
echo ""

# Show build results
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}Build Complete!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Executable location: $DIST_DIR/comfyui-setup"
echo "Size: $(du -h "$DIST_DIR/comfyui-setup" | cut -f1)"
echo ""
echo "To run the application:"
echo -e "${YELLOW}  ./dist/comfyui-setup${NC}"
echo ""
echo "To test the application:"
echo -e "${YELLOW}  cd dist && ./comfyui-setup${NC}"
echo ""
