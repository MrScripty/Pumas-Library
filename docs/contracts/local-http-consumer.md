# Callable CLI consumer workflow

The repository does not publish a supported Python SDK. Its
[binding matrix](../../bindings/support-matrix.json) declares
`no_supported_host_bindings`; experimental generated Python/UniFFI artifacts are
not a supported host integration. This workflow uses the existing supported CLI
and HTTP surface. It neither promotes those bindings nor modifies external
consumer repositories.

The executable and callable reference is
[`scripts/consumers/local_http_session.py`](../../scripts/consumers/local_http_session.py).
It requires Linux, Python 3.11 or later, an existing selected library root, and a
known SHA256 for the exact qualified `pumas-rpc` binary. Local execution evidence
uses CPython 3.12 and the inference-disabled debug producer pinned at
`7b308de53b3ec2a892c32098272deca3979b6256`; it does not qualify inference-enabled
bundles or external consumer integrations. The callable is a CLI integration
reference, not an installable Python SDK or another ownership protocol.

## Distribution support

The accepted [release/host decision](../plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/reports/release-and-host-contract-decision.md)
selects immutable Rust source consumers and removes unsupported host-binding
publication. The [v0.8 distribution plan](../plans/headless-inference-distribution-v0.8.md)
adds headless RPC candidates and consumes the shared discovery/build contracts;
it does not admit a Python SDK. Existing native generator scripts and their older
experimental documentation are inventory, not an accepted support tuple.

This module therefore stays a Pumas-owned, source-exported CLI example under
`scripts/consumers`; external consumers retain their existing HTTP adapters.
Its immutable source pin and exact producer binary hash form separate inputs.
It is absent from the existing flat headless archive inventory, which remains
packaging-owned. Including an example in an archive or promoting an installable
client/native host tuple needs its own accepted distribution contract. No new
package, package version, generated binding, or protocol is introduced here.

## Version and capability agreement

The example accepts the intersection of the producer's named
`pumas.local-http` versions with its implemented set `{1}`. It independently
requires the exact advertisement, admission-fence and owner-retention schema
versions named below. A producer advertising `[2, 1]` remains compatible through
version 1; `[2]`, a missing contract or a different required schema version is
refused before acquiring a retention child or sending RPC. A successful explicit
bootstrap is still subject to this check and owned cleanup on refusal.

Named advertisements are bounded to 32 entries and 32 protocol versions per
entry, with unique bounded names, unique positive u32 versions and exact JSON
integer types. Duplicate names cannot silently select a later schema. Valid
additive names are allowed; they grant no implemented operation. Product SemVer,
compiled features and optional build provenance never substitute for contract
agreement. Rust CLI authentication retains its own canonical IPC negotiation.

The three required schemas establish lifecycle/fencing support, not model or
runtime readiness. For inference, the consumer's existing adapter must consult
the selected owner's fenced `GET /v1/capabilities` and match its declared
available operations, input/output formats and semantic tasks as described in
the [RPC operation contract](../../rust/crates/pumas-rpc/README.md#selected-model-modality-first-operations).
External adapters must qualify their adoption of this contract separately.

```sh
python3 scripts/consumers/local_http_session.py /absolute/pumas-rpc EXISTING_ROOT \
  --binary-sha256 EXACT_QUALIFIED_BINARY_SHA256
```

That default authenticates and borrows an already running owner. It never starts
one when selection fails. Add `--allow-start` only when this consumer is explicitly
authorized to bootstrap. The flag delegates reservation/start to
`--attach-or-start-local-http`, which uses `LocalStartAuthority` inside Rust. An
existing compatible owner returns `borrowed`; the short-lived selection child
exits without stopping it. An admitted new owner returns `owned` and stays in the
actual supervised child process. A missing HTTP service, claiming/incompatible
owner or dead retained row fails without fallback, retry or historical takeover.

For a Python host, place the reference directory on its import path. Descriptor
decoding also needs its adjacent `contracts/capability-descriptor.schema.json`:

```python
from local_http_session import local_http_session

async with local_http_session(
    "/absolute/pumas-rpc",
    existing_library_root,
    binary_sha256=qualified_binary_sha256,
    allow_start=False,  # Explicit True delegates fresh-start admission to Rust.
    environment=consumer_environment,
) as session:
    description = session.description
    reply = await session.rpc("get_models")
    # reply["result"]["models"] is the existing RPC model catalog object.
```

`environment` defaults to the host environment. If supplied, it is the complete
child environment. Use the same persisted `PUMAS_REGISTRY_DB_PATH` and existing
root for owner, observer, retention helper and subsequent contexts. This reference
does not change registry settings, create a registry alternative, or infer history
from an empty database. The root is required to exist before any child starts.
Pinning a local binary does not claim a borrowed owner's artifact cohort matches
it; authenticated build descriptors remain available for the host's additional
distribution/model/feature qualification.

## Pinned descriptor decoding and generic selection

The copied `CapabilityDescriptor` schema is pinned to producer source
`926710fa11067c63cae885180db43be5fd7f31f9`, tree
`b09e274bd8df3f6d2ca98a37c5fbc94d1ea9f290`, containing Image-to-Text ancestor
`a0d5dc476e89b08e7a8b4bde63a27354e26cb145`. These pins identify the schema and
fixture contract; they do not qualify execution of that producer by this example.
The adjacent copied schema is exactly 3,537 bytes with SHA256
`eb25783123d38de5ff6aedf4cfa69de8f4f5f5c60799768b1b90da258b85f922`.
The decoder rechecks that hash before use.

`decode_capability_descriptor` accepts bounded JSON bytes, preserves additional
properties permitted by this exact schema, and validates every required field,
closed enum, Boolean, availability branch and finite numeric bound. Empty and
repeated format arrays remain schema-valid; they grant no extra operation.
It requires explicit contract version 1, existing lifecycle advertisements and
the compiled `pumas.model-operations.image-to-text@1` marker in the supplied
build observation. These checks do not authenticate that observation themselves.
Use the actual held session's description after owner/cohort qualification;
the local observer binary hash alone does not identify a borrowed executable.
The marker advertises compiled structure and grants no runtime availability.

For descriptor bytes already obtained from a separately qualified envelope:

```python
from local_http_session import decode_capability_descriptor, select_capability

build_info = session.description["build_info"]
descriptor = decode_capability_descriptor(
    descriptor_bytes, build_info=build_info, contract_version=1,
)
selected = select_capability(
    [descriptor], build_info=build_info, contract_version=1,
    input_modality="image", output_modality="text", stream=False,
)
```

The selector matches concrete formats to their declared representation families:
Text includes `text`, `text_batch` and `messages_text`; Image includes
`png_base64`, `jpeg_base64` and `messages_image`; Audio includes the two declared
PCM encodings. Output families are Text, Image, Embeddings and Labels. Message
formats retain their message/part restrictions. A family match means a declared
representation is available to choose, not that every payload shape is valid.
The selector does not construct, convert, submit or retry any operation.

Selection uses the descriptor's formats, availability, streaming flag and optional
explicit `semantic_task`; it never selects by model ID or capability name.
Multiple available matches require a semantic task. A missing representation and
a declared unavailable adapter remain distinct refusals. Options never select a
task. The returned observation is detached from caller data. A 1 MiB/32-entry
host limit and refusals of duplicate capability observations, duplicate options
or reversed bounds are explicit selector policies beyond JSON Schema's shape
constraints. They do not revise the producer schema.

The two committed Image-to-Text descriptor fixture values were serialized by
the unchanged producer DTOs. The `available` state is
hypothetical; neither fixture was dispatched. Controlled mutated descriptors and
advertisements in tests are decoder/selection evidence only. The preserved local
`7b308de5` binary has no Image-to-Text marker or inference routes and cannot
qualify these functions against a live capability catalog.

Full `CapabilitiesResponse`, model-list and error schema bytes are still required
before adding their HTTP envelope decoders. This example does not guess missing
constraints, the complete error enum, model/profile identity rules or unknown-field
policy. OperationResponse
has no model field; its absence must not become a fabricated response identity
check. No new model-list transport, operation result decoder, image codec or
inference dispatch is implemented in this descriptor slice. Complete producer
composition, live catalog and external consumer acceptance remain separate gates.

## Exact external integration contract

1. Select/authenticate via the pinned binary's `--describe-local-http`, or opt in
   to `--attach-or-start-local-http`. Require its existing
   `PUMAS_LOCAL_ACCESS=` acknowledgment if bootstrapping. The acknowledgment is
   observational JSON; the opaque start authority remains inside its live Rust
   process. No description, PID or URL reconstructs it.
2. Agree on `pumas.local-http@1`; require exact `pumas.http-advertisement@1`, `pumas.http-admission-fence@1` and
   `pumas.http-owner-retention@1` advertisements. The reference checks the existing
   selected root, numeric loopback endpoint and generation-header syntax.
3. Run `--retain-local-http-owner` against that same root/registry. Wait for its
   `PUMAS_LOCAL_RETENTION=` retained acknowledgment and compare both generations to
   the authenticated selection. A race selecting a different owner fails closed.
   Keep/supervise the actual process; a saved JSON line holds nothing.
4. Each operation uses that exact endpoint and both
   `Pumas-Instance-Generation` / `Pumas-Service-Generation` headers. No request is
   admitted by this reference after known revocation/helper loss. Server fencing
   still decides races with shutdown or a new listener at the same URL. There is
   no automatic reconnection, retry, new start or uncertain-effect replay.
5. Context exit releases/reaps its retention helper. It signals an owner only
   through a child this context actually started and supervises. A borrowed
   owner's process remains untouched. Creation, observation and cleanup tasks
   are retained until their own result is observed even when the caller cancels;
   repeated context cancellation does not abandon child cleanup. CLI output is
   continuously drained with bounded lines rather than left in a blocked pipe.
   Malformed/oversized output stops acknowledgment parsing, but bounded chunk
   drainage continues; the first reader failure is reported after reaping even
   if the child exits zero.

The `rpc` example sends a single existing JSON-RPC request to `/rpc`, validates
its returned ID/envelope and never retries. It limits JSON requests/responses to
1 MiB, headers to 16 KiB, and supports the qualified producer's HTTP/1.1
Content-Length JSON response framing. Chunked/streamed OpenAI gateway responses
are outside this small reference. External HTTP clients should
keep their own existing gateway/response/cancellation code and adopt the same
selection, actual retention handle and paired generation fences. This document
does not qualify external consumer integration.

## Cancellation, shutdown and uncertainty

`rpc` cancellation closes that consumer socket. Timeout, disconnect or failure
after send can leave an unknown request outcome; it proves no remote effect or
child stopped. An operation admitted before shutdown may still finish under
Pumas's existing custody. Model readiness, actual model references and model
operation custody remain separate qualifications.

Passive retention does not promise availability after owner shutdown. Operator
shutdown can revoke the helper while the context is open. Subsequent requests
fail; opening a fresh context explicitly authenticates any successor. An old
context never switches generations or reconnects at the old URL. Context exit
after observed revocation reaps the helper without stopping a successor.

Owned context exit releases the helper first and sends SIGTERM to its own known
owner child, allowing the existing HTTP/core drain to finish. Every child is
observed/reaped. If a child exceeds the ten-second graceful wait, this reference
kills only that child and waits up to ten further seconds, then reports cleanup
as incomplete/unconfirmed; it never deletes a registry row or treats forced exit
as a successful owner drain. A crashed helper/owner also reports cleanup failure.
Cancelled or failed startup may retain unresolved ownership under the existing
start-authority contract. Host process crash/exec is not guarded cleanup or a
transferable lease, and a free native lock/dead PID never grants recovery rights.

## Native validation

Run the owned reference's opt-in tests with an exact locally qualified producer:

```sh
cd scripts/consumers
PUMAS_CONSUMER_TEST_BINARY=/absolute/pumas-rpc \
PUMAS_CONSUMER_EVIDENCE=/absolute/evidence-outside-git \
  python3 -m unittest -v test_local_http_session
```

The suite uses actual local producer/helper processes and disposable existing
roots with sentinels: owned start and authenticated borrowing, retained fenced
request completion, owned/borrowed context cancellation, operator revocation,
same-URL restart/old-fence refusal, and observed SIGKILL/dead-row refusal. A
response-observation gate for request cancellation and a creator-return gate
for startup cancellation are explicitly controlled client fixtures around actual
processes/transports, not inference or remote-effect cessation evidence. Without
an explicitly selected native binary, these process tests skip and qualify
nothing. The tests perform no model downloads or inference. External consumer
integration, inference-enabled bundles, model availability and additional platforms
require separate qualification before release.
