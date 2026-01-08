# Multi-App GUI Implementation

This document describes the multi-app architecture implementation for the ComfyUI launcher.

## Overview

The launcher has been refactored to support multiple applications beyond just ComfyUI. The new architecture includes:

1. **Left Sidebar** - App switcher with visual status indicators
2. **Resource Monitors** - CPU, GPU, RAM, and Disk usage displayed in header
3. **Model Manager** - UI for linking AI models to different apps
4. **Tailwind CSS v4** - Upgraded with offline support
5. **Multi-App Configuration** - Extensible system for adding new apps

## Architecture Changes

### Frontend Changes

#### 1. Tailwind CSS v4 Upgrade

**Files Modified:**
- [frontend/package.json](../frontend/package.json) - Updated to `tailwindcss@^4.1.9`
- [frontend/postcss.config.mjs](../frontend/postcss.config.mjs) - New PostCSS config for v4
- [frontend/src/index.css](../frontend/src/index.css) - Updated with v4 syntax and custom theme tokens

**Key Changes:**
- Uses `@import "tailwindcss"` instead of separate directives
- Offline support (no CDN required)
- Custom CSS variables for launcher theme under `--launcher-*` prefix

#### 2. Type System

**New Files:**
- [frontend/src/types/apps.ts](../frontend/src/types/apps.ts)

**Defines:**
- `AppConfig` - Configuration for each supported application
- `AppStatus` - Application runtime status
- `AppIconState` - Visual state of app icons
- `ModelInfo` - AI model metadata
- `SystemResources` - System resource usage data

#### 3. App Configuration

**New Files:**
- [frontend/src/config/apps.ts](../frontend/src/config/apps.ts)

**Includes:**
- `DEFAULT_APPS` - Array of supported applications (ComfyUI, Open WebUI, InvokeAI, etc.)
- Helper functions for app management
- Extensible architecture for adding new apps

**Currently Configured Apps:**
1. ComfyUI (default)
2. Open WebUI
3. InvokeAI
4. Krita Diffusion
5. SD WebUI
6. Fooocus

#### 4. New Components

**[frontend/src/components/AppIcon.tsx](../frontend/src/components/AppIcon.tsx)**
- Visual icon component with 4 states:
  - `running` - Animated with CPU/GPU usage arcs
  - `offline` - Installed but not running
  - `uninstalled` - Not installed
  - `error` - Error state

**[frontend/src/components/AppSidebar.tsx](../frontend/src/components/AppSidebar.tsx)**
- Left sidebar with app icons
- Settings gear icon at bottom
- App selection logic
- Animated transitions

**[frontend/src/components/ResourceMonitor.tsx](../frontend/src/components/ResourceMonitor.tsx)**
- Displays CPU/GPU/RAM usage
- Color-coded indicators
- Real-time updates

**[frontend/src/components/ModelManager.tsx](../frontend/src/components/ModelManager.tsx)**
- Model library UI
- Star/favorite models
- Link models to apps
- Category grouping

#### 5. App.tsx Refactor

**File Modified:**
- [frontend/src/App.tsx](../frontend/src/App.tsx) (backup: [App.tsx.backup](../frontend/src/App.tsx.backup))

**Key Changes:**
- Multi-app state management
- Integration of new sidebar and resource monitors
- Model manager integration
- Maintains backward compatibility with existing ComfyUI features

### Backend Changes

#### 1. System Resources API

**Files Modified:**
- [backend/api/system_utils.py](../backend/api/system_utils.py)
- [backend/api/core.py](../backend/api/core.py)
- [backend/main.py](../backend/main.py)

**New Method: `get_system_resources()`**

Returns:
```python
{
    "success": bool,
    "resources": {
        "cpu": {"usage": float, "temp": float | None},
        "gpu": {"usage": float, "memory": float, "temp": float | None},
        "ram": {"usage": float, "total": float},
        "disk": {"usage": float, "total": float, "free": float}
    }
}
```

**Features:**
- Uses `psutil` for CPU/RAM monitoring
- Uses `nvidia-smi` for NVIDIA GPU monitoring (when available)
- Graceful fallback when tools are unavailable
- Caches disk usage data

**Dependencies:**
- Requires `psutil` package (already in requirements)
- Optional: `nvidia-smi` for GPU monitoring

## Usage

### Adding a New App

1. **Define the app in `config/apps.ts`:**

```typescript
import { YourIcon } from 'lucide-react';

{
  id: 'your-app',
  name: 'your-app',
  displayName: 'Your App Name',
  icon: YourIcon,
  status: 'idle',
  iconState: 'uninstalled',
  description: 'Description of your app',
  starred: false,
  linked: false,
}
```

2. **Backend Implementation** (future):
   - Create app-specific manager in `backend/`
   - Add install/launch/stop methods
   - Integrate with version manager

### Model Manager

Models can be:
- **Starred** - Favorited for quick access
- **Linked** - Associated with specific apps

Future enhancements will include:
- Model scanning from shared storage
- Automatic linking based on app requirements
- Model download/management

## Design Decisions

### Why Tailwind v4?

1. **Offline First** - No CDN dependencies
2. **Better Performance** - Faster build times
3. **Modern CSS** - Native CSS variables, better customization
4. **Theme System** - Custom `--launcher-*` tokens for consistent theming

### Why Left Sidebar?

1. **Scalability** - Supports many apps without cluttering UI
2. **Quick Switching** - One-click app selection
3. **Visual Status** - Icon states show app status at a glance
4. **Familiar Pattern** - Common in multi-app launchers (Discord, Slack, etc.)

### Why Resource Monitors in Header?

1. **Always Visible** - Critical for AI workloads
2. **Compact** - Minimal screen space
3. **Real-time** - Updates every 4 seconds
4. **Actionable** - Helps users understand system load

## Testing

### Frontend Build Test

```bash
cd frontend
npm run build
```

Expected output: Successful build with no errors

### Backend Syntax Check

```bash
python3 -m py_compile backend/api/system_utils.py backend/api/core.py backend/main.py
```

Expected output: No errors

### Runtime Testing

1. Start the launcher: `./launcher`
2. Check resource monitors appear in header
3. Verify sidebar displays app icons
4. Test app selection by clicking icons
5. Verify model manager is visible

## Known Limitations

1. **GPU Monitoring** - Currently NVIDIA-only (via nvidia-smi)
2. **Model Manager** - UI only, backend integration pending
3. **App Management** - Only ComfyUI fully functional, others are placeholders
4. **Resource Polling** - Fixed 4-second interval (same as status polling)

## Future Enhancements

### Short Term
1. Implement Open WebUI integration
2. Add model scanning and auto-detection
3. Per-app settings and configuration
4. AMD GPU support via `rocm-smi`

### Long Term
1. Plugin system for third-party apps
2. Shared model library with deduplication
3. Resource usage history/graphs
4. App dependency management
5. Multi-instance support (run multiple apps simultaneously)

## Migration Notes

### For Developers

The old `App.tsx` is backed up as `App.tsx.backup`. The new version:
- Maintains all existing ComfyUI functionality
- Adds multi-app support
- Uses new component architecture
- Is fully backward compatible

### For Users

No migration needed. Existing installations will work as before, with new multi-app features available immediately.

## Troubleshooting

### Resource monitors show zeros

**Cause:** `psutil` not installed or GPU monitoring unavailable

**Fix:**
```bash
cd /path/to/launcher
source venv/bin/activate
pip install psutil
```

### Build fails with Tailwind errors

**Cause:** Old Tailwind config files present

**Fix:**
```bash
cd frontend
rm -f tailwind.config.js postcss.config.cjs
npm run build
```

### Apps don't show in sidebar

**Cause:** Configuration not loaded

**Fix:** Check browser console for errors. Verify `config/apps.ts` is properly imported.

## API Reference

### Frontend API

#### `get_system_resources()`

JavaScript API call:
```typescript
const result = await window.pywebview.api.get_system_resources();
```

Response:
```typescript
{
  success: boolean;
  resources?: SystemResources;
  error?: string;
}
```

## Contributors

This implementation was created as part of the multi-app expansion project.

## License

Same as the main ComfyUI Launcher project.
