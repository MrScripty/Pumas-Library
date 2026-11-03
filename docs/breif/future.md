# Pumas Library — Model Resource Intelligence

## Status

**Theoretical future direction.**

This brief assumes the current Pumas briefs have already been fully implemented and are operating successfully, including:

- intent-level model management and durable desired state;

- local and distributed capability discovery;

- host-level Pumas authority;

- fleet/node coordination and peer artifact transfer;

- shared and local model catalogs;

- MCP/agent integration;

- the unified optional inference gateway;

- runtime/provider discovery;

- distributed model availability and capability resolution.

This document does not authorize implementation. It describes a possible architectural direction after those systems have matured.

---

# Purpose

Once Pumas can reliably discover, acquire, verify, distribute, expose, and maintain models across applications and machines, the central problem changes from:

> **How does Pumas obtain a model?**

to:

> **Given the available models, artifacts, hardware, runtimes, policies, sources, and machines, what model resources should exist, where should they exist, in what representation, and how should Pumas make that state true?**

The proposed direction is to evolve Pumas from a **model-management authority** into a **model-resource intelligence layer**.

Pumas remains responsible for the model-resource substrate:

```text
discovery
selection
acquisition
derivation
verification
compatibility
placement
replication
retention
eviction
provenance
policy
```

It should not become a general workload scheduler.

Execution systems such as Pantograph remain responsible for:

```text
which workload runs
when it runs
where computation executes
how compute is scheduled
how workflows are orchestrated
```

The boundary remains:

```text
Pumas
    "What model resources should exist, and how do I make them available?"

Pantograph
    "Where and when should computation execute?"
```

---

# 1. Capability-oriented model resolution

Pumas should move beyond requiring consumers to identify an exact model.

A current-style request might be:

```text
GetModel {
    model: "Qwen/Qwen..."
    format: GGUF
    quantization: Q4_K_M
}
```

A future request could instead describe the required capability:

```text
ResolveModel {
    capability:
        text.embeddings

    require:
        context_length >= 8192
        commercial_use = true
        max_memory <= 6 GiB

    prefer:
        already_local
        low_activation_cost
        smallest_artifact
        fastest_on_hardware
}
```

The consumer describes what it needs. Pumas selects a suitable model and representation.

## Resolution engine

The resolver should evaluate candidates through a deterministic pipeline:

```text
Model Requirement
      │
      ▼
Candidate Discovery
      │
      ▼
Compatibility + Policy Filtering
      │
      ▼
Hardware Feasibility
      │
      ▼
Availability / Derivation Analysis
      │
      ▼
Cost + Preference Ranking
      │
      ▼
Resolution Plan
```

Evaluation may include:

```text
task compatibility
architecture
format
quantization
context size
license
runtime support
VRAM and RAM requirements
hardware compatibility
local availability
peer availability
upstream availability
conversion paths
trust requirements
activation cost
reacquisition cost
```

The resolver should distinguish:

```text
Hard requirements
    candidate is invalid if unmet

Soft preferences
    candidate remains valid but affects ranking
```

## Hardware feasibility

Pumas should determine whether a representation is realistically usable on known hardware.

Inputs may include:

```text
GPU architecture
VRAM
system RAM
CPU
runtime support
quantization
context length
KV-cache requirements
offloading support
backend restrictions
```

Feasibility should be expressed with uncertainty:

```text
Known compatible
Likely compatible
Unknown
Known incompatible
```

Static estimates may later be improved by observed runtime data such as:

```text
actual VRAM usage
actual RAM usage
load time
tokens per second
embedding throughput
runtime failures
backend-specific compatibility
```

Pumas must preserve the distinction between:

```text
declared metadata
estimated metadata
observed metadata
```

## Explainable decisions

Automatic decisions must be inspectable.

For example:

```text
Selected:
    Model C

Reasons:
    compatible with ONNX Runtime
    artifact already local
    estimated VRAM: 3.2 GiB
    required context supported
    commercial-use policy satisfied

Rejected Model A:
    license not permitted

Rejected Model B:
    estimated VRAM exceeds limit

Model D:
    valid but requires 11 GiB download
```

Possible domain concepts include:

```text
ResolutionPlan
ResolutionDecision
CandidateEvaluation
RejectionReason
PreferenceScore
```

---

# 2. Artifact identity, derivation, and reproducibility

Pumas conversion should evolve from individual conversion operations into a general artifact derivation system.

A model may have multiple usable representations:

```text
                BF16 Safetensors
                 /      |       \
                /       |        \
             GGUF      FP8      NVFP4
              │         │
           Q4_K_M      ONNX
```

Pumas should explicitly distinguish:

```text
Model
    semantic identity

Model Revision
    immutable source version

Artifact
    usable representation

Blob
    immutable physical content
```

For example:

```text
Model
    Qwen/X

Revision
    commit abc123

Artifacts
    BF16 Safetensors
    FP8 Safetensors
    GGUF Q4_K_M
```

## Derivation graph

A model requirement may be satisfied by an existing artifact or by deriving one:

```text
required artifact
      │
      ├── exists → use
      │
      └── missing
           │
           ▼
    derivation path?
      │
      ├── yes → derive
      └── no  → acquire source
```

Example:

```text
Required:
    GGUF Q4_K_M

Available:
    BF16 Safetensors

Plan:
    derive GGUF
    quantize Q4_K_M
    verify
    publish
    satisfy requirement
```

## Reproducible transformations

Derived artifacts should retain enough information to answer:

> Where did this artifact come from?

and:

> Can this artifact be recreated?

A derivation record may include:

```text
parent artifact hash
conversion recipe
converter implementation
converter version
runtime dependencies
parameters
hardware-sensitive behavior
result hash
verification result
```

Conceptually:

```text
Artifact A
    +
Conversion Recipe R
    +
Toolchain Version V
    =
Artifact B
```

Artifact derivation should therefore behave more like a reproducible build system than a collection of ad hoc conversion commands.

---

# 3. Content-addressed artifact storage

As Pumas manages more models, conversions, adapters, tokenizers, projectors, and replicas, directory-oriented storage becomes increasingly inefficient.

Pumas should consider an internal content-addressed store.

```text
Artifact Manifest
      │
      ▼
Required Blobs
      │
      ├── sha256:aaa...
      ├── sha256:bbb...
      └── sha256:ccc...
```

Physical storage:

```text
objects/
    aa/
        aaaa...
    bb/
        bbbb...
```

A model artifact becomes a logical manifest over immutable content.

Benefits include:

```text
deduplication
immutable identity
verification by hash
partial synchronization
efficient peer transfer
shared tokenizers
shared adapters
shared diffusion components
simpler replication
derivation tracking
```

Multiple artifacts referencing the same content should not require duplicate physical storage.

Physical storage layout should remain private to Pumas. Applications should work with logical model and artifact identities rather than filesystem structure.

---

# 4. Placement, replication, and storage lifecycle

Once Pumas operates across a fleet, acquisition naturally evolves into placement management.

A policy may express:

```text
Ensure model X:
    minimum_replicas: 2

Prefer:
    GPU-capable nodes
    nodes in region A

Avoid:
    low-storage nodes
```

Or:

```text
Embedding model:
    present on every developer workstation

Large image model:
    present on at least one LAN node

Rare archive model:
    retain at least one verified replica
```

Pumas reconciles desired placement against observed locations:

```text
Desired Placement
      │
      ▼
Observed Artifact Locations
      │
      ▼
Placement Plan
      │
      ├── transfer
      ├── replicate
      ├── derive locally
      ├── acquire upstream
      └── remove excess replica
```

## Replication

Replication policy may include:

```text
minimum_replicas
preferred_replicas
failure-domain requirements
preferred locations
```

If a node disappears and replica count falls below policy, Pumas can restore the desired state automatically.

## Storage lifecycle

Pumas also needs to determine what may safely be removed.

Inputs may include:

```text
active declarations
placement policy
replica count
last use
artifact size
reacquisition cost
derivation cost
peer availability
upstream availability
uniqueness
disk pressure
user pinning
```

Example:

```text
Artifact A
    42 GiB
    no active requirements
    upstream available
    4 peer replicas
    last used 90 days ago
    → strong eviction candidate

Artifact B
    18 GiB
    sole verified copy
    upstream unavailable
    → retain
```

Pumas should distinguish:

```text
Eviction
    remove a local replica while preserving overall availability

Deletion
    intentionally remove an artifact from managed state
```

## Reacquisition cost

Storage policy should consider how difficult an artifact is to recover.

For example:

```text
Artifact A
    20 GiB
    downloadable in 5 minutes

Artifact B
    12 GiB
    requires 2-hour quantization
```

Artifact A may be the better eviction candidate despite being larger.

---

# 5. Provenance, trust, and policy

As artifacts come from public repositories, shared caches, peers, private stores, imports, and conversion pipelines, provenance becomes central.

Every managed artifact should have an inspectable lineage.

```text
Artifact
   │
   ▼
Source Revision abc123
   │
   ▼
Downloaded from Provider
   │
   ▼
Hash verified
   │
   ▼
Converted using Toolchain X
   │
   ▼
Derived Artifact
   │
   ▼
Result verified
```

Pumas should preserve information such as:

```text
source identity
source provider
immutable revision
hash
signatures
conversion lineage
license
verification history
import origin
trust classification
```

## Trust and license policy

Policy should be a first-class domain rather than scattered resolver logic.

Potential policy areas include:

```text
source policy
license policy
trust policy
storage policy
replication policy
placement policy
conversion policy
network policy
cost policy
```

Examples:

```text
require immutable revision
require SHA256 verification
deny pickle-based weights
require Safetensors
deny unknown provenance
require signed organizational artifacts
allow only approved source organizations
```

License-aware resolution might support:

```text
license:
    commercial_use_required: true
```

or organizational policy:

```text
allowed:
    apache-2.0
    mit

review_required:
    custom

denied:
    non-commercial
```

Pumas should not replace legal review. It should make model selection and governance aware of declared policy.

Policy evaluation should be explicit:

```text
Requirement
    │
    ▼
Candidate Plan
    │
    ▼
Policy Evaluation
    │
    ├── allowed
    ├── denied
    └── approval required
```

---

# 6. Portable model environments

Applications should be able to declare their model-resource requirements in a portable format.

Example:

```yaml
models:

  embeddings:
    capability: text.embeddings

    require:
      context_length: 8192
      commercial_use: true

    prefer:
      max_memory: 6GiB

  vision:
    model: organization/model-x
    revision: abc123
    format: safetensors
```

Then:

```text
pumas ensure pumas.yaml
```

could reconcile the environment.

## Lockfiles

Abstract requirements should optionally resolve into immutable state:

```text
pumas.yaml
    abstract requirements
         │
         ▼
      resolve
         │
         ▼
pumas.lock
    exact models
    revisions
    artifacts
    hashes
    derivation recipes
```

Conceptually:

```text
Cargo.toml → Cargo.lock

Pumas manifest → Pumas lock
```

The same lockfile could be reconciled on:

```text
developer workstations
CI machines
production nodes
fleet deployments
```

The acquisition path may differ:

```text
reuse local blob
peer transfer
download
derive artifact
```

but the resulting logical model state should be equivalent.

---

# 7. Source and catalog ecosystem

Hugging Face should eventually become one source provider among several.

Potential providers include:

```text
Hugging Face
ModelScope
OCI registries
S3-compatible storage
organization artifact servers
local filesystems
Pumas fleet peers
private model registries
```

The intent layer should not need to understand provider-specific mechanics.

Conceptually:

```text
ModelSourceProvider
    discover
    inspect
    resolve_revision
    acquire
    verify
```

Organizations may also maintain catalogs containing:

```text
approved models
private fine-tunes
internal adapters
licensed models
pre-converted artifacts
organization-specific metadata
```

## Shared catalog as knowledge system

The shared discovery catalog could eventually contain more than provider metadata.

Possible observations include:

```text
known formats
known quantizations
runtime compatibility
hardware observations
conversion paths
artifact hashes
licenses
model capabilities
benchmark observations
known issues
```

Every observation should retain provenance.

For example:

```text
Observation:
    model runs on llama.cpp

Source:
    Pumas node 83...

Runtime version:
    ...

Observed:
    ...
```

A central rule should remain:

```text
Catalog knowledge
    "This model is believed to support X."

Local verification
    "This artifact has been verified here."

Runtime observation
    "This artifact actually worked here."
```

Pumas must preserve distinctions such as:

```text
declared
discovered
cached
estimated
observed
verified
derived
trusted
```

Shared knowledge should never silently become local authority.

---

# 8. Model-resource planning and approval

Once resolution, derivation, placement, and policy exist, Pumas should be able to produce a plan before changing state.

Example:

```text
Requirement:
    embedding capability on Node B
```

Plan:

```text
1. Select Model X
2. Resolve ONNX FP16 artifact
3. Source artifact already exists on Node A
4. Transfer 2.8 GiB
5. Verify SHA256
6. Register artifact
7. Mark desired state Ready
```

Another plan might require conversion:

```text
1. Source model exists locally
2. Required GGUF artifact is missing
3. Convert using managed toolchain
4. Temporary storage required: 30 GiB
5. Expected output: 11 GiB
6. Verify and publish
```

Planning enables:

```text
dry-run
cost inspection
approval workflows
auditability
agent reasoning
```

Pumas should support modes such as:

```text
plan only
plan and require approval
automatic execution
```

For example:

```text
pumas ensure environment.yaml --plan
```

might report:

```text
Downloads:
    48 GiB

Transfers:
    31 GiB

Conversions:
    2

Additional storage:
    54 GiB

Evictions:
    none
```

---

# 9. Deterministic intelligence and domain exposure

Even as Pumas becomes more intelligent, correctness should not depend on an LLM.

Model selection, feasibility analysis, policy evaluation, placement, lifecycle, and reconciliation should remain deterministic systems over typed state.

Agents may request decisions and inspect explanations:

```text
Agent:
    "Find an embedding model under 6 GiB."

Pumas:
    deterministic resolver executes
```

rather than requiring the agent to manually:

```text
search catalogs
guess compatibility
choose artifacts
orchestrate downloads
manage conversions
```

The same domain model should remain available through all supported transports:

```text
Pumas Domain
    │
    ├── Rust
    ├── RPC
    ├── bindings
    ├── MCP
    ├── fleet protocol
    └── management adapters
```

Potential operations include:

```text
resolve_model
explain_resolution

plan_requirement
apply_plan

inspect_artifact_lineage
inspect_feasibility

list_artifact_locations
inspect_replication

inspect_storage_pressure
plan_eviction

verify_artifact
```

Transport-specific layers should remain projections of the same underlying semantics.

---

# 10. Boundary with workload scheduling

This architecture gives Pumas increasingly rich information about nodes and model availability.

For example:

```text
Node A
    has Model X
    24 GiB VRAM
    model loaded
```

Pumas may expose that fact.

It should not necessarily decide:

```text
Run request 1837 on Node A at 13:42
```

That belongs to systems such as Pantograph, Kubernetes, inference schedulers, or application-specific orchestration.

Pumas provides:

```text
resource facts
model capability
artifact availability
hardware compatibility
placement state
activation cost
```

The scheduler decides how to use them.

A mature relationship with Pantograph could look like:

```text
                    APPLICATION
                         │
                         ▼
                    PANTOGRAPH
             workload / execution intent
                         │
               model requirement
                         │
                         ▼
                       PUMAS
              model resource authority
                         │
       ┌─────────────────┼─────────────────┐
       ▼                 ▼                 ▼
   Artifact Store     Model Fleet        Sources
```

Pantograph may ask:

```text
I require capability X for workload Y.
```

Pumas can answer:

```text
Node A:
    ready

Node B:
    compatible after artifact transfer

Node C:
    incompatible
```

Pantograph decides where execution occurs.

Pumas makes the required model-resource state true.

---

# 11. Target architecture

A mature control path could resemble:

```text
                   Consumer Requirement
                           │
                           ▼
                       Pumas API
                           │
                           ▼
                    Requirement Engine
                           │
         ┌─────────────────┼──────────────────┐
         ▼                 ▼                  ▼
    Model Resolver     Policy Engine      Planner
         │                 │                  │
         └─────────────────┼──────────────────┘
                           ▼
                       Reconciler
                           │
             ┌─────────────┼─────────────┐
             ▼             ▼             ▼
         Acquisition    Derivation    Placement
             │             │             │
             └─────────────┼─────────────┘
                           ▼
                     Artifact State
```

The artifact layer could resemble:

```text
                       Model Requirement
                              │
                              ▼
                        Model Resolver
                              │
                              ▼
                        Resolution Plan
                              │
             ┌────────────────┼────────────────┐
             ▼                ▼                ▼
         Existing          Derivable        Acquire
         Artifact          Artifact          Source
             │                │                │
             └────────────────┼────────────────┘
                              ▼
                       Artifact Manifest
                              │
                              ▼
                  Content-Addressed Store
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
           Local           Peer Node      Object Store
```

---

# 12. Development sequence

A reasonable development order after the current briefs mature would be:

## Phase 1 — Resolution intelligence

Build:

```text
capability-oriented model requirements
model resolver
hard constraints
soft preferences
hardware feasibility
resolution explanations
```

## Phase 2 — Artifact derivation and lineage

Build:

```text
artifact identity
derivation graph
conversion recipes
reproducible transformations
provenance
```

## Phase 3 — Content-addressed storage

Introduce:

```text
immutable blobs
artifact manifests
deduplication
hash-based identity
partial peer transfer
```

Migration should preserve existing libraries and keep physical storage layout private.

## Phase 4 — Placement and lifecycle

Build:

```text
replication policy
placement policy
storage pressure handling
reacquisition cost
safe eviction
distributed replica reconciliation
```

## Phase 5 — Trust and governance

Add:

```text
source policy
signatures
license constraints
trust policy
approval workflows
```

## Phase 6 — Reproducible environments

Add:

```text
Pumas manifests
lockfiles
environment reconciliation
CI integration
fleet deployment integration
```

---

# Core design principles

### Pumas manages model resources, not workloads

Richer model intelligence must not turn Pumas into a general execution scheduler.

### Consumers describe requirements

Pumas determines how to satisfy them and reconciles toward desired state.

### Models, artifacts, and blobs are different concepts

A semantic model may have many representations composed from shared physical content.

### Prefer immutable identity

Exact revisions, hashes, and reproducible derivations should be preferred over mutable references.

### Knowledge requires provenance

Cached, peer-provided, or community-provided information must remain distinguishable from locally verified truth.

### Intelligence remains deterministic

The resolver may become sophisticated without becoming conversational or probabilistic.

### Decisions should be explainable

Users and applications should be able to inspect why Pumas selected, converted, transferred, replicated, retained, or evicted an artifact.

### Local nodes remain authoritative for local reality

Fleet and catalog systems aggregate state but do not invent local artifact or runtime state.

---

# Target end state

Pumas would evolve through approximately three architectural stages:

```text
Stage 1 — Model Library

discover
download
import
validate
repair
```

```text
Stage 2 — Model Authority

intent
desired state
discovery
distributed acquisition
capability resolution
fleet coordination
```

```text
Stage 3 — Model Resource Intelligence

select
derive
verify
place
replicate
optimize
retain
evict
govern
```

A consumer could eventually express:

```text
I require a commercially usable embedding capability
with at least 8K context,
usable within 6 GiB VRAM,
available on these nodes.
```

Pumas determines:

```text
which model satisfies the requirement
which artifact representation is appropriate
whether that artifact already exists
whether it should be downloaded, transferred, or derived
where replicas should be placed
whether the available hardware can use it
whether policy permits it
how the artifact was produced
when redundant copies may safely be removed
```

The consumer should not need to understand storage layout, transfer mechanisms, conversion tooling, source providers, or fleet mechanics.

The long-term role of Pumas becomes:

> **Pumas is the authority responsible for turning model-resource requirements into verified, reproducible, appropriately placed model state.**
