# Private audio preparation foundations

These helpers prepare inputs for a later owning audio load path. They do not
admit production audio, authorize a model load, attest an existing runtime slot,
or register public operation/status/cancel routes. The typed HTTP endpoint must
continue reporting audio as unavailable until that path is qualified.

## Selected bytes

`ModelLibrary::prepare_cohere_artifact_use` is crate-private. It accepts an
indexed model ID and selected artifact ID; it does not accept a caller path,
manifest, digest, observation fingerprint, or JSON authority record. Both the
trusted index and held canonical metadata must describe the same ready, valid,
library-owned selection.

The supported variant is an unsharded native Cohere ASR package. Required members
are `config.json`, `model.safetensors`, `preprocessor_config.json`,
`tokenizer.json`, and `tokenizer_config.json`. Supported optional members are
`added_tokens.json`, `generation_config.json`, `processor_config.json`, and
`special_tokens_map.json`; a present optional member must be explicitly selected.
Unknown selections, missing required members, unsupported custom-code bindings
and unsupported variants are refused. Exact known original Cohere `auto_map`
bindings and tokenizer aliases may remain as inert metadata: the loader uses
explicit installed native classes and never executes repository Python. See
[the native qualification contract](cohere-native-qualification.md).

Preparation retains the existing root execution grant before opening selected
files. It copies actual bytes through held source descriptors into a private
read source, retaining copied file and directory handles. The per-member SHA-256
manifest records path, byte count, and digest; a domain-separated manifest digest
also includes model and artifact identity. It is independent of the package
observation fingerprint. Copies are separate files, so later original-package
mutation does not change the prepared read source. The returned owner is not
cloneable or deserializable and retains the root grant until disposal.

Validation observes root identity, private directory identity, the exact copied
member set, copied file identity, length, and hashes before a provider effect.
This blocking file scan must not be used as a synchronous per-operation borrow
on the speech event loop. Cleanup restores permissions on held private copies;
if the scratch pathname is already observed to refer to a different directory,
cleanup is disabled rather than deleting that replacement.

The root grant excludes cooperating Pumas mutations across the whole root. It
does not freeze original bytes against external writers during copying. The
manifest attests the bytes actually copied. Private permissions and revalidation
are not protection from hostile code running as the same user; cleanup's
pathname check is not a hostile-race guarantee. Native Windows handling is
implemented but has not been executed in the Linux qualification environment.

## Audio conversion

`audio_input.normalize_audio` validates the closed declaration of PCM16LE or
normalized finite float32LE, 8–192 kHz, one or two channels, positive frame count,
exact base64/byte length, and at most 30 seconds. It preserves already-normalized
mono 16 kHz PCM16LE bytes. Other supported input is actually decoded, downmixed,
low-pass resampled in bounded blocks, and quantized to mono 16 kHz PCM16LE. It
does not relabel sample rate or channel count. This helper imports no Torch,
loads no models, and grants no artifact or runtime authority.

## Required production bridge

The owning load path must additionally qualify the installed interpreter,
recipe, sidecar and loader code, and the loader's complete consumed read set.
The closed file-copy variant and descriptor checks alone do not prove that an
installed Transformers processor cannot consume another path-valued setting.
The bundled Transformers 4.57.6 recipe does not qualify native Cohere ASR.
The installer now includes the speech operation owner, native loader and audio
helpers; installed selected-byte custody is described in the transport contract.
A compatible dependency recipe and native execution closure remain required.

Before the first load RPC, the owner must reserve serving/load admission and
retain prepared-byte custody under the exact managed child generation. A single
composite cleanup owner must be attached when the child is spawned; replacing
the child's existing cleanup lease with individual model records would lose
custody. Caller loss and unknown load acknowledgements retain that custody.

A ready load must bind runtime instance, slot ID, and load generation to that
retained owner through a private owning channel. Legacy arbitrary-path slots
remain unattested. Later operation borrows validate this in-memory lineage;
successfully validating bytes after an arbitrary past load cannot attest it.
Release requires exact-slot native/device cessation with no outstanding borrows,
or complete exact-generation child-tree drainage. Unknown unload outcomes,
failed native cleanup, and quarantined device work retain custody. HTTP
transport closure by itself establishes none of these release conditions.

Controlled qualification covers ten selected-byte/custody tests and seven PCM
conversion tests without model downloads, Torch installation, or inference.
The full Python tests use the repository's pinned FastAPI/Starlette recipe and
fake native workers. These foundations are a separate local development slice
from the independently usable text/embedding/image typed endpoint.
