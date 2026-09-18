Generic Model Fetching Backend

Purpose

Pumas should separate model source resolution from artifact transfer so that Hugging Face is no longer a special-purpose download path.

The goal is to provide a generic fetching backend capable of obtaining model artifacts from:

- Hugging Face
- S3
- S3-compatible object stores
- arbitrary HTTP/HTTPS sources
- Pumas cluster peers
- future registries and model repositories

Hugging Face should become one resolver/provider layered on top of generic transport and artifact-acquisition infrastructure rather than owning the download implementation.

This should reduce source-specific implementation, allow Pumas to support many storage providers without bespoke integrations, and establish infrastructure that can later support chunk-addressed artifact storage and deduplicated transfers.

---

Problem

Pumas currently has significant Hugging Face-specific functionality around:

- model search
- metadata lookup
- authentication
- repository inspection
- downloading
- progress tracking
- pause/resume
- persistence
- recovery

Several of these responsibilities are not inherently Hugging Face-specific.

At the transport level, model acquisition is generally a combination of:

1. determining which artifact is required;
2. locating the bytes;
3. obtaining authorization;
4. transferring ranges or objects;
5. verifying the resulting artifact;
6. publishing the artifact into the Pumas model library.

Hugging Face itself ultimately resolves repository artifacts into downloadable storage objects. Modern Hugging Face repositories may additionally use Xet content-addressed storage and reconstruction before the underlying ranges are retrieved.

Pumas should therefore avoid treating "Hugging Face download" as synonymous with "model download."

---

Design Principle

Separate acquisition into four conceptual layers:

Model Source / Resolver
        ↓
Artifact Manifest
        ↓
Generic Transfer Backend
        ↓
Pumas Artifact / Model Storage

More concretely:

                     Pumas
                       │
                Model Requirement
                       │
                       ▼
                Source Resolver
                       │
      ┌────────────────┼─────────────────┐
      │                │                 │
 Hugging Face          S3              HTTP
      │                │                 │
      └────────────────┼─────────────────┘
                       │
                       ▼
                Artifact Manifest
                       │
                       ▼
                Transfer Engine
          HTTP / ranges / S3 / peers
                       │
                       ▼
               Verification Layer
                       │
                       ▼
                  Model Library

A resolver understands the source.

The transfer engine understands bytes.

The model library understands models.

These responsibilities should remain distinct.

---

Goals

The backend should:

- allow Hugging Face artifacts to be acquired without depending on the Hugging Face CLI;
- avoid requiring the official Hugging Face client library as a core dependency;
- provide first-class support for S3 and S3-compatible storage;
- support generic HTTP sources through the same transfer engine;
- preserve Pumas' existing download lifecycle features;
- allow additional sources to be implemented primarily as resolvers rather than complete download systems;
- support ranged and resumable transfers;
- allow artifacts to be supplied by Pumas cluster peers;
- establish an architecture compatible with future content-addressed and chunk-deduplicated storage;
- preserve normal GGUF, safetensors, and model-directory representations for consumers.

---

Non-Goals

The initial implementation does not need to:

- replace the existing Pumas model library with a CAS;
- require Xet internally for all models;
- build a Pumas virtual filesystem;
- guarantee zero-copy model materialization;
- implement every model registry individually;
- make all model sources searchable;
- expose storage-provider details to normal Pumas consumers.

The first objective is generic acquisition, not a complete redesign of local model storage.

---

Source Resolver

A source resolver converts a model or artifact reference into a source-independent acquisition description.

Example:

hf://Qwen/Qwen3-8B-GGUF@revision/model.gguf

may resolve into:

ArtifactManifest {
    identity,
    revision,
    size,
    expected_hash,
    files,
    transport_plan,
    provenance,
}

Likewise:

s3://my-model-bucket/qwen/model.gguf

could produce the same logical manifest shape.

The resolver should answer:

- What artifact is being requested?
- What immutable version/revision is being used?
- Which files comprise it?
- How large are they?
- How are they retrieved?
- What authentication is required?
- What hashes or other verification information are available?
- What provenance information should Pumas retain?

The resolver should not itself own the generic transfer lifecycle.

---

Proposed Resolver Types

Hugging Face Resolver

Responsible for Hugging Face-specific semantics:

- repository IDs;
- revisions/commits;
- model filenames;
- gated/private repository authentication;
- repository metadata;
- model metadata;
- file enumeration;
- Xet/LFS resolution;
- generation of an artifact transfer plan.

The Hugging Face resolver should progressively become thinner.

Conceptually:

HuggingFaceResolver
    │
    ├── search
    ├── repository metadata
    ├── auth
    ├── revision resolution
    └── artifact resolution
             │
             ▼
       ArtifactManifest

It should not need independent implementations of:

- retries;
- range transfer;
- pause/resume;
- persistence;
- progress;
- partial-file recovery;
- checksum verification.

Those should belong to shared infrastructure.

---

S3 Resolver

Provide native support for S3 object references:

s3://bucket/key

Configuration should allow:

- endpoint;
- bucket;
- region;
- credentials;
- anonymous/public access;
- path-style versus virtual-host addressing where required;
- signed URLs where appropriate.

The same implementation should support S3-compatible providers when protocol-compatible, including examples such as:

- AWS S3;
- Cloudflare R2;
- MinIO;
- Backblaze B2's S3 API;
- Wasabi;
- DigitalOcean Spaces;
- private/self-hosted object stores.

Provider-specific behavior should be introduced only where required rather than through separate downloader implementations.

---

HTTP Resolver

Support direct:

https://...

artifact references.

Useful for:

- release assets;
- static model hosting;
- presigned object URLs;
- internal servers;
- mirrors;
- source resolvers that ultimately produce ordinary HTTPS ranges.

HTTP should be a first-class transport rather than merely an implementation detail of Hugging Face.

---

Cluster Resolver

A future Pumas cluster resolver can resolve artifacts or artifact chunks available from other nodes.

Example:

Model requirement
      ↓
Artifact manifest
      ↓
Local inventory
      ↓
Cluster inventory
      ↓
Internet sources

This should allow the same artifact identity to be satisfied from multiple locations.

---

Transport Layer

The transfer layer should be source-agnostic.

Responsibilities should include:

- HTTP GET;
- byte-range requests;
- S3 object retrieval;
- concurrent range transfer;
- retry/backoff;
- pause;
- resume;
- cancellation;
- progress reporting;
- bandwidth accounting;
- temporary acquisition state;
- persistence;
- crash recovery;
- integrity verification;
- source failover where supported.

The transfer layer should not need to know whether the bytes originated from Hugging Face, R2, MinIO, or a cluster peer.

---

Artifact Manifest

Introduce a source-independent artifact description.

Illustrative structure:

ArtifactManifest {
    artifact_id
    source_identity
    revision
    files[]
    provenance
    verification
    acquisition_plan
}

A file might contain:

ArtifactFile {
    logical_path
    size
    hash
    representation
    sources[]
}

Sources could include:

HttpRangeSource
S3ObjectSource
XetReconstructionSource
ClusterSource
LocalSource

The manifest should describe what must exist, while the transfer system determines how best to obtain it.

---

Multiple Sources for the Same Artifact

The design should permit an artifact or chunk to have multiple candidate locations:

Artifact X
├── local cache
├── cluster node A
├── S3 mirror
└── Hugging Face

Pumas should be able to choose based on availability and policy rather than artifact identity being tied to a single provider.

This is particularly important for:

- offline operation;
- local mirrors;
- enterprise deployment;
- clusters;
- resilient downloads;
- future peer-to-peer distribution.

---

Xet Integration

Xet should be treated as an optional artifact reconstruction mechanism, not as synonymous with Hugging Face.

For an Xet-backed Hugging Face file:

HF repository reference
        ↓
Hugging Face resolver
        ↓
Xet file identity
        ↓
Xet reconstruction plan
        ↓
required ranges/chunks
        ↓
generic transfer engine
        ↓
reconstructed artifact

Pumas should investigate integrating the existing Rust Xet implementation rather than immediately reimplementing the protocol.

The architectural requirement is that Xet output feeds the same acquisition system used by other sources.

---

Future Chunk CAS

The generic backend should be designed so a future chunk-level content-addressed store can be added without changing the public model-access contract.

Potential flow:

Remote artifact
      ↓
content-defined chunks
      ↓
Pumas CAS
      ↓
artifact manifest
      ↓
runtime model representation

This could allow Pumas to determine which portions of a requested model are already present locally.

For example:

Requested model:

A B C D E F G H

Already present:

A B   D E   G

Need:

    C     F   H

Only missing chunks need to be acquired.

This could reduce transfer volume between:

- related checkpoints;
- model revisions;
- artifacts containing identical regions;
- cluster nodes with overlapping content.

Exact savings depend on byte-level identity. A fine-tune that changes most weights may have little deduplication, whereas checkpoints or artifacts with unchanged regions may have substantial reuse.

---

Cluster Implications

Chunk-addressed acquisition becomes particularly valuable in a Pumas cluster.

A node requesting an artifact could satisfy it from several sources:

Node requesting model
        │
        ▼
Missing chunk inventory
        │
   ┌────┼───────────────┐
   │    │               │
local  peers          internet
CAS    CAS          HF / S3 / HTTP
   │    │               │
   └────┴───────┬───────┘
                ▼
           complete CAS
                │
                ▼
          runtime artifact

This allows model distribution to become content-based rather than file-transfer-based.

The cluster therefore does not necessarily need to transfer a complete 20 GB model if only 3 GB of content is missing.

---

Runtime Representation

Generic artifact storage must not compromise compatibility with existing inference software.

Pumas consumers should continue receiving ordinary paths such as:

/path/model.gguf

or:

/path/model/
    config.json
    tokenizer.json
    model-00001.safetensors
    model-00002.safetensors

Applications such as llama.cpp, Transformers, and other inference systems should not need Pumas-specific filesystem APIs.

The artifact backend is an internal storage/acquisition concern.

---

Zero-Copy and Filesystem Optimization

Pumas may later use filesystem capabilities to expose CAS-backed data as ordinary files without duplicating physical storage.

Possible strategies:

range cloning / reflinks
whole-file CoW clone
normal materialization

Support varies substantially by filesystem, so this must remain an optimization.

Pumas should guarantee:

«A usable normal model representation is available regardless of filesystem capabilities.»

It should not guarantee:

«CAS content can always be exposed without copying.»

A future filesystem-capability layer may select between:

RangeClone
WholeFileClone
Materialize

depending on the host.

---

CAS Retention Policy

On systems where artifact data must be physically copied into a runtime model file, retaining both may double disk usage.

Pumas should eventually support policies such as:

Artifact cache:
    persistent

Artifact cache:
    temporary

Artifact cache:
    cluster-seed

Artifact cache:
    bounded/LRU

For ordinary local systems, chunks may be discarded after successful verified materialization.

For cluster/cache nodes, retaining them may be desirable.

---

Authentication

Authentication should be resolver/source-specific but passed into generic transport through scoped credentials.

Examples:

Hugging Face token
AWS credentials
S3 access key
presigned URL
cluster identity
anonymous access

The transfer engine should not need provider-specific authentication logic where a resolver can convert authorization into a generic request or signed URL.

Credentials must not become part of persistent artifact identity.

---

Artifact Identity

Artifact identity should not depend solely on its source URL.

Prefer immutable characteristics such as:

source namespace
repository/object identity
revision
content hash
artifact/file identity

This allows:

HF artifact
S3 mirror
cluster copy

to potentially resolve to the same underlying content.

That becomes important for deduplication and source failover.

---

Provenance

Even when transport is generic, Pumas should preserve origin information.

Example:

Artifact provenance:
    discovered_from: Hugging Face
    repo: Qwen/Qwen3
    revision: abc123
    original_file: model.gguf
    retrieved_from: cluster-node-4
    verified_hash: ...

Discovery source and transfer source should therefore be distinct.

A model may have originated from Hugging Face while the actual bytes were retrieved from a Pumas peer.

---

Migration from Current Hugging Face Client

Migration should be incremental.

The existing Hugging Face implementation already contains useful functionality that should not be discarded.

A likely progression is:

Current
HuggingFaceClient
├── search
├── metadata
├── auth
├── download
├── resume
├── persistence
└── verification

Move toward:

HuggingFaceResolver
├── search
├── metadata
├── auth
└── artifact resolution

ArtifactTransfer
├── HTTP
├── S3
├── ranges
├── pause/resume
├── retry
├── persistence
└── progress

ArtifactVerifier
├── size
├── hashes
└── revision/provenance

Existing behavior should be extracted into generic components where possible instead of rewritten solely to satisfy the new abstraction.

---

Suggested Initial Implementation Phases

Phase 1 — Generic transfer abstraction

Extract current download lifecycle functionality behind a transport-independent interface.

Support:

HTTP
HTTP ranges
resume
progress
verification

Existing Hugging Face behavior should continue functioning through this layer.

---

Phase 2 — S3 backend

Implement S3-compatible object fetching.

Validate against at least:

AWS S3
one non-AWS compatible service
local MinIO

S3 should use the same persistence, progress, and verification systems as HTTP/Hugging Face.

---

Phase 3 — Resolver abstraction

Introduce source resolvers and artifact manifests.

Convert Hugging Face to produce generic artifact descriptions rather than directly owning transfer execution.

---

Phase 4 — Xet integration

Investigate integrating the Rust Xet implementation.

Allow Xet reconstruction plans to be satisfied through the generic transfer engine where practical.

Avoid coupling Xet directly to the model-library layer.

---

Phase 5 — Multi-source acquisition

Allow one artifact to expose several equivalent sources.

Example:

local
cluster
S3 mirror
Hugging Face

Implement source selection/failover policy.

---

Phase 6 — Optional chunk CAS

Add content-addressed local storage and chunk manifests.

Initially use it for acquisition/cache purposes while retaining normal runtime model files.

Do not make CAS-backed virtual files a prerequisite.

---

Phase 7 — Cluster content distribution

Expose content/chunk availability through Pumas discovery and cluster protocols.

Permit peers to satisfy artifact acquisition.

---

Design Constraints

The implementation should preserve the following constraints:

1. Normal files remain the runtime compatibility boundary.

2. A model's identity must not depend on a single provider.

3. Transport should not contain model semantics.

4. Resolvers should not duplicate generic download lifecycle code.

5. Hugging Face support must not require the Hugging Face CLI.

6. S3-compatible providers should work through configuration whenever possible rather than bespoke integrations.

7. Xet should remain optional infrastructure, not a requirement for Pumas operation.

8. Chunk CAS support must degrade cleanly on filesystems without reflink/range-clone capabilities.

9. Existing Pumas recovery, provenance, verification, and progress behavior should be retained or generalized rather than lost.

10. The architecture must support future cluster and peer sources without another downloader redesign.

---

Desired End State

The eventual acquisition flow should resemble:

Application
    │
    │ "Get model X"
    ▼
Pumas Intent / Model Resolution
    │
    ▼
Artifact identity
    │
    ▼
Available sources
├── already local
├── cluster peers
├── S3
├── Hugging Face
└── HTTP
    │
    ▼
Determine missing content
    │
    ▼
Generic transfer backend
    │
    ▼
Verification
    │
    ▼
Artifact/cache storage
    │
    ▼
Normal runtime representation
    │
    ▼
GGUF / safetensors / model directory

The important architectural outcome is that Pumas becomes responsible for obtaining model content rather than downloading models from specific websites.

Hugging Face then becomes one discovery and resolution provider among several, S3 becomes a broadly reusable storage backend, and a future Xet/chunk-CAS layer can provide deduplication and cluster-efficient content acquisition without changing how applications consume models.