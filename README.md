# Pumas Library

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Python](https://img.shields.io/badge/python-3.12+-blue.svg)
![Electron](https://img.shields.io/badge/electron-38+-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux-green.svg)

Easy to use AI model library that links your models to other apps in the launcher, and other QOL improvements.

## Features

- A single portable model library with rich metadata
- Links your apps to your library, no manual setup required
- System and per-app resource monitoring
- Search and download new models into your library
- Install and run different app versions with ease
- Smart system shortcuts that don't require the launcher to work
- Ghost bust the background servers when closing apps
- And other technical mumbo-jumbo

## Architecture

Pumas Library uses a modern **Electron + Python sidecar** architecture:

- **Frontend**: React 19 + Vite (rendered in Electron's Chromium)
- **Desktop Shell**: Electron 38+ with native Wayland support
- **Backend**: Python 3.12+ running as a sidecar process
- **IPC**: JSON-RPC communication between Electron and Python

## Installation

### System Requirements

- **Operating System**: Linux (Debian/Ubuntu-based distros recommended)
- **Python**: 3.12+
- **Node.js**: 24+ LTS

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

### Manual Installation

If you prefer to install manually:

1. **Install system dependencies** (Debian/Ubuntu):

   ```bash
   sudo apt update
   sudo apt install python3.12 python3.12-venv nodejs npm
   ```

2. **Create Python virtual environment**:

   ```bash
   python3.12 -m venv venv
   source venv/bin/activate
   ```

3. **Install Python dependencies**:

   ```bash
   pip install --upgrade pip
   pip install -r requirements-lock.txt
   ```

   If `requirements-lock.txt` is unavailable:

   ```bash
   pip install -r requirements.txt
   ```

4. **Install and build frontend**:

   ```bash
   cd frontend
   npm install
   npm run build
   cd ..
   ```

5. **Install Electron dependencies**:

   ```bash
   cd electron
   npm install
   npm run build
   cd ..
   ```

6. **Make launcher executable** (should already be executable):

   ```bash
   chmod +x launcher
   ```

### Optional: Add to PATH

For system-wide access:

```bash
ln -s $(pwd)/launcher ~/.local/bin/pumas-library
```

Then run from anywhere:

```bash
pumas-library
```

## Usage

### Launcher Commands

Run the launcher with different modes:

| Command                      | Description                                                      |
| ---------------------------- | ---------------------------------------------------------------- |
| `./launcher`                 | Launch the application (Electron)                                |
| `./launcher dev`             | Launch with developer tools enabled                              |
| `./launcher build`           | Rebuild the frontend                                             |
| `./launcher build-electron`  | Rebuild Electron TypeScript                                      |
| `./launcher package`         | Package Electron app for distribution                            |
| `./launcher electron-install`| Install Electron dependencies                                    |
| `./launcher dev-install`     | Install dev tooling (requirements-dev.txt)                       |
| `./launcher test`            | Run pre-commit hooks (formatting, linting, tests, type checking) |
| `./launcher sbom`            | Generate Software Bill of Materials (SBOM) for dependencies      |
| `./launcher help`            | Display usage information                                        |

## More Details Later (WIP)
