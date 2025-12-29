#!/usr/bin/env python3
"""
Diagnostic script to check import paths and version manager initialization
"""
import sys
from pathlib import Path

print("=" * 60)
print("IMPORT DIAGNOSTIC REPORT")
print("=" * 60)

# Check current working directory
print(f"\nCurrent working directory: {Path.cwd()}")

# Check Python path
print(f"\nPython sys.path (first 5 entries):")
for i, path in enumerate(sys.path[:5], 1):
    print(f"  {i}. {path}")

# Check if backend directory exists
backend_dir = Path("backend")
print(f"\nBackend directory exists: {backend_dir.exists()}")
if backend_dir.exists():
    print(f"Backend directory contents:")
    for item in sorted(backend_dir.iterdir()):
        print(f"  - {item.name}")

# Try importing backend.api
print("\n" + "=" * 60)
print("TESTING IMPORTS")
print("=" * 60)

try:
    print("\nAttempting: from backend.api import ComfyUISetupAPI")
    from backend.api import ComfyUISetupAPI

    print("✓ SUCCESS: backend.api imported")

    print("\nAttempting: api = ComfyUISetupAPI()")
    api = ComfyUISetupAPI()
    print("✓ SUCCESS: ComfyUISetupAPI initialized")

    print("\nChecking version manager state:")
    if hasattr(api, "version_manager"):
        if api.version_manager is None:
            print("✗ PROBLEM: api.version_manager is None")
        else:
            print("✓ SUCCESS: api.version_manager exists")

            # Try to get versions
            print("\nAttempting: api.get_available_versions()")
            versions = api.get_available_versions(force_refresh=False)
            print(f"✓ SUCCESS: Retrieved {len(versions)} versions")

            if len(versions) > 0:
                print(f"\nFirst version: {versions[0].get('tag_name', 'unknown')}")
    else:
        print("✗ PROBLEM: api has no version_manager attribute")

except ImportError as e:
    print(f"✗ IMPORT ERROR: {e}")
    print("\nTroubleshooting:")
    print("  1. Make sure you're running from the project root directory")
    print("  2. Use: python run_app.py (NOT python3 backend/main.py)")

except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
    print(f"✗ ERROR: {type(e).__name__}: {e}")

print("\n" + "=" * 60)
print("END OF DIAGNOSTIC REPORT")
print("=" * 60)
