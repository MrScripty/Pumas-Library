# Pumas Native Bindings

Pumas ships a native binding surface through the `pumas_uniffi` shared library.
Host-language consumers use generated bindings plus the matching native library;
they do not need to link directly against Rust crates.

## Architecture

```text
Host app
  -> generated host-language binding
    -> pumas_uniffi native library
      -> pumas-uniffi adapter crate
        -> pumas-core
```

The adapter crate owns FFI-safe records, enums, objects, and flattened error
handling. Generated host-language code is derived from the compiled native
library and must be regenerated when the exported UniFFI surface changes.

## C# Artifact Layout

The generated C# binding package contains:

```text
bindings/csharp/pumas_uniffi.cs
bindings/csharp/pumas_library.cs
docs/native-bindings.md
README.md
manifest.json
```

The generated `.cs` files are artifacts, not checked-in source files.

## Native Artifact Layout

The native package contains:

```text
native/<platform>/libpumas_uniffi.so
native/<platform>/pumas_uniffi.dll
native/<platform>/libpumas_uniffi.dylib
docs/native-bindings.md
README.md
manifest.json
```

Only one native library is present per platform package.

## Compatibility

- Keep generated bindings and the native library from the same build or
  release.
- The internal native module identity remains `pumas_uniffi` in this release
  flow even when outer package names are product-facing.
- Generated code is disposable and must not be hand-edited.

## C# Loading

Generated C# resolves the native library using the .NET runtime's normal
native-library lookup rules.

```bash
# Linux
export LD_LIBRARY_PATH=/path/to/native/linux-x64:$LD_LIBRARY_PATH

# macOS
export DYLD_LIBRARY_PATH=/path/to/native/osx:$DYLD_LIBRARY_PATH

# Windows PowerShell
$env:PATH = "C:\path\to\native\win-x64;$env:PATH"
```

Applications may also copy the platform library into their output directory.

## Verification Workflow

Use the repo scripts to validate the binding surface before packaging:

```bash
./scripts/check-uniffi-surface.sh
./scripts/check-uniffi-csharp-smoke.sh
./scripts/package-uniffi-csharp-artifacts.sh
```
