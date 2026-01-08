# Linux ComfyUI Launcher

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Python](https://img.shields.io/badge/python-3.12+-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux-green.svg)

A comprehensive yet easy to use launcher that opens ComfyUI as a standalone app, manages models accross multiple installs, and other minor QOL improvments.

## Features

- A single portable model library with rich metadata
- Links your apps to your library, no manual setup required
- System and per-app resource monitoring
- Search and download new models into your library
- Install and run different app versions with ease
- Smart system shortcuts that dont require the launcher to work
- Ghost buster the backgorund servers when closing apps
- And other technical mubo-jumbo

## Installation

### Quick Install (Recommended)

Run the automated installation script:

```bash
./install.sh
```

The installer will:
1. Check and install system dependencies (with your permission)
2. Create a Python virtual environment
3. Install Python dependencies
4. Install and build the frontend
5. Create the launcher script

### System Requirements

- **Operating System**: Linux (Debian/Ubuntu-based distros recommended)
- **Python**: 3.12+ (3.12 recommended)
- **Node.js**: 14+ (for building the frontend)
- **System Libraries**:
  - GTK 3.0 (`libgtk-3-0`)
  - WebKit2GTK (`libwebkit2gtk-4.1-0` or `libwebkit2gtk-4.0-37`)
  - Python GObject bindings (`python3-gi`, `gir1.2-gtk-3.0`, `gir1.2-webkit2-4.1`)

### Manual Installation

If you prefer to install manually:

1. **Install system dependencies** (Debian/Ubuntu):
   ```bash
   sudo apt update
   sudo apt install python3.12 python3.12-venv nodejs npm \
     libgtk-3-0 libwebkit2gtk-4.1-0 gir1.2-webkit2-4.1 \
     python3-gi gir1.2-gtk-3.0
   ```

2. **Create Python virtual environment**:
   ```bash
   python3.12 -m venv --system-site-packages venv
   source venv/bin/activate
   ```

3. **Install Python dependencies**:
   ```bash
   pip install --upgrade pip
   pip install -r requirements.txt
   ```

4. **Install and build frontend**:
   ```bash
   cd frontend
   npm install
   npm run build
   cd ..
   ```

5. **Make launcher executable** (should already be executable):
   ```bash
   chmod +x launcher
   ```

### Optional: Add to PATH

For system-wide access:

```bash
ln -s $(pwd)/launcher ~/.local/bin/comfyui-launcher
```

Then run from anywhere:
```bash
comfyui-launcher
```

## Usage

### Launcher Arguments

Run the launcher with different modes:

| Command | Description |
|---------|-------------|
| `./launcher` | Launch the application in normal mode |
| `./launcher dev` | Launch with developer console enabled for debugging |
| `./launcher build` | Rebuild the frontend (useful after making UI changes) |
| `./launcher test` | Run unit tests and code quality checks (pytest, ruff, black, isort, mypy) |
| `./launcher sbom` | Generate Software Bill of Materials (SBOM) for dependencies |
| `./launcher help` | Display usage information |

## More Details Later (WIP)
