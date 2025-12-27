# Troubleshooting Guide

This guide covers common issues and their solutions for the Linux ComfyUI Launcher.

## Quick Diagnostics

Before troubleshooting, run the system dependency checker:

```bash
scripts/system-check.sh
```

This will verify all required system packages are installed (Python 3.12+, Node.js, npm, GTK3, WebKitGTK).

## Table of Contents

1. [Installation & Launch Issues](#installation--launch-issues)
2. [Version Management Issues](#version-management-issues)
3. [Shortcut & Desktop Integration Issues](#shortcut--desktop-integration-issues)
4. [Performance & Resource Issues](#performance--resource-issues)
5. [Development & Build Issues](#development--build-issues)
6. [System Compatibility Issues](#system-compatibility-issues)

---

## Installation & Launch Issues

### Install Dialog Shows "No Versions Available"

**Symptom:**
When you click the download button to open the Install Dialog, it shows "No versions available" even though versions should be available from GitHub.

**Root Cause:**
Python version manager fails to initialize due to incorrect module import paths. The terminal shows:
```
Warning: Version management initialization failed: No module named 'backend'
```

**Solution:**

DO NOT run the app like this:
```bash
# ❌ WRONG - Breaks Python imports
python3 backend/main.py
```

USE ONE OF THESE METHODS:

**Method 1: Use launcher script (END USERS - RECOMMENDED)**
```bash
cd /path/to/Linux-ComfyUI-Launcher
./launcher
```

**Method 2: Use dev runner (DEVELOPERS)**
```bash
cd /path/to/Linux-ComfyUI-Launcher
scripts/dev/run-dev.sh
```

**Method 3: Run as Python module (MANUAL)**
```bash
cd /path/to/Linux-ComfyUI-Launcher
source venv/bin/activate
python3 backend/main.py
```

**Why This Happens:**
- Direct execution with incorrect paths can cause module import issues
- The launcher wrapper and dev scripts handle paths correctly
- Manual activation requires proper venv activation first

**Verification:**

After launching correctly, you should see:

Terminal output:
```
[DEBUG] get_available_versions called (force_refresh=False)
[DEBUG] Retrieved 30 versions from backend
```

Browser console (F12):
```
Total available: 30 Filtered: 30
```

UI:
- Install Dialog shows list of ComfyUI versions
- Version numbers like "v0.5.1", "v0.4.0" appear
- Dialog shows "30 versions" (or similar)

**Quick Diagnostic:**
```bash
cd /path/to/Linux-ComfyUI-Launcher
python3 diagnose_imports.py
```

If you see "✓ SUCCESS" and "Retrieved 30 versions", your backend works correctly.

---

### Installation Fails Mid-Process

**Symptom:**
Installation starts but fails during Download, Extract, Venv, Dependencies, or Setup stage.

**Common Causes & Solutions:**

**1. Network Issues (Download stage fails):**
```bash
# Check internet connection
ping github.com

# Check GitHub API access
curl -I https://api.github.com/repos/comfyanonymous/ComfyUI/releases

# If behind proxy, set environment variables
export HTTP_PROXY=http://proxy.example.com:8080
export HTTPS_PROXY=http://proxy.example.com:8080
```

**2. Disk Space Issues:**
```bash
# Check available space
df -h .

# Each ComfyUI version needs ~2-3GB
# Models shared across versions (one-time cost)
```

**3. Dependency Installation Fails:**
Check installation log:
```bash
cat launcher-data/logs/installation-[version]-[timestamp].log
```

Common issues:
- Missing build tools: `sudo apt install build-essential python3-dev`
- Pip timeout: Retry installation or use different PyPI mirror
- Package conflicts: Check log for specific error

**4. Permission Issues:**
```bash
# Ensure launcher directory is writable
ls -la comfyui-versions/
chmod -R u+w launcher-data/ comfyui-versions/
```

**Recovery Steps:**
1. Cancel the failed installation (if still running)
2. Delete partial installation:
   ```bash
   rm -rf comfyui-versions/[failed-version]
   ```
3. Check logs for root cause
4. Fix the issue (disk space, network, dependencies)
5. Retry installation

---

### Installation Hangs or Freezes

**Symptom:**
Installation progress stops updating, stays at same percentage for long time.

**Diagnosis:**

**1. Check if process is actually running:**
```bash
ps aux | grep python
ps aux | grep pip
```

**2. Check installation log for activity:**
```bash
tail -f launcher-data/logs/installation-[version]-[timestamp].log
```

**3. Check network activity (if in Download stage):**
```bash
# Install nethogs if not present
sudo apt install nethogs
sudo nethogs

# Look for download activity
```

**Solutions:**

**If frozen during Dependencies stage:**
- Large packages like `torch` can take 5-15 minutes to download
- Log may not update frequently during wheel download
- Wait at least 20 minutes before cancelling

**If truly frozen:**
1. Cancel installation via UI
2. If cancel button doesn't work, kill process:
   ```bash
   pkill -f "install_version"
   ```
3. Clean up partial installation:
   ```bash
   rm -rf comfyui-versions/[version]
   ```
4. Check system resources:
   ```bash
   free -h  # Memory
   df -h    # Disk
   top      # CPU usage
   ```
5. Retry installation

---

### Can't Cancel Installation

**Symptom:**
Clicking "Cancel" button doesn't stop the installation.

**Cause:**
Subprocess may be in uninterruptible state (e.g., downloading large file).

**Solution:**

**1. Wait 10-15 seconds after clicking Cancel**
- Cancellation sends SIGTERM first (graceful)
- After 5s timeout, sends SIGKILL (forceful)

**2. If still running, manually kill:**
```bash
# Find the pip process
ps aux | grep pip

# Kill it (replace PID)
kill -9 <PID>

# Or kill all pip processes
pkill -9 pip
```

**3. Clean up partial installation:**
```bash
rm -rf comfyui-versions/[version]
rm -rf launcher-data/cache/downloads/[archive]
```

**4. Restart the launcher**

---

### GTK/WebKitGTK Errors

**Symptom:**
Window doesn't open, or errors like:
```
Gtk-WARNING **: cannot open display:
Could not load WebKitGTK
```

**Solution:**

**1. Install required system packages:**
```bash
sudo apt update
sudo apt install -y libgtk-3-0 libwebkit2gtk-4.1-0 gir1.2-webkit2-4.1 \
                    python3-gi gir1.2-gtk-3.0
```

**2. Verify GTK installation:**
```bash
python3 -c "import gi; gi.require_version('Gtk', '3.0'); from gi.repository import Gtk; print('GTK OK')"
```

**3. If running via SSH or headless:**
- GTK requires X11 display
- Use VNC or connect a monitor
- Cannot run in pure headless mode

**4. If using Wayland:**
```bash
# Force X11 backend
export GDK_BACKEND=x11
./comfyui-setup
```

---

## Version Management Issues

### Version Appears Installed But Can't Launch

**Symptom:**
Version shows as installed in UI, but Launch button doesn't work or shows error.

**Diagnosis:**

**1. Check if version directory exists:**
```bash
ls -la comfyui-versions/[version]/
```

**2. Check for main.py:**
```bash
ls -la comfyui-versions/[version]/main.py
```

**3. Check for virtual environment:**
```bash
ls -la comfyui-versions/[version]/venv/bin/python
```

**4. Check for dependencies:**
```bash
comfyui-versions/[version]/venv/bin/pip list
```

**Solutions:**

**If directory exists but incomplete:**
This is an orphaned installation. The launcher should detect and clean it on startup.

Manual cleanup:
```bash
rm -rf comfyui-versions/[version]
# Restart launcher to update metadata
```

**If directory seems complete but won't launch:**
Check launch log:
```bash
cat launcher-data/logs/launch-[version]-[timestamp].log
```

Common issues:
- Port 8188 already in use
- Missing dependencies
- Corrupted Python venv

**Recovery:**
1. Remove the version completely
2. Reinstall from Install Dialog

---

### Can't Switch Between Versions

**Symptom:**
Selecting different version in dropdown doesn't change active version.

**Diagnosis:**

**1. Check browser console (F12):**
Look for JavaScript errors

**2. Check terminal output:**
Look for Python exceptions

**Solutions:**

**If UI doesn't respond:**
- Refresh the launcher window (Ctrl+R in dev mode)
- Close and restart launcher

**If switch appears to work but Launch uses wrong version:**
- Check `launcher-data/metadata/active_version.json`
- Verify `defaultVersion` or `lastSelectedVersion` is set correctly

**Manual fix:**
```bash
# Edit active_version.json
nano launcher-data/metadata/active_version.json

# Set to desired version:
{
  "lastSelectedVersion": "v0.6.0"
}
```

---

### Wrong Version Launches

**Symptom:**
Selected version v0.6.0 but v0.5.1 actually launches.

**Cause:**
Multiple instances running, or previous instance not killed.

**Solution:**

**1. Check for running ComfyUI processes:**
```bash
ps aux | grep "main.py"
```

**2. Kill all ComfyUI instances:**
```bash
pkill -f "comfyui.*main.py"
```

**3. Check PID files:**
```bash
ls -la launcher-data/*.pid
cat launcher-data/[version].pid
```

**4. Remove stale PID files:**
```bash
rm -f launcher-data/*.pid
```

**5. Launch again from launcher**

---

## Shortcut & Desktop Integration Issues

### Desktop Shortcuts Don't Appear

**Symptom:**
Toggle "Desktop Shortcuts" on, but no icon appears on desktop.

**Diagnosis:**

**1. Check if .desktop file was created:**
```bash
ls -la ~/Desktop/ComfyUI-*.desktop
```

**2. Check file permissions:**
```bash
ls -la ~/Desktop/ComfyUI-v0.6.0.desktop
# Should be executable (-rwxr-xr-x)
```

**Solutions:**

**If file exists but not executable:**
```bash
chmod +x ~/Desktop/ComfyUI-*.desktop
```

**If file doesn't exist:**
1. Check launcher logs for errors
2. Verify desktop directory exists:
   ```bash
   ls -ld ~/Desktop
   ```
3. Some desktop environments use different paths:
   ```bash
   # Try these locations:
   ls -la ~/Desktop/
   ls -la ~/Schreibtisch/  # German
   ls -la ~/Bureau/        # French
   ```

**Manual shortcut creation:**
```bash
# Create .desktop file
cat > ~/Desktop/ComfyUI-v0.6.0.desktop << EOF
[Desktop Entry]
Type=Application
Name=ComfyUI v0.6.0
Exec=/path/to/launcher-data/shortcuts/launch-v0.6.0.sh
Icon=comfyui-v0.6.0
Terminal=false
Categories=Graphics;
EOF

chmod +x ~/Desktop/ComfyUI-v0.6.0.desktop
```

---

### Application Menu Shortcuts Don't Appear

**Symptom:**
Toggle "Application Menu Shortcuts" on, but shortcut doesn't appear in application launcher.

**Solutions:**

**1. Check if .desktop file was created:**
```bash
ls -la ~/.local/share/applications/comfyui-*.desktop
```

**2. Update desktop database:**
```bash
update-desktop-database ~/.local/share/applications/
```

**3. Update icon cache:**
```bash
gtk-update-icon-cache ~/.local/share/icons/hicolor/ -f
```

**4. Refresh application menu:**
- Log out and log back in
- Or restart desktop environment
- Or run: `killall gnome-shell` (GNOME), `kquitapp5 plasmashell && kstart5 plasmashell` (KDE)

**5. Check if icon is installed:**
```bash
ls -la ~/.local/share/icons/hicolor/*/apps/comfyui-*.png
```

---

### Shortcut Launches Wrong Browser

**Symptom:**
Shortcut opens ComfyUI in default browser instead of Brave, or opens wrong profile.

**Cause:**
Brave browser not found, fallback to xdg-open.

**Solution:**

**1. Check if Brave is installed:**
```bash
which brave-browser
```

**2. Install Brave:**
```bash
sudo apt install brave-browser
```

**Or download from:** https://brave.com/linux/

**3. Verify launch script uses Brave:**
```bash
cat launcher-data/shortcuts/launch-v0.6.0.sh | grep brave
```

**4. Manually edit launch script if needed:**
```bash
nano launcher-data/shortcuts/launch-v0.6.0.sh
```

Change `xdg-open` to `brave-browser --user-data-dir=...`

---

## Performance & Resource Issues

### High Memory Usage

**Symptom:**
Launcher or ComfyUI instances using excessive RAM.

**Diagnosis:**

```bash
# Check memory usage
free -h
top
htop

# Check per-process memory
ps aux --sort=-%mem | head -20
```

**Normal Memory Usage:**
- Launcher (PyWebView): 80-150 MB
- ComfyUI instance: 500-1500 MB (depends on loaded models)
- During installation: +100-200 MB (pip subprocess)

**Solutions:**

**If launcher using >300 MB:**
- Memory leak possible (shouldn't happen)
- Restart launcher

**If ComfyUI using >2 GB:**
- Normal for large models (SDXL checkpoints)
- Close unused browser tabs in ComfyUI
- Unload models when not in use
- Consider upgrading RAM

**If system running out of memory:**
```bash
# Check swap
free -h

# Add swap if needed (8GB example)
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

---

### Disk Space Running Out

**Symptom:**
System running low on disk space after installing multiple versions.

**Diagnosis:**

```bash
# Check disk usage
df -h

# Check ComfyUI versions size
du -sh comfyui-versions/*

# Check models size
du -sh shared-resources/models/*

# Check cache size
du -sh launcher-data/cache/
```

**Cleanup Strategies:**

**1. Remove unused versions:**
```bash
# List installed versions
ls -la comfyui-versions/

# Remove old version (example: v0.3.0)
rm -rf comfyui-versions/v0.3.0
```

Or use launcher UI to switch version first, then reinstall if needed later.

**2. Clear download cache:**
```bash
# Archives are kept after installation
rm -rf launcher-data/cache/downloads/*
```

**3. Clear pip cache:**
```bash
rm -rf launcher-data/cache/pip/*
```

**4. Remove unused models:**
```bash
# List large models
du -sh shared-resources/models/*/* | sort -h

# Remove specific model
rm shared-resources/models/checkpoints/huge_model.safetensors
```

**5. Compress old output images:**
```bash
# ComfyUI outputs stored per version
du -sh comfyui-versions/*/output/

# Archive and compress
tar -czf old-outputs-v0.5.0.tar.gz comfyui-versions/v0.5.0/output/
rm -rf comfyui-versions/v0.5.0/output/*
```

---

### Slow Installation

**Symptom:**
Installation taking very long time (>30 minutes).

**Common Causes:**

**1. Slow network:**
```bash
# Test download speed
wget -O /dev/null http://speedtest.tele2.net/100MB.zip

# If slow, installation will be slow
# Dependencies stage downloads large packages (torch ~800MB)
```

**2. Slow disk:**
```bash
# Test disk speed
sudo hdparm -Tt /dev/sda

# If <50 MB/s, disk is slow
```

**3. Limited CPU:**
- Package extraction and compilation slower on old CPUs
- Normal on older hardware

**Solutions:**

- Be patient (first install can take 20-30 minutes)
- Use wired connection instead of WiFi
- Close other applications during installation
- Consider using faster disk (SSD recommended)

---

## Development & Build Issues

### Frontend Build Fails

**Symptom:**
`npm run build` fails with errors.

**Solutions:**

**1. Clear and reinstall dependencies:**
```bash
cd frontend
rm -rf node_modules package-lock.json dist
npm install
npm run build
```

**2. Check Node.js version:**
```bash
node --version  # Should be 18+
npm --version   # Should be 8+
```

**3. Update Node.js if needed:**
```bash
# Using nvm (recommended)
nvm install 20
nvm use 20

# Or using apt
sudo apt install nodejs npm
```

**4. Check for TypeScript errors:**
```bash
cd frontend
npm run type-check
```

Fix any type errors before building.

---

### PyInstaller Build Fails

**Symptom:**
`scripts/dev/build.sh` fails during PyInstaller step.

**Solutions:**

**1. Upgrade PyInstaller:**
```bash
source venv/bin/activate
pip install --upgrade pyinstaller
```

**2. Clean build artifacts:**
```bash
rm -rf build dist
scripts/dev/build.sh
```

**3. Check Python version:**
```bash
python3 --version  # Should be 3.12+
```

**4. Check for missing dependencies:**
```bash
source venv/bin/activate
pip install -r requirements.txt
```

**5. Check spec file:**
```bash
# Ensure comfyui-setup.spec exists and is valid
cat comfyui-setup.spec
```

---

### Development Mode Not Working

**Symptom:**
Running `scripts/dev/run-dev.sh` but PyWebView API doesn't work, or shows dev mode message.

**Diagnosis:**

**1. Check if Vite dev server is running:**
```bash
# Should see "Local: http://127.0.0.1:3000" in this terminal
cd frontend
npm run dev
```

**2. Check if frontend/dist exists:**
```bash
ls -la frontend/dist/index.html
```

If exists, app will use production mode instead of connecting to Vite.

**Solutions:**

**Temporary move dist for dev mode:**
```bash
mv frontend/dist frontend/dist.bak
```

**Stop Vite server when building:**
```bash
# Ctrl+C in the Vite terminal before running build.sh
```

**Restore dist after development:**
```bash
mv frontend/dist.bak frontend/dist
```

---

## System Compatibility Issues

### Doesn't Work on Non-Debian Distributions

**Symptom:**
Launcher doesn't run on Fedora, Arch, OpenSUSE, etc.

**Cause:**
Launcher is primarily tested on Debian-based distros (Ubuntu, Mint, Pop!_OS).

**Workarounds:**

**For Fedora/RHEL:**
```bash
sudo dnf install gtk3 webkit2gtk3 python3-gobject
```

**For Arch:**
```bash
sudo pacman -S gtk3 webkit2gtk python-gobject
```

**For OpenSUSE:**
```bash
sudo zypper install gtk3 webkit2gtk3 python3-gobject
```

Then try running the launcher.

**If still doesn't work:**
- Package names may differ
- GTK bindings may be incompatible
- Consider running in Docker or using Debian-based VM

---

### Display Issues on HiDPI Screens

**Symptom:**
Launcher window appears tiny or huge on HiDPI displays.

**Solution:**

**Force specific DPI:**
```bash
export GDK_SCALE=2  # For 2x scaling
export GDK_DPI_SCALE=0.5
./comfyui-setup
```

**Or edit backend/main.py:**
```python
# Add before webview.create_window():
import os
os.environ['GDK_SCALE'] = '2'
os.environ['GDK_DPI_SCALE'] = '0.5'
```

---

### Window Decorations Missing

**Symptom:**
Launcher window has no title bar, can't move/resize.

**Cause:**
Some window managers (tiling WMs) don't show decorations for certain window types.

**Solution:**

**Edit backend/main.py:**
```python
window = webview.create_window(
    ...
    frameless=False,  # Ensure this is False
    ...
)
```

**For tiling WMs (i3, sway):**
Add rule to float the launcher window.

i3 config:
```
for_window [title="ComfyUI Launcher"] floating enable
```

---

## Still Having Issues?

If your issue isn't covered here:

**1. Check logs:**
```bash
# Installation logs
ls -la launcher-data/logs/installation-*.log
cat launcher-data/logs/installation-[version]-[timestamp].log

# Launch logs
ls -la launcher-data/logs/launch-*.log
cat launcher-data/logs/launch-[version]-[timestamp].log
```

**2. Run diagnostic script:**
```bash
python3 diagnose_imports.py
```

**3. Enable debug mode:**
Edit `backend/main.py`:
```python
webview.start(debug=True)  # Change to True
```

Then check terminal output and browser console (F12).

**4. Open an issue on GitHub with:**
- Linux distribution and version (`lsb_release -a`)
- Launcher version
- Relevant log files
- Steps to reproduce
- Screenshot of error (if applicable)

**5. Check existing issues:**
- https://github.com/[your-repo]/issues
- Your issue might already be reported/solved
