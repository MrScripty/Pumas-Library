# Explicit S3 credential boundary — 2026-10-05

The authenticated successor follows frozen anonymous `106a6ca40ac41717854d847214ee3a1cf1e673b4`
and its separately tested desktop-review fixes `887308dc094570fe6072b162074294c3dc580cd0`.
Scope is reported before implementation: first contain S3 transport errors and
decode at privileged IPC receipt, then add a distinct explicit authenticated
start command and one-use form inputs through the existing native operation.
No native reader/import/discovery/repair files are admitted.

## Traced boundaries

| Boundary | Serialization/lifetime | Required containment |
| --- | --- | --- |
| Renderer inputs | Explicit DOM values; no account/configuration/default storage | One-use inputs, cleared on submit, close and mode replacement; no secret in task state or recovery |
| Renderer hook/API | Ephemeral call arguments, UUID/progress/result observation | No secret state/ref, error logging, automatic credential replay or retry after lost acknowledgement |
| Generated preload decoder | Bounded immutable JSON copies, Electron structured clone | Closed source/credential fields and static validation errors; no diagnostic stringify |
| Privileged IPC handler | Another decoded request during startup/short admission call | Decode/project response before IPC; contain initialization/transport errors |
| Node RPC bridge | Explicit JSON request over existing direct 127.0.0.1 control plane | No proxy/redirect forwarding, finite deadline and response bound, matching envelope ID, static transport failures, no reflected raw body/backend message |
| Rust HTTP/parser | Bounded body/serde values consumed into a non-Debug command | Static admission errors and numeric request diagnostics; no body/params tracing |
| RPC worker | One request-owned credential capability | Pending/safe result records contain UUID/control/progress only; move credentials to native operation, never persist/reset/replay them |
| Native reader/selection | Existing opaque in-memory credentials and static SDK provider | HTTPS S3 origin regardless of allow_http; sensitive signing headers, no ambient credentials/proxy/redirect/refresh/fallback |
| Acquisition/import/caches | Existing source pins, byte evidence and ModelImportSpec receipts | No credentials in manifest/receipt payload, publication metadata, output/cache/workspace or telemetry; finalization/retained custody unchanged |
| Child stdout/stderr and errors | Existing bridge forwards native process logs | Native logs include safe method/numeric ID/error class only; SDK headers stay sensitive and public errors stay static; synthetic capture/scan required |

The existing desktop control plane is loopback HTTP and Electron IPC; it is not
a new TLS or remote RPC service. HTTPS remains mandatory for credentialed S3
source access. No new origin, listener exposure, trust exception or security
setting is introduced. JS/serde/string copies are request-scoped; no secure
memory erasure or protection from privileged process inspection is claimed.

## Bounded transport prerequisite

Inspection found two concrete gaps in generic bridge behavior: malformed JSON
errors embed raw response bodies and JSON-RPC errors embed backend message/data;
main also returns a raw response across IPC before preload decoding. These
paths are unsafe for a future request that can encounter reflected credentials.
The prerequisite applies only to the explicit S3 methods and reserves the future
authenticated start name; it exposes no credential input or authentication UI.

S3 calls now require a matching JSON-RPC envelope/ID and a normal successful
HTTP response, cap response bytes at 64 KiB under the existing finite deadline,
use a direct agent with explicit empty proxy configuration, clear the owned
serialized request/body buffers after use, and contain failures behind static
unchained errors. Node and privileged IPC both decode the closed source outcome
before returning it. Safe typed error code/class/custody/model identity remain;
arbitrary error message text is projected to a static S3 message. Other RPC
routes retain their existing behavior. No source retry/object/receipt policy
changes.

Actual controlled Node HTTP tests cover malformed/reflected JSON and JSON-RPC
errors, oversized output, wrong IDs, malformed result shapes, typed reflected
failure messages and throwing request serializers. The actual compiled main IPC
handler is also exercised with malformed values, exceptions and reflected typed
errors. Existing bridge lifecycle, deadlines and bundled-preload regressions
remain required. Browser/packaged/native/default qualification stays separate;
the local sandbox helper is not modified or bypassed.

Raw evidence and subsequent credential qualification are retained under
`/workspace/scratch/s3-desktop-auth/`. Parent owns independent review, hosted
qualification and PR/merge/Library coordination.

## Implemented successor evidence

Transport prerequisite is `0023bcf9954f99c2755395d82b8daa632f73bfcd`;
authenticated successor is `39f7689ab20a10276839a8f5dff206879d53b008`.
The closed credential DTO has no Debug/Clone/Serialize; per-field printable ASCII
is bounded to 4096 bytes, with native access-key delimiter restrictions. Before
admission, bounded temporary copies validate using the actual authenticated
constructor and are dropped; owned originals move into the bounded native job.
Safe Current/task/result records never contain credentials. All credential DOM
nodes clear before the admission wait and on close/replacement/unmount; only
boolean mode is React state. Lost acknowledgement observes UUID without replay.
Actual controlled HTTPS, RPC, compiled preload/main transport and DOM tests cover
these owners, capture and scan DEBUG stdout/stderr and owned files/receipts, and
refuse reflected diagnostics. A proxy-configured global HTTP agent does not
redirect the local request; a 302 response is refused without following Location.
See [complete qualification](s3-desktop-authentication-qualification-2026-10-05.md)
for exact logs and remaining browser/default/platform/provider limits.
