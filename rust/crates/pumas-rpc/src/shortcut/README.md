# Shortcut

## Purpose

Desktop shortcut and menu entry management for launching application versions directly from
the desktop environment. Creates XDG-compliant `.desktop` files, version-specific launch
scripts, and installs application icons. Currently supports Linux; the architecture is
ready for Windows (.lnk) and macOS extensions.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports `ShortcutManager` |
| `manager.rs` | `ShortcutManager` - Orchestrates shortcut creation, removal, and listing |
| `desktop_entry.rs` | `DesktopEntry` - Generates XDG Desktop Entry `.desktop` files |
| `launch_script.rs` | `LaunchScriptGenerator` - Generates bash launch scripts with server start delay and browser profiles |
| `icon.rs` | `IconManager` - Installs icons to XDG icon directories with size variants and cache updates |

## Design Decisions

- **XDG Desktop Entry Specification**: Shortcuts are standard `.desktop` files placed in
  `~/.local/share/applications` (menu) and `~/Desktop` (desktop), ensuring compatibility
  with all major Linux desktop environments.
- **Version-specific launch scripts**: Each version gets its own bash script that handles
  server startup delay and browser profile isolation, rather than parameterizing a single script.

## Dependencies

### Internal
- `pumas_library::error` - Error types
- `pumas_library::platform` - `set_executable` for script permissions

### External
- `tracing` - Structured logging
