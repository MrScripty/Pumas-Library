# Pumas C# Binding Package

This archive contains generated C# sources for `pumas_uniffi`. Its
`manifest.json` names the required native library and the separately packaged
native archive for the same platform and Cargo profile.

Keep the generated sources and native library from the same Pumas version,
commit, target, and build profile. Place the native library beside the host
executable or on the platform's native-library search path before loading the
binding.

These bindings are experimental and not a supported SDK. The current smoke
evidence covers native loading and a synchronous version call, not the full
async or model-library API.
See `docs/native-bindings.md` in the archive for integration constraints and
the evidence required before claiming host support.
