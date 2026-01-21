# Pumas Library

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)
![Python (depreciated)](https://img.shields.io/badge/python-3.12+-blue.svg)
![Electron](https://img.shields.io/badge/electron-38+-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux-green.svg)

Availible as a desktop GUI for end-users, and as a headless Rust crate for embedabble API use.

Pumas Library is an easy to use AI model library that downloads, organizes, and serves AI model weights and metadata to other apps. Instead of having models duplicated or scattered accross applications, Pumas Library provides a standardized central source that is automaticly maintained. When integrated into other software via the Rust crate, it eliminates the need for a slew of file, network, and remote API boilerplate and smart logic.

Please note: The python backend is depreciated and will be removed soon. It is being fully replaced by the new Rust backend as Rust was proven to be much cleaner and easier to maintain for Pumas Library. 

## Features

- A single portable model library with rich metadata
- Links your apps to your library, no manual setup required (GUI only, use API for direct integration with the Rust crate)
- System and per-app resource monitoring (partial Rust crate integration)
- Search and download new models into your library
- Install and run different app versions with ease (GUI only)
- Smart system shortcuts that don't require the launcher to work (GUI only)
- Ghost bust the background servers when closing apps (GUI only)
- And other technical mumbo-jumbo

## Architecture

Pumas Library uses a modern **Electron + Rust backend** architecture:

- **Frontend**: React 19 + Vite (rendered in Electron's Chromium)
- **Desktop Shell**: Electron 38+ with native Wayland support
- **Backend**: Rust (default) or Python 3.12+ (depreciated) running as a sidecar process
- **IPC**: JSON-RPC communication between Electron and backend

## Installation

### System Requirements

- **Operating System**: Linux (Debian/Ubuntu-based distros recommended)
- **Rust**: 1.75+ (for building the backend)
- **Python**: 3.12+ (optional)
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
| `./launcher`                 | Launch the application (Rust backend, default)                   |
| `./launcher python`          | Launch with Python backend                                       |
| `./launcher dev`             | Launch with developer tools (Rust backend)                       |
| `./launcher dev-python`      | Launch with developer tools (Python backend)                     |
| `./launcher build`           | Build all components (Rust, frontend, Electron)                  |
| `./launcher build-rust`      | Build Rust backend only                                          |
| `./launcher build-electron`  | Build Electron TypeScript only                                   |
| `./launcher package`         | Package Electron app for distribution                            |
| `./launcher electron-install`| Install Electron dependencies                                    |
| `./launcher dev-install`     | Install dev tooling (requirements-dev.txt)                       |
| `./launcher test`            | Run pre-commit hooks (formatting, linting, tests, type checking) |
| `./launcher sbom`            | Generate Software Bill of Materials (SBOM) for dependencies      |
| `./launcher help`            | Display usage information                                        |
```bash
./launcher python
```

## More Details Later (WIP)
