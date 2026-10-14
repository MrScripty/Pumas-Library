# Pumas Library 0.8.0 candidate

Status: source checkpoint preparation. Current manifests are `0.8.0-rc.1`.
There is no final v0.8.0 tag, accepted installer set or publication decision in
this change. [Release QA](release-qa-0.8.0.md) owns candidate identity, observed
consumer revisions, evidence and remaining gates.

## Consumer-visible changes

The generic HTTP operation boundary accepts typed text, embeddings, image and
audio requests against an explicitly selected model/profile. Capability
descriptors report what the current serving context can execute. Bounded
Image-to-Text adds named image and image-bearing message inputs, preserving
request identity, image bytes, resource bounds and typed refusal outcomes.
See the [Image-to-Text contract](contracts/image-to-text-v0.8.md).
Production installed Audio-to-Text remains unavailable; qualification-only
constructors and controlled provider replies do not enable production speech.

Local consumers can discover and explicitly attach to a selected ready owner,
retain its lifetime and fence requests against root/generation changes. An
existing catalogue entry does not imply a loaded or executable model. Borrowed
attachment must not terminate another application's owner; a disconnect or
uncertain request result does not authorize inference replay.

Acquisition includes digest-bound local input capabilities and conditional S3
single-object/authored-bundle GGUF imports. Hugging Face browse/cache observations
remain separate from download details resolved after selection. Managed models
keep receipt-bound identities, including the selected GGUF primary/projector
through owned library moves. These contracts do not promise arbitrary
ONNX/Safetensors/Diffusers imports through the public S3 GGUF path.
The current S3 checks use controlled HTTPS fixtures; they do not certify AWS,
non-AWS providers or MinIO endpoint combinations. Those provider-specific claims
retain their existing AQ-S3 acceptance gate.

## Existing libraries and migration

Read [Opening an older library](library-upgrade.md) before opening a shared
library with this candidate. Download-store schema 7 is an explicit one-way
metadata conversion from the supported v4/v5/schema-6 states. Stop all readers,
writers, old applications and developer instances first. Use the desktop's
**Upgrade Library** confirmation or the documented Linux offline RPC command.
The converter creates a private exact backup of `downloads.json`; it does not
copy, scan, hash or rewrite model payloads. Opening the converted library then
performs ordinary startup work.

Do not reopen upgraded metadata with an older binary or automatically restore
the backup after an uncertain publication. Preserve metadata, backups and logs
on failure. Unsupported/corrupt stores are refused. Migration cannot retire a
previously retained failed owner claim: a dead PID is insufficient authority to
delete registry state. Recovery requires the existing custody contract.

Missing download destinations remain represented in their retained error/paused
state without startup creating folders or pretending absent bytes completed.
Interactive download-location reconciliation is separately planned work; this
checkpoint does not turn its proposal into implemented functionality.

## Compatibility and distribution

Native Rust consumers use an immutable Git revision and their locked dependency
closure. Update Pantograph's manifest, lockfile and CI setup pin as one consumer
cohort, then run its actual library/IPC/lifecycle checks. Independent HTTP
consumers must validate the named protocol/schema advertisements and closed
operation variants they use; matching product versions alone is insufficient.
The source-exported Python session helper is a reference, not a supported SDK.
The binding support matrix still accepts no UniFFI/Rustler host package tuple.

The [artifact plan](../scripts/release/artifact-plan.json) requires sixteen
files: five full desktop installers, five no-inference desktop installers,
three no-inference headless archives and three inference headless archives.
Targets are Linux x86_64, Windows x86_64 and macOS ARM64. Headless binaries
include the core; embedders do not need a separate core binary. Choose the
correct inference variant explicitly. Run only one desktop variant per machine
because their product name is shared.

Inference archives bind exact source/schema/runtime bytes and upstream notices.
CPU ONNX Runtime is pinned to 1.24.2; Cargo never downloads it. Current PR Linux
review can prove an extracted archive's finite untrained-graph execution and
cleanup. This is distinct from pretrained Nomic quality, native Windows/macOS
acceptance, vision model/projector execution, production speech and final
distribution acceptance. Follow [Releasing](../RELEASING.md) for native runtime
prerequisites, packaged model checks, notices, security review and publication.

Custom Torch component assembly, additional discovery networks, arbitrary
modality pairs and new language SDKs are later scope. Their absence does not
authorize weakening current capability, ownership or migration refusals.
