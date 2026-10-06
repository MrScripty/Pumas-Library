# Explicit wheel target contract

Status: implemented data/preflight and optional native-consumer validation slice.
The automatic/preview resolver migration, upstream catalog authority and AQ gates
remain unaccepted. This contract supplies package compatibility data; it grants
no source, redirect, filesystem, interpreter execution or installation authority.

`torch-server/wheel_target.py` owns `pumas.wheel-target.v1`. An explicit target has
exactly these fields; unknown, missing, contradictory and incorrectly typed
fields refuse without native-host inference:

| Field | Meaning |
| --- | --- |
| `schema` | Exact `pumas.wheel-target.v1` |
| `python` | Explicit stable CPython `major.minor.patch`, 3.10+; no patch-zero/minor-only inference |
| `abi` | Exact normal GIL CPython ABI for that version, e.g. `cp312`; debug/free-threaded/PyPy unsupported |
| `os`, `arch` | Existing native-owner scope: Linux/x86_64, Windows/x86_64, macOS/arm64; other or translated targets unsupported |
| `libc` | Linux requires exact `{family, version}`: glibc2.5+ or musl1.x, with explicit bounded major.minor; other OSes require null |
| `macos_deployment` | macOS arm64 requires explicit deployment major.minor >=11; other OSes require null |
| `native_linux_tag` | Required Boolean; explicitly admit or exclude nonportable `linux_x86_64` tags; false on other OSes |
| `markers` | Complete bounded string environment listed below; no absent/host-derived values |

Required marker keys are `implementation_name`, `implementation_version`,
`os_name`, `platform_machine`, `platform_python_implementation`,
`platform_release`, `platform_system`, `platform_version`, `python_full_version`,
`python_version` and `sys_platform`. Python/implementation and OS/architecture
values must agree with the other fields. Every field, including kernel/build
strings, is supplied by the target owner. `extra` is evaluated separately by the
existing package closure owner; dependency-group contexts are not implicit target
environment fields.

Generate tags using public standalone packaging APIs with **explicit** version,
ABI, interpreter and platform inputs. No inspection-host platform, ABI/GIL/debug
flags or patch values may fill gaps. Windows uses win_amd64. Linux glibc policy
uses PEP600 floors down to the supported x86_64 glibc2.5 floor, including the
standard legacy aliases; musl uses PEP656 floors within its declared major.
macOS uses public `mac_platforms` with explicit deployment and arm64, preserving
supported universal2 tags. Native Linux tags are a distinct declared policy;
they do not claim portable libc evidence. Compatibility is label-level package
policy, not proof of arbitrary binary execution on every provider/target.

`WheelTarget(document)` validates supplied data without observing the host.
`marker_environment`, `tags`, `allows_python`, `supports` and `to_dict` expose the
same checked target for metadata preflight, closure, compatibility and provenance.
The input/returned JSON and marker map cannot mutate the captured marker snapshot.
`capture_native()` is a separate explicit operation, run **inside the selected
interpreter**. It reuses public native marker/tag observations, establishes libc
compatibility policy from native tags, and refuses unsupported/ambiguous native
observations. It must not be used to guess another interpreter/target.

`install_verified_wheels.local_requirements(..., wheel_target=document)` uses
that exact target for filename/WHEEL compatibility, Requires-Python and actual
METADATA dependency markers/extras. `install(..., wheel_target=document)` first
rechecks the selected native interpreter's complete markers/ABI and supported
tag set; foreign or stale target data refuses **before** stage creation or pip.
The consumer CLI accepts optional resolution field `wheel_target`; an explicitly
present null/incomplete value refuses. Absent fields retain the existing native
consumer and qualified-recipe behavior. The embedded helper is materialized with
the existing trusted scripts. No implicit cross-platform installation is added.

Resolver adapters must consume the same target and verify the public tool can
represent it. The current offline experiment passes the full patch version and
explicit Windows or qualified glibc floor. It rejects unsupported libc/deployment
projections, native-Linux tag exclusion, and marker variables whose target values
the pinned CLI cannot override, including platform_release/platform_version.
Rejection applies even to inactive branches. It never substitutes generic Linux,
inspection-host strings, another interpreter or a source fallback. Unsupported
projection does not mean the target itself is globally incompatible. Exact
direct-root artifact bindings and original source provenance remain separate.

Targets are owner declarations, not self-authenticating evidence. Future
automatic/preview integration must bind the approved target/projection to request,
resolution and consumer provenance, identify the actual selected interpreter,
and retain existing custody/recovery requirements. The finite catalog authority,
complete enumeration/byte budget and full resolver/consumer/platform qualification
remain separate acceptance work. The experimental resolver is not adopted here.

Sources: [platform tags](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/),
[public packaging tags](https://packaging.pypa.io/en/stable/tags.html) and
[public uv CLI](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile).
