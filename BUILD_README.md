# ComfyUI Setup Launcher

A modern, zero-dependency desktop application for setting up ComfyUI on Linux systems. Built with PyWebView + React for a native desktop experience without requiring any pre-installed dependencies from end users.

## Features

- **Zero Dependencies**: Single executable binary with no runtime dependencies (uses system WebKitGTK)
- **Modern UI**: Professional React interface with smooth animations (Framer Motion)
- **Python Backend**: Clean API architecture with PyWebView JavaScript bridge
- **Linux Native**: Optimized for Debian-based distributions (Mint, Ubuntu, etc.)
- **Setup Management**:
  - Install dependencies (setproctitle, git, brave-browser)
  - Patch ComfyUI main.py for process naming
  - Create application menu shortcuts
  - Create desktop shortcuts

## Technology Stack

- **Frontend**: React 19 + TypeScript + Vite + Framer Motion + Tailwind CSS
- **Backend**: Python 3.13 + PyWebView
- **Desktop**: PyWebView with GTK backend
- **Packaging**: PyInstaller (single executable)
- **Target**: Linux x86_64 (Debian-based)

## Architecture

```
ComfyUI-Launcher/
├── frontend/              # React application
│   ├── src/
│   │   ├── App.tsx       # Main React component
│   │   ├── index.tsx     # React entry point
│   │   └── components/   # UI components
│   ├── dist/             # Built frontend (created by npm run build)
│   ├── package.json
│   └── vite.config.ts
├── backend/               # Python backend
│   ├── main.py           # PyWebView application entry point
│   └── api.py            # Business logic API
├── build/                 # PyInstaller build artifacts
├── dist/                  # Final executable output
│   └── comfyui-setup     # Single executable binary
├── requirements.txt       # Python dependencies
├── comfyui-setup.spec    # PyInstaller configuration
├── dev-setup.sh          # Development environment setup
└── build.sh              # Production build script
```

## For End Users

### Running the Application

Simply download and run the executable:

```bash
chmod +x comfyui-setup
./comfyui-setup
```

**System Requirements:**
- Linux (Debian-based: Ubuntu, Mint, Debian, etc.)
- GTK3 and WebKitGTK (pre-installed on most distributions)

That's it! No Python, Node.js, or other dependencies needed.

## For Developers

### Prerequisites

- **Python 3.13+** (required)
- **Node.js 18+** (for building frontend)
- **GTK3 + WebKitGTK** (for PyWebView)

### Initial Setup

Run the development setup script:

```bash
./dev-setup.sh
```

This script will:
1. Create a Python virtual environment
2. Install Python dependencies (PyWebView, PyInstaller, etc.)
3. Install system dependencies (GTK, WebKitGTK)
4. Install frontend npm packages

### Development Workflow

#### Running in Development Mode

You need two terminals:

**Terminal 1 - Frontend Dev Server:**
```bash
cd frontend
npm run dev
```

**Terminal 2 - Python Application:**
```bash
source venv/bin/activate
python backend/main.py
```

The Python app will automatically connect to the Vite dev server at `http://127.0.0.1:3000` if the built frontend doesn't exist.

**Benefits:**
- Hot module replacement for React changes
- Faster iteration
- Browser DevTools available

**Limitations:**
- PyWebView API bridge won't work (shows dev mode message)
- For full testing, build the production version

#### Building for Production

To create a production build:

```bash
./build.sh
```

This will:
1. Build the React frontend (`frontend/dist/`)
2. Bundle everything with PyInstaller
3. Create a single executable at `dist/comfyui-setup`

**Build Output:**
- Executable: `dist/comfyui-setup`
- Typical size: 30-50 MB
- Includes: Python runtime, PyWebView, React build, all dependencies

#### Running Production Build

```bash
./dist/comfyui-setup
```

### Project Structure Details

#### Backend (`backend/`)

**main.py** - Application entry point
- Initializes PyWebView window
- Exposes Python API to JavaScript
- Handles development vs production mode
- Configures window properties

**api.py** - Business logic
- ComfyUI version detection
- Dependency checking (setproctitle, git, brave)
- Main.py patching/reverting
- Desktop shortcut management
- All setup operations

#### Frontend (`frontend/`)

**App.tsx** - Main React component
- UI state management
- PyWebView API integration
- Status polling
- User interactions

**Components:**
- `SpringyToggle.tsx` - Animated toggle switches
- Additional components as needed

### JavaScript ↔ Python Communication

The PyWebView bridge exposes Python methods to JavaScript:

**Python (backend/main.py):**
```python
class JavaScriptAPI:
    def get_status(self):
        return self.api.get_status()

    def toggle_patch(self):
        return self.api.toggle_patch()
```

**JavaScript (frontend/src/App.tsx):**
```typescript
// Access Python API
const status = await window.pywebview.api.get_status();
await window.pywebview.api.toggle_patch();
```

### Key Features Implementation

#### Zero Dependency Packaging

PyInstaller bundles:
- Python 3.13 runtime
- All Python packages
- React build files
- PyWebView libraries

**Not bundled (uses system):**
- GTK3
- WebKitGTK

This keeps the binary size manageable while ensuring compatibility.

#### Development vs Production

The app detects its mode by checking if `frontend/dist/index.html` exists:
- **Exists**: Production mode (serve from bundle)
- **Missing**: Development mode (connect to `http://127.0.0.1:3000`)

### Customization

#### Changing Window Size

Edit `backend/main.py`:
```python
window = webview.create_window(
    width=400,   # Change this
    height=520,  # Change this
    ...
)
```

#### Adding New API Methods

1. Add method to `ComfyUISetupAPI` class in `backend/api.py`
2. Expose it in `JavaScriptAPI` class in `backend/main.py`
3. Add TypeScript definition in `frontend/src/App.tsx`
4. Call from React component

#### Modifying UI

Edit React components in `frontend/src/`
- `App.tsx` for main layout
- Add new components in `src/components/`
- Tailwind CSS for styling

### Troubleshooting

#### Build Issues

**Frontend build fails:**
```bash
cd frontend
rm -rf node_modules package-lock.json
npm install
npm run build
```

**PyInstaller fails:**
```bash
source venv/bin/activate
pip install --upgrade pyinstaller
./build.sh
```

#### Runtime Issues

**GTK errors:**
```bash
sudo apt update
sudo apt install -y libgtk-3-0 libwebkit2gtk-4.1-0 gir1.2-webkit2-4.1
```

**Window doesn't open:**
- Check if GTK backend is available: `python -c "import gi"`
- Try running with debug: Edit `backend/main.py` and set `debug=True`

### Best Practices

#### KISS Principles

This project follows Keep It Simple, Stupid:

1. **Minimal dependencies**: Only essential packages
2. **Clear separation**: Frontend/backend cleanly separated
3. **Standard tools**: No exotic frameworks
4. **Single purpose**: Does one thing well
5. **Simple build**: Two-step process (build frontend, bundle app)

#### Code Style

- **Python**: Follow PEP 8
- **TypeScript/React**: Use functional components, hooks
- **Comments**: Explain why, not what
- **Naming**: Clear, descriptive names

### Performance Considerations

- Bundle size: ~30-50 MB (reasonable for desktop app)
- Startup time: <2 seconds on modern hardware
- Memory usage: ~80-150 MB (PyWebView + WebKit)

### Security Notes

- No network requests except GitHub API for version check
- No data collection or telemetry
- All operations local to user's machine
- Sudo only for system package installation (explicit user action)

## Building for Distribution

### Creating a Release

1. **Test the build:**
   ```bash
   ./build.sh
   ./dist/comfyui-setup
   ```

2. **Test on clean system:**
   - Use a VM or fresh install
   - Verify zero-dependency execution

3. **Create release archive:**
   ```bash
   cd dist
   tar -czf comfyui-setup-linux-x64.tar.gz comfyui-setup
   ```

4. **Optional: Create AppImage:**
   - Better Linux integration
   - Easier distribution
   - See AppImage documentation

### Distribution Checklist

- [ ] Test on Debian/Ubuntu/Mint
- [ ] Verify executable runs without dependencies
- [ ] Check file permissions (chmod +x)
- [ ] Test all features (install deps, patch, shortcuts)
- [ ] Create README for end users
- [ ] Tag release in git

## Contributing

### Development Process

1. Fork the repository
2. Run `./dev-setup.sh` to set up environment
3. Make changes
4. Test in development mode
5. Test production build
6. Submit pull request

### Testing

Manual testing checklist:
- [ ] All buttons work
- [ ] Status updates correctly
- [ ] Dependency installation works
- [ ] Patch/unpatch works
- [ ] Shortcuts create/remove correctly
- [ ] Window closes properly
- [ ] Production build works

## License

[Your License Here]

## Credits

- React + Framer Motion for beautiful UI
- PyWebView for native desktop integration
- Vite for fast frontend builds
- PyInstaller for Python packaging

## Support

For issues, feature requests, or questions:
- Open an issue on GitHub
- Check existing issues first
- Provide system info and error logs

---

**Built with simplicity in mind. KISS principles applied throughout.**
