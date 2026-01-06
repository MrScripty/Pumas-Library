# Icon and Launch Button Updates

This document describes the latest updates to the icon scaling and contextual launch button.

## Changes Made

### 1. Removed Unused Apps

**File Modified:** [frontend/src/config/apps.ts](../frontend/src/config/apps.ts)

**Removed:**
- SD WebUI (sdwebui)
- Fooocus

**Remaining Apps:**
1. ComfyUI
2. Open WebUI
3. InvokeAI
4. Krita Diffusion

**Reasoning:** These apps won't be added to the launcher, so they were removed to simplify the UI.

### 2. Icon Scaling Update

**File Modified:** [frontend/src/components/ComfyUIIcon.tsx](../frontend/src/components/ComfyUIIcon.tsx)

**Change:**
```typescript
// Before
className="w-8 h-8 object-contain"

// After
className="w-full h-full object-cover p-1"
```

**Effect:**
- ComfyUI icon now fills the entire circle
- Small padding (p-1) prevents edge clipping
- Icon is scaled up from 32x32px to fill the full 60x60px circle

**Visual Result:**
- Before: Icon was centered and small within the circle
- After: Icon fills the entire circle with minimal padding

### 3. Contextual Launch Button

**File Modified:** [frontend/src/App.tsx](../frontend/src/App.tsx)

**Added:** Launch button in header that appears **only** when ComfyUI is selected

**Features:**
- Shows when `selectedAppId === 'comfyui'`
- Hides when other apps are selected
- Identical behavior to original commit a0c3b27

**Launch Button States:**

1. **Idle (Can Launch):**
   - Green Play icon with glow effect
   - Text: "Launch"
   - Shows active version

2. **Running:**
   - Animated spinner (`/`, `-`, `\`, `|`)
   - Text: "Running"
   - Hover: Shows red Stop icon with text "Stop"

3. **Error:**
   - Flashing between Alert icon and Play icon
   - Red color scheme
   - Shows version

4. **Disabled (Can't Launch):**
   - Grayed out
   - Text: "No version installed"
   - Not clickable

**Additional Features:**
- **Log Button:** Appears next to launch button when a log file exists
  - Red icon if there's an error
  - Gray icon if successful
  - Click to open log file

## Header Layout

```
┌────────────────────────────────────────────────────────────────┐
│ [CPU] [GPU]  │  [Disk Space]  │  [Launch][Log][v1.0][▲][✕]   │
│ [RAM]        │                │                                │
└────────────────────────────────────────────────────────────────┘
     Resources      Center              Right Controls

When ComfyUI selected:
- Launch button visible
- Log button visible (if log exists)

When other app selected:
- Launch button hidden
- More space for version/close buttons
```

## Icon States Visual Guide

### ComfyUI Icon (All States)

**Running:**
- Full-size icon filling circle
- Green background with 40% opacity
- Green border
- Animated CPU/GPU usage arcs
- Green indicator dot on right edge

**Offline:**
- Full-size icon filling circle
- Dark background
- Gray border
- Small play icon on right edge
- 80% opacity

**Uninstalled:**
- Full-size icon filling circle
- Dark background
- Gray border
- Small play icon on right edge
- 60% opacity (more faded)

**Error:**
- Full-size icon filling circle
- Dark background
- Gray border
- Red warning triangle on right edge
- 80% opacity

## Code Architecture

### Contextual Rendering

The launch button uses conditional rendering based on selected app:

```typescript
{selectedAppId === 'comfyui' && (
  <>
    <motion.button onClick={handleLaunchComfyUI}>
      {/* Launch Button Content */}
    </motion.button>
    {launchLogPath && (
      <motion.button onClick={openLogPath}>
        {/* Log Button */}
      </motion.button>
    )}
  </>
)}
```

This ensures:
1. Launch button only appears for ComfyUI
2. Other apps get cleaner header
3. Easy to extend for other apps in future

## Testing Checklist

When you run `./launcher`:

### Icon Tests
- [ ] ComfyUI icon fills the entire circle
- [ ] Icon has small padding around edges
- [ ] Icon is clear and not pixelated
- [ ] All 4 states (running/offline/uninstalled/error) show correctly

### Launch Button Tests (ComfyUI Selected)
- [ ] Launch button appears in header
- [ ] Button is disabled when no version installed
- [ ] Button shows "Launch" when idle
- [ ] Clicking launches ComfyUI
- [ ] Button shows spinner when running
- [ ] Hovering while running shows "Stop"
- [ ] Clicking while running stops ComfyUI
- [ ] Log button appears when log exists
- [ ] Log button opens log file

### Contextual Behavior
- [ ] Launch button hidden when OpenWebUI selected
- [ ] Launch button hidden when InvokeAI selected
- [ ] Launch button hidden when Krita Diffusion selected
- [ ] Launch button reappears when ComfyUI selected again

## Future Enhancements

### Per-App Launch Buttons

When other apps are implemented, add launch buttons for them:

```typescript
{selectedAppId === 'comfyui' && (
  <LaunchButton app="comfyui" ... />
)}
{selectedAppId === 'openwebui' && (
  <LaunchButton app="openwebui" ... />
)}
```

### Shared Launch Component

Create a reusable `AppLaunchButton` component:

```typescript
<AppLaunchButton
  appId={selectedAppId}
  isRunning={appRunning}
  canLaunch={appCanLaunch}
  onLaunch={handleLaunch}
  onStop={handleStop}
/>
```

## File Summary

**Modified Files:**
1. [frontend/src/config/apps.ts](../frontend/src/config/apps.ts) - Removed SD WebUI and Fooocus
2. [frontend/src/components/ComfyUIIcon.tsx](../frontend/src/components/ComfyUIIcon.tsx) - Scaled icon to fill circle
3. [frontend/src/App.tsx](../frontend/src/App.tsx) - Added contextual launch button to header

**Build Status:** ✅ Successful

**Backward Compatibility:** ✅ Maintained - All original functionality preserved
