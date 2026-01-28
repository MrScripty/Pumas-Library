# Pumas Library

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)
![Electron](https://img.shields.io/badge/electron-38+-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux-green.svg)

Available as a desktop GUI for end-users, and as a headless Rust crate for embeddable API use.

Pumas Library is an easy to use AI model library that downloads, organizes, and serves AI model weights and metadata to other apps. Instead of having models duplicated or scattered across applications, Pumas Library provides a standardized central source that is automatically maintained. When integrated into other software via the Rust crate, it eliminates the need for a slew of file, network, and remote API boilerplate and smart logic.

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
- **Backend**: Rust running as a sidecar process
- **IPC**: JSON-RPC communication between Electron and backend

## Installation

### System Requirements

- **Operating System**: Linux (Debian/Ubuntu-based distros recommended)
- **Rust**: 1.75+ (for building the backend)
- **Node.js**: 24+ LTS

### Quick Install (Recommended)

Run the automated installation script:

```bash
./install.sh
```

The installer will:

1. Check and install system dependencies (with your permission)
2. Build the Rust backend
3. Install and build the frontend
4. Install and build Electron
5. Create the launcher script

### Manual Installation

If you prefer to install manually:

1. **Install system dependencies** (Debian/Ubuntu):

   ```bash
   sudo apt update
   sudo apt install nodejs npm cargo
   ```

2. **Build Rust backend**:

   ```bash
   cd rust
   cargo build --release
   cd ..
   ```

3. **Install and build frontend**:

   ```bash
   cd frontend
   npm install
   npm run build
   cd ..
   ```

4. **Install Electron dependencies**:

   ```bash
   cd electron
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
ln -s $(pwd)/launcher ~/.local/bin/pumas-library
```

Then run from anywhere:

```bash
pumas-library
```

## Usage

### Launcher Commands

Run the launcher with different modes:

| Command                       | Description                             |
| ----------------------------- | --------------------------------------- |
| `./launcher`                  | Launch the application                  |
| `./launcher dev`              | Launch with developer tools             |
| `./launcher build`            | Build all components (Rust, frontend, Electron) |
| `./launcher build-rust`       | Build Rust backend only                 |
| `./launcher build-electron`   | Build Electron TypeScript only          |
| `./launcher package`          | Package Electron app for distribution   |
| `./launcher electron-install` | Install Electron dependencies           |
| `./launcher help`             | Display usage information               |

## More Details Later (WIP)
