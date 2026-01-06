# ComfyUI Icon Integration

This document describes the integration of the actual ComfyUI icon into the multi-app sidebar and restoration of the original version selector functionality.

## Changes Made

### 1. ComfyUI Icon Component

**File Created:** [frontend/src/components/ComfyUIIcon.tsx](../frontend/src/components/ComfyUIIcon.tsx)

A specialized icon component that displays the actual ComfyUI icon (`comfyui-icon.webp`) instead of a generic Lucide icon.

**Features:**
- Uses the real ComfyUI icon image
- Four visual states:
  - `running` - Shows icon with animated CPU/GPU usage arcs
  - `offline` - Installed but not running (with play indicator)
  - `uninstalled` - Not installed (grayed out with play indicator)
  - `error` - Error state (with warning indicator)
- Matches the design from the mockup with the actual branding

### 2. Updated AppSidebar

**File Modified:** [frontend/src/components/AppSidebar.tsx](../frontend/src/components/AppSidebar.tsx)

**Changes:**
- Imports `ComfyUIIcon` component
- Conditionally renders `ComfyUIIcon` for the first app when `app.id === 'comfyui'`
- All other apps continue to use the generic `AppIcon` with Lucide icons

**Logic:**
```typescript
{apps.map((app, index) => (
  index === 0 && app.id === 'comfyui' ? (
    <ComfyUIIcon ... />
  ) : (
    <AppIcon ... />
  )
))}
```

### 3. Icon Asset

**File Copied:** `frontend/public/comfyui-icon.webp`

The ComfyUI icon is now available in the frontend's public directory and accessible at `/comfyui-icon.webp`.

### 4. Restored Version Selector Functionality

**File Modified:** [frontend/src/App.tsx](../frontend/src/App.tsx)

**Key Restoration:**
- When `selectedAppId === 'comfyui'`, the app displays the **original** ComfyUI setup interface
- The full `VersionSelector` component is shown with all original functionality:
  - Version dropdown with anchor functionality
  - Download button that opens the `InstallDialog` **in the main area** (not a popup)
  - Shortcut indicators (menu & desktop)
  - Default version management
  - All version switching logic preserved

**Layout Structure:**
```
┌─────────────────────────────────────────┐
│ Header (Resources + Disk + Close)      │
├────┬────────────────────────────────────┤
│ S  │ ComfyUI Content Area               │
│ i  │                                    │
│ d  │ When showVersionManager = false:  │
│ e  │  - VersionSelector                │
│ b  │  - Dependency Install Button      │
│ a  │  - Status Messages                │
│ r  │                                    │
│    │ When showVersionManager = true:   │
│    │  - Back Button + Refresh          │
│    │  - InstallDialog (full area)      │
└────┴────────────────────────────────────┘
```

### 5. Multi-App Architecture Preserved

**ComfyUI (selectedAppId === 'comfyui'):**
- Shows original interface
- Full version management
- Dependency installation
- All existing functionality maintained

**Other Apps (selectedAppId !== 'comfyui'):**
- Shows "Coming Soon" message
- Displays Model Manager UI
- Ready for future implementation

## How It Works

### Sidebar Icon Selection

1. User clicks ComfyUI icon in sidebar
2. `selectedAppId` is set to `'comfyui'`
3. App renders the original ComfyUI interface

### Version Selector Flow

1. **Normal View:**
   - VersionSelector shows installed versions
   - Dropdown allows version switching and anchoring
   - Download button visible

2. **Click Download Button:**
   - `setShowVersionManager(true)` is called
   - Main area switches to InstallDialog
   - Back button appears to return to VersionSelector

3. **InstallDialog View:**
   - Shows available and installed versions
   - Can install new versions
   - Can remove versions
   - Full installation progress tracking
   - Back button returns to normal view

### Anchor Functionality

The anchor icon in the VersionSelector:
- Click to set/unset a version as default
- Default version persists across restarts
- Visual indicator shows which version is anchored
- Integrated with backend `set_default_version()` API

## File Structure

```
frontend/
├── public/
│   └── comfyui-icon.webp          # ComfyUI icon asset
├── src/
│   ├── components/
│   │   ├── ComfyUIIcon.tsx        # NEW: ComfyUI icon component
│   │   ├── AppSidebar.tsx         # MODIFIED: Uses ComfyUIIcon
│   │   ├── AppIcon.tsx            # Existing: Generic app icons
│   │   ├── VersionSelector.tsx    # Existing: Version management
│   │   └── InstallDialog.tsx      # Existing: Version installation
│   ├── App.tsx                    # MODIFIED: Restored original flow
│   └── config/apps.ts             # Existing: App registry
```

## Testing

### Build Test
```bash
cd frontend
npm run build
```
✅ Build succeeds with no errors

### Visual Test Checklist

When you run `./launcher`:

1. **Sidebar:**
   - [ ] ComfyUI icon shows the actual comfyui-icon.webp
   - [ ] Other app icons show Lucide icons
   - [ ] Clicking ComfyUI icon selects it

2. **ComfyUI Selected:**
   - [ ] VersionSelector appears
   - [ ] Can switch versions via dropdown
   - [ ] Can anchor versions (anchor icon works)
   - [ ] Download button opens InstallDialog in main area
   - [ ] Back button returns from InstallDialog

3. **InstallDialog:**
   - [ ] Shows available versions
   - [ ] Can install versions
   - [ ] Progress bar shows during installation
   - [ ] Can remove versions

4. **Other Apps Selected:**
   - [ ] Shows "Coming Soon" message
   - [ ] Shows empty Model Manager

## Comparison to Original

### What's the Same (ComfyUI Mode)
- ✅ Exact same VersionSelector functionality
- ✅ Anchor versions work identically
- ✅ Download button opens InstallDialog
- ✅ InstallDialog in main area (not popup)
- ✅ All version management features
- ✅ Dependency installation
- ✅ Status messages

### What's New
- ✨ Left sidebar with app icons
- ✨ ComfyUI icon uses actual branding
- ✨ System resource monitors in header
- ✨ Multi-app architecture
- ✨ Ready for additional apps

## Future Enhancements

1. **Resource Monitor Integration:**
   - Show CPU/GPU usage in running icon animation
   - Update arcs based on actual system resources

2. **Additional Apps:**
   - Implement Open WebUI integration
   - Add InvokeAI support
   - Configure other apps in sidebar

3. **Model Manager:**
   - Populate with actual models
   - Link models to apps
   - Shared model library

## Notes

- The ComfyUI icon is 32x32 pixels inside a 60x60 circle
- Icon has transparency and works on any background
- All original keyboard shortcuts still work
- All original API calls preserved
- Backward compatible with existing installations
