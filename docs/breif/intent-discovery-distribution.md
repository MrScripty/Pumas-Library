# Pumas Library — Intent, Discovery, and Distributed Model Management Brief

## Purpose

Pumas should evolve from primarily exposing low-level model-management operations into a **model-management authority** that accepts high-level requirements and determines how to satisfy them.

Today, applications can use Pumas operations such as searching for models, downloading them, importing them, inspecting the library, and reconciling state. This exposes substantial implementation detail to consumers. The consuming application may consequently need to understand *how* Pumas should obtain and prepare a model rather than simply describing *what model it requires*.

The proposed direction is:

> **Consumers describe the model state they require. Pumas resolves and maintains that state.**

The same domain model should apply whether the consumer is a local application, another Pumas node, a fleet controller, Pantograph, a CLI, or an external management system such as a Kubernetes operator.

---

## 1. Introduce an intent-level Pumas API

Pumas should gain a higher-level domain API above its existing operational functionality.

Instead of a client orchestrating:

```text
search
→ inspect results
→ choose artifact
→ download
→ resume if necessary
→ import
→ verify
→ reconcile
→ locate artifact
→ provide model to application
```

the client should be able to express something conceptually similar to:

```text
GetModel {
    model: "Qwen/Qwen3.8-27B"

    requirements {
        format: GGUF
        quantization: Q4_K_M
    }
}
```

Pumas then determines how that requirement should be satisfied:

```text
GetModel(requirement)
        │
        ▼
    Resolve state
        │
        ├── already available ───────→ return
        │
        ├── invalid ─────────────────→ repair
        │
        ├── partial ─────────────────→ resume
        │
        ├── available locally ───────→ use/adopt
        │
        ├── available from peer ─────→ acquire
        │
        ├── available from cache ────→ acquire
        │
        └── available upstream ──────→ acquire
                                            │
                                            ▼
                                      verify/index
                                            │
                                            ▼
                                          return
```

The application should not normally need to know which of these operations occurred.

### API layers

The existing functionality should not necessarily disappear. Instead, distinguish two levels:

**Intent/domain API**

```text
get_model
ensure_model
release_model
query_models
get_model_status
```

**Operational/administrative API**

```text
search_huggingface
download
import
reconcile
repair
inspect_cache
```

Normal application integrations should prefer the intent API. Administrative applications, the Pumas GUI, diagnostics, and specialized integrations can still use lower-level operations.

---

## 2. Distinguish immediate requests from desired state

Two related concepts should be considered.

### `GetModel`

An application says:

> Give me a model satisfying these requirements, making it available if necessary.

The result could be a `ModelHandle` containing the resolved artifact, revision, path/reference, metadata, runtime compatibility, etc.

### `EnsureModel`

Infrastructure says:

> This model should exist here and Pumas should reconcile actual state toward that desired state.

For example:

```text
DesiredModelState
    model: Qwen3.8-27B
    revision: abc123
    format: GGUF
    quantization: Q4_K_M
    state: Present
```

Observed state might temporarily be:

```text
ObservedModelState
    state: Downloading
    progress: 63%
```

Eventually:

```text
desired state == observed state
```

This declarative approach provides a basis for retries, recovery, distributed management, and idempotent operations.

---

# 3. Define a transport-independent Pumas domain language

The domain model should not be designed specifically as an HTTP API or cluster RPC protocol.

Define concepts representing **intent and state** independently of transport.

Potential concepts include:

```text
ModelRequirement
ArtifactRequirement
ModelHandle

DesiredModelState
ObservedModelState

NodeIdentity
NodeCapabilities
NodeSnapshot

ArtifactIdentity
ArtifactLocation
ArtifactOffer
ArtifactTransferRequest
```

The important distinction is that these should describe meaningful Pumas concepts rather than procedural instructions.

Avoid making the distributed language primarily:

```text
DownloadModel
MoveModel
ImportModel
FixDatabase
```

Those describe implementation mechanisms.

Prefer:

```text
GetModel
EnsureModel
ReleaseModel
VerifyModel
ReportInventory
```

A receiving Pumas implementation decides which lower-level operations are required.

---

# 4. Use the same domain language for applications and clusters

This is a central design goal.

A local application could issue:

```text
Application
    │
    │ GetModel(requirement)
    ▼
Pumas
```

A fleet controller could issue:

```text
Fleet Controller
    │
    │ EnsureModel(requirement)
    ▼
Pumas Node
```

Pantograph could eventually issue the same kind of requirement.

The semantic language remains consistent. Only transport, authorization, lifetime, and authority differ.

This prevents Pumas from developing separate conceptual APIs for:

- embedded applications;

- local applications;

- remote applications;

- Pumas nodes;

- fleet controllers;

- managed infrastructure.

---

# 5. Introduce Pumas nodes and optional fleet management

A machine participating in distributed Pumas management can expose itself as a **Pumas node**.

A node reports its identity, capabilities, libraries, models, health, and relevant state.

Conceptually:

```text
NodeSnapshot
    identity
    protocol versions
    Pumas version
    capabilities
    libraries
    models
    downloads/acquisitions
    runtime capabilities
    storage
    health
```

A fleet controller can aggregate these snapshots:

```text
                 Pumas Fleet

                 Controller
                /    |     \
               /     |      \
              ▼      ▼       ▼
           Node A  Node B   Node C
```

This provides answers such as:

```text
Qwen3.8-27B

Node A    Ready
Node B    Missing
Node C    Acquiring 42%
```

The controller maintains the distributed view, while each local Pumas installation remains authoritative about the actual contents and validity of its libraries.

---

# 6. Separate the control plane from artifact transfer

Cluster communication should distinguish between management information and model data.

### Control plane

Handles:

```text
node identity
capabilities
inventory
health
desired model state
observed model state
artifact locations
```

### Data plane

Handles potentially very large model artifacts.

For example:

```text
             Fleet Controller
              /           \
       EnsureModel       inventory
            ↓                ↑
         Node A            Node B
            │                ▲
            └── artifact ────┘
```

The controller does not need to proxy a 30 GB model.

It can tell Node B that the required immutable artifact is available from Node A, a central cache, or an upstream provider.

Pumas then chooses an appropriate acquisition source.

---

# 7. Allow peer artifact acquisition without requiring peer-to-peer architecture

Pumas nodes should eventually be capable of serving verified artifacts to one another.

A resolver could consider:

```text
1. active local library
2. another local library
3. partial/local cache
4. Pumas peer
5. organization artifact cache
6. shared Pumas cache
7. Hugging Face/upstream source
```

This does not require every Pumas node to gossip or independently coordinate the cluster.

A centralized or externally managed control plane can coexist with direct node-to-node artifact transfer.

---

# 8. Unify local and distributed discovery

The current ownership/discovery problem and cluster discovery are closely related.

Pumas should have a general concept of discovering:

> **libraries and Pumas endpoints**

rather than simply finding a currently running owner process.

Conceptually:

```text
Pumas Discovery
       │
       ├── local machine registry
       ├── local service discovery
       ├── LAN/network discovery
       └── managed fleet registry
```

The upper domain layer should not need to care how the information was discovered.

---

# 9. Persist library identity independently of running processes

Live process discovery alone cannot solve an important existing problem.

Consider:

```text
Application A previously used Pumas
        │
        ▼
Library exists at /data/models

Application A stops.

Application B starts later.
```

Application B needs to discover that `/data/models` is an existing Pumas library even though no Pumas process currently owns it.

Therefore libraries should have durable identities.

Conceptually:

```text
PumasLibraryIdentity
    library_id
    schema_version
    metadata
```

The library itself should contain its identity.

The machine should additionally maintain a small persistent registry of known libraries:

```text
Library 018ef...
    path: /data/models/pumas
    last_seen: ...
    schema: ...
```

The machine registry is a discovery mechanism/cache. The library itself remains the authority for its own identity.

This prevents an application from unnecessarily creating another model library simply because the application that previously used Pumas is not currently running.

---

# 10. Separate discovery from ownership/authority

The existing root ownership model was created partly to solve two problems simultaneously:

1. discover the appropriate existing Pumas library;

2. prevent multiple processes from conflicting while modifying it.

These should become separate concerns.

### Discovery

Answers:

> Where are the libraries?

### Authority

Answers:

> Who currently has permission to mutate this library?

### Reconciliation

Answers:

> What state should the library be in, and how do we make it true?

### Transport

Answers:

> How do I communicate with the authority?

This separation preserves the safety property of the existing ownership model without requiring ownership to also serve as library discovery.

---

# 11. Preserve single-writer safety, not necessarily the current ownership mechanism

Multiple applications should be able to use the same Pumas library simultaneously.

What should generally be prevented is:

> multiple independent authorities concurrently mutating the same library.

For example:

```text
App A ─┐
App B ─┼──→ Library Authority ──→ Library
App C ─┘
```

The authority can serialize mutation, coordinate downloads, perform reconciliation, and publish state changes.

This could eventually use leases or another explicit authority mechanism rather than treating "root owner" as both identity and discovery.

---

# 12. Consider a host-level Pumas daemon

The resulting architecture makes an optional `pumasd` service attractive.

Instead of:

```text
App A → Pumas instance → library
App B → Pumas instance → library
App C → Pumas instance → library
```

a machine could operate:

```text
        App A ─┐
        App B ─┼─→ pumasd
        App C ─┘      │
                      ▼
                  Pumas Core
                      │
                      ▼
                   Library
```

Benefits include:

- one library authority;

- shared model inventory;

- download deduplication;

- shared cache;

- centralized reconciliation;

- concurrent client support;

- one machine/node identity;

- straightforward fleet participation.

Pumas should still retain embedded operation where appropriate.

Thus there could be:

```text
Embedded mode
Application → Pumas Core → Library

Service mode
Application → Pumas protocol → pumasd → Pumas Core → Library
```

Both should expose essentially the same intent-level domain semantics.

---

# 13. Make deployment management external to Pumas

Pumas should manage **model artifacts**, not entire server/software deployments.

A production environment may use:

```text
OCI/Docker
Kubernetes
Nomad
VM images
Nix/NixOS
OS/package management
cloud deployment systems
```

to determine which software runs on a server.

For example:

```text
Server Image
    application
    Pumas
    runtime dependencies

Persistent Storage
    Pumas libraries
    model artifacts
```

Updating the server image should not require rebuilding or redownloading the model library.

This also means different nodes can run different Pumas versions while sharing a defined protocol.

---

# 14. Support managed environments through adapters

The Pumas distributed domain model should not depend on a Pumas-specific fleet controller.

For example, Kubernetes integration could eventually provide:

```text
Kubernetes desired state
        │
        ▼
Pumas Operator
        │
        ▼
Pumas domain protocol
        │
        ▼
Pumas node
```

A standalone Pumas deployment could instead use:

```text
Pumas Fleet Controller
        │
        ▼
Pumas domain protocol
        │
        ▼
Pumas node
```

Both communicate equivalent intent.

This allows Pumas to work naturally in simple desktop/server environments and highly managed production infrastructure.

---

# 15. Include capability and protocol negotiation

Nodes should not assume identical Pumas versions.

A node should advertise something conceptually similar to:

```text
Pumas version: 0.9.1

Protocol versions:
    1
    2

Capabilities:
    artifact.ensure
    artifact.peer-transfer
    artifact.verify.sha256
    runtime.llamacpp
```

A controller or another node can therefore determine what operations and protocol versions are mutually supported.

This enables gradual upgrades and future extension of the domain language.

---

# 16. Rename/refactor the existing `network` domain

The existing `pumas-core/src/network` name will become confusing once Pumas has actual node networking.

Currently that area contains concepts such as HTTP clients, retries, downloads, GitHub/web access, circuit breakers, and internet connectivity. The public API similarly uses "network" to mean internet/upstream connectivity and circuit-breaker state.

It should be renamed or decomposed before introducing distributed Pumas concepts.

Possible terminology includes:

```text
upstream
external
remote_sources
transport
http
```

The eventual node/fleet domain should have an unambiguous name such as:

```text
fleet
cluster
node
distributed
```

The exact naming should be determined during architecture planning based on the responsibilities remaining in the existing module.

---

# 17. Do not simply expose the existing RPC server over the network

The current architecture intentionally restricts `pumas-rpc` to loopback communication.

That should not simply become an unrestricted LAN API.

Local administrative RPC and remote node control have different security requirements.

A future architecture could resemble:

```text
pumas-rpc
    full/local API
    loopback or local IPC

pumas-node
    restricted remote interface
    authenticated/encrypted
    capability negotiated
    domain-intent operations only
```

Remote consumers should not automatically gain access to arbitrary local paths, process control, GUI functionality, raw database operations, or every low-level Pumas RPC method.

---

# 18. Responsibility boundaries

The intended long-term division is:

```text
Deployment infrastructure
"What software should exist on this machine?"
             │
             ▼
       Pumas Fleet
"What model artifacts should exist on which machines?"
             │
             ▼
       Local Pumas
"How do I make the requested model state true?"
             │
             ▼
        Pantograph
"Where and when should computational workloads execute?"
```

Pumas can expose information useful to Pantograph—model availability, runtime compatibility, node capabilities, etc.—without becoming a workload scheduler.

---

## Target architectural model

Taken together, the proposed direction looks approximately like:

```text
                 APPLICATION / PANTOGRAPH
                           │
                    Model Requirement
                           │
                           ▼
                 PUMAS DOMAIN LANGUAGE
                  /                  \
                 /                    \
          Local transport          Fleet transport
               │                         │
               ▼                         ▼
             pumasd               Remote Pumas node
               │                         │
               └──────────┬──────────────┘
                          ▼
                    Pumas Resolver
                          │
                    Desired State
                          │
                          ▼
                     Reconciler
                          │
          ┌───────────────┼────────────────┐
          ▼               ▼                ▼
     Local Library    Local/Peer       Upstream
                          │
                          ▼
                    Artifact Transfer
```

Discovery sits alongside this:

```text
                    Pumas Discovery
                          │
          ┌───────────────┼────────────────┐
          ▼               ▼                ▼
    Machine Registry   Live Services    Fleet Registry
          │               │                │
          └───────────────┼────────────────┘
                          ▼
                 Known Libraries/Nodes
```

And authority is separately established for each mutable library.

## Core development principle

The key architectural change can be summarized as:

> **Pumas should expose model-management intent as its primary abstraction. Consumers specify the model state they need; Pumas owns resolution, acquisition, verification, reconciliation, and location. Library discovery, library authority, communication transport, and distributed fleet management should be separate concerns built around that common domain model.**

The existing operational APIs remain useful implementation capabilities, but they should increasingly sit **beneath** this abstraction rather than defining how applications are expected to integrate with Pumas.

This brief should be treated as architectural intent rather than an implementation plan. A development plan should first map these concepts onto the existing `PumasApi`, RPC, reconciliation, library-ownership, model-resolution, event, persistence, and networking boundaries before deciding milestones or concrete types.
