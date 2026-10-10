# Conditional installed audio session ownership

The private Linux x86-64 session producer consumes an already qualified
`AudioRuntimeOwner` and the original `Arc<PreparedArtifactUse>`. It cannot
qualify an installed recipe, accept a caller executable, or enable a model.
Shipping admission remains closed. There is no public session constructor.

Before launch, the prepared source must belong to the retained library's
physical root. The source correlation ID is `pumas-cohere-owned-v1:` followed
by the prepared manifest digest. This is a comparison value, never authority.

The blocking session worker owns child supervision independently of the caller
waiting for startup. It retains the library through every spawn/setup failure
and until confirmed process-tree drainage. Runtime/model ownership is also
attached by the confined child constructor. Registry runtime retention and
child attachment occur atomically before publishing private protocol pipes;
a refused pre-child launch does not leave an unresolved runtime in a registry.

The session creates the original private channel, binds its runtime identity,
loads the original prepared allocation, and registers the resulting endpoint.
A startup guard requests shutdown on timeout, cancellation, or failure. The
caller must retain the returned session owner; an endpoint clone alone cannot
keep admission open after that owner is dropped.

Stopping first closes registry admission and quarantines the channel. The
independent worker retries bounded drain attempts while preserving the exact
child and all byte owners on uncertainty. Completion is published only after
the child tree is drained and its diagnostic reader has joined. Diagnostic EOF
and diagnostic read/thread failure are distinct outcomes. Diagnostic bytes are
consumed without unbounded buffering or exposure to callers.

Controlled regressions cover pre-spawn cancellation, spawn refusal, attachment
and diagnostic setup failures, lost startup waiters, uncertain drain followed by
recovery, atomic retention, and library-root mismatch. These are lifecycle
fixtures, not ASR. Regression fixtures also cover cancellation while the original
child is waiting to answer hello, lost async workers during uncertain drainage,
and retained endpoint clones through retirement/restart. Actual installed-model
startup/load cancellation and real-model execution remain qualification gates.
A successful dependency import probe does not satisfy them.

The runtime-profile service now retains conditional installed sessions in its
private lifecycle owner. It reserves the original profile, model and generation
before polling the deferred installed-byte producer, then consumes only the
qualified opaque runtime and original prepared allocation produced by owned
blocking preparation. It returns the exact generation with the endpoint. It does not replace Torch's
HTTP provider or add a public runtime factory. Binary and audio launches share a
monotonic generation namespace. The profile operation guard remains held through
startup, operation and confirmed drain, excluding overlapping launches, edits,
and deletion. The independent blocking child owner retains that guard even if
the async executor drops its lifecycle task.

Ordinary profile stop first checks installed ownership; generation-specific stop
refuses a mismatched generation. Composed shutdown closes both owners before
waiting and joins startup cancellation as well as live sessions. Dropping a
startup waiter or the lifecycle owner requests stop without discarding the
independent drain obligation. Controlled registry tests cover stale stops,
cancelled waiters, close-before-join and retried joins. These are lifecycle
regressions, not real-model execution qualification.

## Production producer connection

The existing Torch serving adapter dispatches audio library records to the
installed-audio producer; existing image/HTTP providers keep their own path.
The app manager retains the source-pinned interpreter, dependencies, native
libraries and embedded sidecar. Core's `serve_installed_audio_for_operation`
accepts a deferred producer of that opaque byte owner and the original model
selector. Admission and the shared profile guard precede every preparation await,
so stop/edit/delete cannot pass an unregistered startup. Core resolves the indexed
artifact itself and prepares the original held model allocation. A stop during
preparation closes the original admission immediately and joins already admitted
preparation before refusing child launch.
Finite preparation is registered with the primary runtime-task owner, including
its blocking copy/capture and potentially large recipe/model validation, so cancellation cannot make shutdown skip that work.
Only the source-pinned interpreter member is selected; no executable path,
source ID or qualifier is accepted from RPC. The installed recipe/model policy
still refuses unqualified execution before child launch.

Successful load publication requires both the current serving-load receipt and
the current running audio generation. Status uses the private endpoint, with no
HTTP URL; HTTP-only operations refuse that active profile. Unserve stops the
captured original audio generation. The session worker removes serving rows and
retires only its exact endpoint after child/diagnostic join, before releasing the
profile-operation guard. Old endpoint clones remain closed, cannot block a
confirmed-drained successor, and cannot retire that successor. Lifecycle task
panic follows the same cleanup path; task loss is an explicit error after the
independent child drain, never a successful stop observation. Unresolved worker
loss retains the pending profile guard fail-closed; normal terminal publication
explicitly releases it so retained stop waiters cannot block a successor.

## Explicit local Cohere experiment

`serve_experimental_local_cohere` is an explicit JSON-RPC operation using the
existing `ServeModelRequest`. It selects a separate source-fixed experimental
CPU policy. Ordinary `serve_model` and the ordinary core installed API still
use the closed shipping policy. No environment variable, submitted manifest,
metadata flag or executable path enables the experiment.

A successful experimental load returns `experimental: true`,
`production_available: false` and `experimental_local_cohere`: the actual
selected model/artifact, observed model manifest digest, source recipe and
worker-code digests, kernel Landlock ABI, CPU device and exact profile generation.
`child_drained: false` describes the live child at load completion. These are
observations, not a qualification certificate. Real-model ASR remains locally
untested until the user runs it. Native disposal is unqualified: use profile
stop/unserve, which closes admission and joins the original child tree.

### Linux local workflow

Use Linux x86_64 with a kernel exposing Landlock ABI 6 or newer. The pinned
managed-runtime path accepts ASCII letters/digits and `/._-` only; choose a root
without spaces or non-ASCII characters. CPU execution can need substantial RAM:
original selected bytes, sealed weight snapshots and native tensors coexist.
No real-model memory requirement has been measured here. Linux Mint's
name/version alone does not establish this. The load API performs a read-only
kernel preflight before polling runtime preparation or copying the model. It
refuses unavailable enforcement; do not disable confinement or substitute a
broader filesystem grant. A read-only check before downloading anything is:

```sh
python3 - <<'PY'
import ctypes, platform
if platform.system() != 'Linux' or platform.machine() != 'x86_64':
    raise SystemExit('Requires Linux x86_64')
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
abi = libc.syscall(444, None, 0, 1)  # x86_64 landlock_create_ruleset VERSION
if abi < 6:
    raise SystemExit(f'Landlock ABI >= 6 required; query={abi}, errno={ctypes.get_errno()}')
print(f'Landlock ABI {abi}; runtime/model checks are still required')
PY
```

Build the normal inference-enabled RPC target and run one owner of an explicit
library root. Do not concurrently launch the GUI against that same root.

```sh
cargo build --locked --manifest-path rust/Cargo.toml -p pumas-rpc
ROOT=/absolute/path/to/your/pumas-root
rust/target/debug/pumas-rpc --launcher-root "$ROOT" --host 127.0.0.1 --port 18743
```

In another terminal, use `curl` and `jq` with the same running owner:

```sh
BASE=http://127.0.0.1:18743
rpc() {
  jq -nc --arg method "$1" --argjson params "$2" \
    '{jsonrpc:"2.0",id:1,method:$method,params:$params}' |
    curl --fail-with-body -sS "$BASE/rpc" -H 'Content-Type: application/json' --data-binary @-
}
```

Install through the existing trusted managed-runtime workflow. It downloads
public CPU runtime/dependency artifacts; it does not download the user's model.
Use a fresh library root if `v2.10.0` already identifies a different installed
recipe; do not overwrite an image runtime in place.

```sh
rpc preview_torch_runtime '{"tag":"v2.10.0","build":"cpu","python":"python3.12","adapter":"cohere-asr"}' | tee cohere-preview.json
jq -e '.result.status == "ready"' cohere-preview.json
PREVIEW=$(jq -er '.result.preview.previewId' cohere-preview.json)
rpc install_version "$(jq -nc --arg preview "$PREVIEW" '{app_id:"torch",tag:"v2.10.0",preview_id:$preview}')"
rpc get_installation_progress '{"app_id":"torch","tag":"v2.10.0"}'
```

Repeat the progress query until installation reports terminal success; an
accepted installation request is not completion. Refused previews or failed
installations are blockers, not permission to replace hashes or edit recipes.
After successful installation:

```sh
rpc switch_version '{"app_id":"torch","tag":"v2.10.0"}'
rpc upsert_runtime_profile '{"profile":{"profile_id":"cohere-local","provider":"torch","provider_mode":"torch_serve","management_mode":"managed","name":"Experimental local Cohere CPU","device":{"mode":"cpu"}}}'
```

Import a real local directory (no symlink root or selected members) through the
explicit local selection operation. This copies the supported native files into
the library and records local identity/speech intent without claiming Hugging
Face provenance. No HF credential is needed for the local artifact.

```sh
MODEL_DIR=/absolute/path/to/your/cohere-model
rpc import_local_cohere "$(jq -nc --arg path "$MODEL_DIR" '{local_path:$path,official_name:"Local Cohere Transcribe"}')" | tee cohere-import.json
jq -e '.result.success == true' cohere-import.json
MODEL=$(jq -er '.result.model_id' cohere-import.json)
```

Use the returned indexed model ID, not the source path. The publication must
be library-owned, valid and ready, with an explicit selected artifact. The five
required files are `config.json`, `model.safetensors`,
`preprocessor_config.json`, `tokenizer.json` and `tokenizer_config.json`. Select
every present supported optional member: `added_tokens.json`,
`generation_config.json`, `processor_config.json`, `special_tokens_map.json`.
Repository Python is not selected or executed. Do not hand-edit canonical/index
metadata to fabricate provenance or admission.

With `MODEL` set to that returned model ID:

```sh
rpc serve_experimental_local_cohere "$(jq -nc --arg model "$MODEL" '{request:{model_id:$model,config:{provider:"torch",profile_id:"cohere-local",device_mode:"cpu",keep_loaded:true}}}')" | tee cohere-load.json
jq -e '.result.loaded == true and .result.experimental_local_cohere.experimental == true and .result.experimental_local_cohere.production_available == false' cohere-load.json
```

Proceed only after that check succeeds. This does not start Torch's HTTP image
provider. The existing selected-model operation endpoint consumes the retained
native slot. For a mono 16 kHz, 16-bit PCM WAV, create the closed request from its
actual samples (the script refuses other WAV formats rather than resampling):

```sh
WAV=/absolute/path/to/your/mono-16000hz.wav
python3 - "$MODEL" "$WAV" > cohere-transcribe.json <<'PY'
import base64, json, sys, wave
with wave.open(sys.argv[2], 'rb') as audio:
    if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(), audio.getcomptype()) != (1, 2, 16000, 'NONE'):
        raise SystemExit('Requires mono 16 kHz PCM16 WAV')
    count = audio.getnframes()
    if count == 0 or count > 30 * 16000:
        raise SystemExit('Use a nonempty clip of at most 30 seconds')
    samples = audio.readframes(count)
print(json.dumps({'contract_version':1,'request_id':'local-cohere-1',
    'model':sys.argv[1],'profile':'cohere-local','capability':'audio_transcription',
    'input':{'kind':'audio','encoding':'pcm_s16le','sample_rate_hz':16000,
             'channels':1,'sample_count':count,'data_base64':base64.b64encode(samples).decode()},
    'output':'text','options':{'kind':'audio','language':'en','max_output_tokens':512},
    'stream':False}))
PY
curl --fail-with-body -sS "$BASE/v1/model-operations" \
  -H 'Content-Type: application/json' --data-binary @cohere-transcribe.json | tee cohere-result.json
rpc unserve_model "$(jq -nc --arg model "$MODEL" '{request:{model_id:$model,provider:"torch",profile_id:"cohere-local"}}')" | tee cohere-unload.json
jq -e '.result.unloaded == true' cohere-unload.json
```

A successful `unloaded: true` comes only after the captured generation's child
and diagnostic reader have joined. A cancelled or timed-out stop is not evidence
of drain. Retry the captured generation, so a later replacement cannot be
stopped accidentally:

```sh
GENERATION=$(jq -er '.result.experimental_local_cohere.profile_generation' cohere-load.json)
rpc stop_runtime_profile_if_generation "$(jq -nc --argjson generation "$GENERATION" '{profile_id:"cohere-local",generation:$generation}')"
```

`stopped: true` confirms that stop joined that generation. `false` is not a new
drain receipt; inspect current status rather than targeting a successor.
Orderly application shutdown also joins outstanding owned cleanup. Retain the
root and runtime/model owners until it settles. Preserve the load, operation
and unload responses when reporting a local result. Refusals identify bounded
phases; arbitrary native stderr is deliberately not exposed. No successful
fixture, import, dependency probe or load alone establishes real ASR accuracy.
