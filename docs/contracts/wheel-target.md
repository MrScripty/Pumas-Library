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
present value now requires the observation binding below; null/incomplete values
refuse. Absent context retains the existing native consumer and qualified-recipe
behavior. The embedded helper is materialized with the existing trusted scripts.
No implicit cross-platform installation is added. Native observation checks the
reconstructed tag set against actual public interpreter tags, so Windows native
machine reporting cannot admit win_amd64 for a win32 interpreter. Public debug or
free-threaded ABI tags refuse even when that interpreter also accepts normal ABI
extensions; Windows missing `Py_DEBUG` does not create a normal-ABI exemption.

## Approved observation and accepted packet

`capture_observation()` (CLI `wheel_target.py --observe`) is run by the trusted
owner in its selected interpreter under existing stage/child custody. It emits
`pumas.wheel-target-observation.v1` with exactly four fields: `schema`, exact
absolute `interpreter` path, lowercase executable `interpreter_sha256`, and the
complete validated `target`. This declaration is not self-authenticating. Only
output of the owned selected-interpreter observation may be supplied as approval;
resolver output, a cache hit, the inspection host or a replacement packet cannot
mint it. Unknown versions, missing/extra fields and unsupported target context
refuse. Observation input is bounded to 64 KiB at the consumer boundary.

An explicit-target resolution carries both `wheel_target` and
`wheel_target_observation_sha256`. The latter is SHA-256 of the whole approved
observation's UTF-8 JSON: recursively sorted object keys, preserved array order,
compact separators, unescaped Unicode, explicit nulls/booleans and every declared
field. Target/observation schemas permit no floating-point fields. Rust uses the
repository JSON serializer; a shared nested/Unicode vector checks the projection.
`bind_resolution` adds those fields without changing original artifact identity.

The private Rust selection boundary receives approved observation separately from
resolution/report bytes. It checks the fixed complete schema, exact target and
observation digest, selected interpreter path/hash and the public report's whole
runtime marker environment before accepting the packet. Standard compatibility
semantics remain with the Python target owner. The selected executable hash is
separate from the existing managed-provider proof; venv redirectors do not replace
or inherit the provider digest. Approved bytes/digest live in the
accepted packet and cannot be decoded from its resolution JSON. Before acquisition
the consumer rechecks retained packet bindings, then materializes the approved
bytes with exclusive creation in its owned runtime. The existing provenance fence
checks those bytes and resolution/report before acquisition, before local pip,
and after the probe. Shared stage/input custody remains live through child effects,
publication, cleanup and settlement. The existing opaque consumer receipt payload
adds the observation digest; the neutral durable schema does not change.

The local CLI takes `--target-observation` from that owner-held provenance. It
validates the complete observation and packet binding, then rechecks native
markers/ABI, compatibility against actual interpreter tags, and executable bytes
before creating package/output staging or invoking pip. A new venv path may use
the original approval only when its executable bytes and complete native context
match. A copied launcher/redirector with different bytes requires independent
approval of the actual selected consumer; a provider label alone cannot renew it.

Compatibility is explicit: only absence of **both** resolution binding fields and
separate approval selects legacy behavior. Missing one field, bare `wheel_target`,
null values, unsupported context or supplied approval with removed fields refuse;
accepted explicit context cannot fall back to legacy. The low-level
`local_requirements`/`install` Python APIs still accept a declared target for
standalone compatibility validation; those APIs alone do not mint accepted packet
authority. Current automatic/retained-preview producers pass no target approval
and keep their existing resolver. They refuse unsolicited target-bearing packets.
The finite recipe remains in its existing absent-context mode. No experimental
resolver or new production catalog is activated by this opt-in boundary.

At the CLI boundary, absence means the `--target-observation` flag was omitted.
If the flag is supplied, its file must decode to an observation object; JSON null,
other scalar/list values, malformed JSON and missing files refuse before package
or output staging and pip, including when both resolution binding fields are
absent. Parsed null must never be converted into absent-approval authority.

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
