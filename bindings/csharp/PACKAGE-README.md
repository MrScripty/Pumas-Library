# Pumas C# Bindings

This package contains generated C# bindings for the Pumas native shared
library.

## Contents

| Path | Purpose |
| ---- | ------- |
| `bindings/csharp/pumas_uniffi.cs` | Primary generated C# binding surface for `pumas-uniffi`. |
| `bindings/csharp/pumas_library.cs` | Generated support types required by the C# surface. |
| `docs/native-bindings.md` | Native binding contract, compatibility notes, and loader guidance. |
| `manifest.json` | Machine-readable package summary. |

## Required Native Library

This package does not bundle the native Pumas shared library by default.
Download the matching `pumas-library-native-<platform>.zip` package from the
same build or release and place the platform library next to your application
binary or on the platform's native-library search path.

Do not mix generated C# files from one build with a native library from
another build.

## Minimal Usage

```csharp
using uniffi.pumas_uniffi;

Console.WriteLine(PumasUniffiMethods.Version());
```

For artifact layout and native-library loading guidance, see
`docs/native-bindings.md`.
