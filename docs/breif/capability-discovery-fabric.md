# Pumas Library — Capability Discovery and Resolution Fabric

## Purpose

Pumas should provide a unified mechanism for discovering both **Pumas itself** and the **capabilities available through Pumas**, regardless of whether those capabilities are provided by:

- the local Pumas process;

- a local Pumas daemon;

- an embedded Pumas instance;

- a runtime plugin;

- a locally hosted inference gateway;

- another Pumas node on the LAN;

- another node in a managed cluster;

- an externally managed deployment such as Kubernetes;

- or a future remote Pumas service.

The discovery architecture should not be centered around finding ports.

Instead:

> **Consumers should request capabilities. Pumas should resolve those capabilities to currently valid providers and endpoints.**

Ports, sockets, process IDs, runtime implementations, and network topology should normally remain implementation details.

The same discovery semantics should work from a single desktop installation through a multi-node production cluster.

---

# 1. Core design principle

Pumas discovery should distinguish between:

```text
Finding Pumas
      │
      ▼
Finding capabilities through Pumas
      │
      ▼
Resolving a usable provider
```

An application should not need to:

```text
scan ports
→ identify Pumas
→ identify runtime
→ determine API compatibility
→ inspect loaded models
→ select provider
→ track endpoint changes
```

It should instead be able to express something conceptually similar to:

```text
ResolveCapability {
    capability: inference.embeddings

    require {
        protocol: openai.v1
        model: Qwen/Qwen3-Embedding
    }

    prefer {
        local
        already_loaded
    }
}
```

Pumas resolves the current environment and returns a usable provider.

---

# 2. Treat endpoints as ephemeral

An endpoint should not be the durable identity of a service.

The following may change at any time:

```text
TCP port
IP address
Unix socket path
runtime process
runtime implementation
cluster node
model-serving process
```

The durable concepts should instead be:

```text
NodeIdentity
ServiceIdentity
CapabilityIdentity
ProtocolIdentity
```

For example:

```text
Capability:
    inference.chat

Protocol:
    openai.v1

Provider:
    service-8fa7...

Endpoint:
    127.0.0.1:43821
```

The endpoint is only the current route to the provider.

The architectural rule should therefore be:

> **Endpoints are ephemeral implementation details. Capabilities are the durable public interface.**

---

# 3. Introduce a node-local Pumas authority

The ideal local architecture should include an optional host-level service:

```text
pumasd
```

`pumasd` acts as the local Pumas node agent and capability resolver.

Conceptually:

```text
                     MACHINE

                ┌──────────────┐
                │    pumasd    │
                │ Node Agent   │
                └──────┬───────┘
                       │
       ┌───────────────┼─────────────────┐
       │               │                 │
       ▼               ▼                 ▼
   Libraries        Plugins           Runtimes
                                        │
                                  llama.cpp
                                  Ollama
                                  ONNX
                                  future runtimes
```

Applications communicate with `pumasd` rather than independently discovering every Pumas-backed service.

Embedded Pumas operation should remain supported.

An embedded Pumas instance may register its capabilities with `pumasd` when one is present, while still functioning independently when no daemon exists.

---

# 4. Use an OS-native local rendezvous mechanism

Finding the local Pumas authority should not require TCP port discovery.

Prefer a stable local operating-system primitive.

For example:

```text
Linux/macOS
    Unix domain socket

Windows
    named pipe
```

Conceptually:

```text
/run/user/<uid>/pumas/control.sock
```

or:

```text
\\.\pipe\pumas\<user>
```

The exact location should be platform-defined and stable.

Applications can therefore locate Pumas deterministically:

```text
Application
     │
     ▼
well-known local Pumas endpoint
     │
     ▼
pumasd
```

Loopback TCP may remain available as a fallback transport but should not be the preferred discovery mechanism where better native IPC exists.

---

# 5. Build a capability resolver, not merely a service registry

The public discovery interface should expose capability-oriented operations such as:

```text
ResolveCapability
WatchCapability
ListCapabilities
DescribeCapability
```

The simplest common operation is:

```text
ResolveCapability(requirement)
```

A request might conceptually contain:

```text
ResolveCapability {
    capability: inference.chat

    require {
        protocol: openai.v1
        model: Qwen/Qwen3.8-27B
        streaming: true
        context_length: >= 32768
    }

    prefer {
        locality: local
        loaded: true
    }
}
```

The result might contain:

```text
CapabilityResolution {
    resolution_id
    capability
    provider
    protocol
    endpoint
    state
    generation
    expiry
}
```

Consumers therefore ask Pumas for what they need rather than discovering infrastructure and implementing their own selection logic.

---

# 6. Define semantic capabilities

Capabilities should describe useful Pumas functionality rather than implementation mechanisms.

Avoid treating these as primary capabilities:

```text
http_server
tcp_port
llamacpp_process
ollama_port
```

Prefer semantic concepts such as:

```text
model.query
model.ensure
model.release
model.verify

artifact.acquire
artifact.transfer
artifact.serve

inference.chat
inference.completions
inference.embeddings
inference.rerank

protocol.openai.v1
protocol.mcp

runtime.llamacpp
runtime.ollama
runtime.onnx
```

Capabilities should be composable.

For example:

```text
inference.embeddings

protocol:
    openai.v1

modalities:
    text

features:
    batching

model:
    Qwen3-Embedding
```

This allows applications to find services according to meaningful requirements.

---

# 7. Capabilities should be typed advertisements

A capability advertisement should contain substantially more information than an endpoint.

Conceptually:

```text
CapabilityAdvertisement

identity
    node_id
    service_id
    instance_id

capability
    capability_id
    capability_version

protocols
    supported_protocols

constraints
    model_families
    formats
    quantizations
    context_limits
    modalities
    batching
    streaming

state
    capable
    available
    ready
    degraded

location
    node
    cluster
    region

endpoints
    unix_socket
    named_pipe
    tcp
    quic
    http

security
    trust_domain
    authorization_scope

health
    generation
    lease_expiry
    last_verified

dynamic_state
    model_loaded
    artifact_local
    startup_cost
    current_pressure
```

This representation should be transport-independent.

---

# 8. Separate capability, availability, and readiness

These are different concepts and should not be collapsed.

A node may be capable of serving a model without currently serving it.

For example:

```text
Node A

capable:
    yes

model artifact:
    local

runtime:
    available

model loaded:
    no

ready:
    no
```

Another node might report:

```text
Node B

capable:
    yes

model loaded:
    yes

ready:
    yes
```

Pumas should therefore distinguish:

```text
Capability
    This provider can perform the operation.

Availability
    The provider has the resources/state needed to perform it.

Readiness
    A consumer may use it immediately.

Activation Cost
    The work required to become ready.
```

Activation cost might include:

```text
model load
runtime startup
artifact transfer
artifact download
runtime installation
```

This information can greatly improve capability resolution.

---

# 9. Unify local and distributed discovery

The same upper-level capability API should work regardless of where the provider exists.

Internally, discovery may use:

```text
                     Capability Resolver
                              │
          ┌───────────────────┼────────────────────┐
          │                   │                    │
          ▼                   ▼                    ▼
    Local Registry        LAN Discovery       Cluster Directory
          │                   │                    │
          ▼                   ▼                    ▼
 local processes       nearby Pumas nodes     managed fleet
```

Consumers should not need to know which mechanism produced the result.

This creates one discovery model for:

```text
same process
same machine
LAN
cluster
managed infrastructure
remote service
```

---

# 10. Support several discovery transports

No single discovery transport is ideal for every environment.

Pumas should support multiple mechanisms feeding the same discovery layer.

## Local machine

Use the stable `pumasd` IPC endpoint.

## Zero-configuration LAN

Support service discovery such as:

```text
mDNS / DNS-SD
```

Conceptually:

```text
_pumas._tcp.local
```

This should primarily locate Pumas nodes, not advertise every runtime port independently.

## Explicit cluster formation

Allow seed-based joining:

```text
pumas cluster join <node>
```

## Managed network environments

Support mechanisms such as:

```text
DNS SRV
```

for discovering known control-plane endpoints.

## Kubernetes

Integrate through existing infrastructure such as:

```text
Services
EndpointSlices
CRDs
Pumas Operator
```

## Managed Pumas fleets

Support a dedicated replicated Pumas directory.

All of these should feed the same capability resolver.

---

# 11. Introduce a cluster capability directory

For managed Pumas clusters, capability state should be aggregated into a replicated directory.

Conceptually:

```text
             Pumas Cluster Directory

             ┌──────┬──────┬──────┐
             │      │      │      │
             ▼      ▼      ▼      ▼
            C1     C2     C3     ...
```

A consensus protocol such as Raft can provide strong consistency for important cluster metadata.

The directory should contain information such as:

```text
node identities
node membership
service identities
capability advertisements
protocol support
leases
health information
artifact locations
```

The directory is a distributed view of the cluster.

Individual nodes remain authoritative regarding their own actual state.

---

# 12. Use renewable leases for ephemeral services

Service advertisements should not persist indefinitely.

Each runtime or node registration should have a lease.

For example:

```text
Node A advertises:

    inference.embeddings
    artifact.transfer
    runtime.llamacpp

lease:
    15 seconds
```

The node continually renews the lease.

If it disappears:

```text
lease expires
      │
      ▼
advertisement removed
```

This prevents stale ports and dead runtime registrations from remaining discoverable.

Local process-backed services may additionally attach their registration lifetime directly to process lifetime.

---

# 13. Nodes remain authoritative for their own state

The cluster directory should not independently determine facts such as:

```text
model loaded
artifact valid
runtime healthy
GPU available
```

The owning node should determine and publish those facts.

Conceptually:

```text
Node
 │
 │ authoritative observed state
 ▼
Capability Advertisement
 │
 ▼
Cluster Directory
 │
 ▼
Distributed cached view
```

This keeps Pumas's existing model intact:

> The local Pumas installation is authoritative regarding the actual contents and state of its libraries and runtimes.

The cluster aggregates that information rather than replacing local authority.

---

# 14. Give every node a durable cryptographic identity

Node identity should not primarily depend on:

```text
hostname
IP address
TCP port
```

Instead, a Pumas node should have a durable identity backed by a cryptographic key pair.

Conceptually:

```text
NodeIdentity {
    public_key
    node_id = hash(public_key)
}
```

The node may then change:

```text
IP
hostname
network
port
physical location
```

while retaining the same identity.

This also provides a foundation for authentication, trust relationships, and secure cluster membership.

---

# 15. Secure distributed discovery and control

Cluster communication should be authenticated and encrypted.

A production deployment should support something equivalent to:

```text
mutual TLS
```

with identities tied to Pumas nodes or managed workload identities.

A capability should therefore be associated with:

```text
provider identity
trust domain
authorization requirements
```

Remote discovery must not imply remote administrative access.

The existing local Pumas RPC surface should not simply be exposed to the network.

Instead:

```text
Local Pumas API
    broad local authority
    local IPC

Distributed Pumas API
    restricted domain-level operations
    authenticated
    authorized
    encrypted
```

---

# 16. Runtime plugins register capabilities with Pumas

Runtime plugins should dynamically register what they provide.

For example:

```text
llama.cpp plugin
      │
      │ starts runtime on port 0
      ▼
OS selects port 48317
      │
      ▼
plugin verifies runtime
      │
      ▼
RegisterCapability
```

The plugin might register:

```text
inference.chat
inference.completions
protocol.openai.v1
```

with an endpoint such as:

```text
127.0.0.1:48317
```

If the runtime dies:

```text
process exit
     │
     ▼
capability revoked
```

Consumers do not need to know that llama.cpp was involved.

---

# 17. Prefer a stable Pumas gateway for application-facing APIs

For ordinary applications, Pumas should normally expose a stable gateway rather than exposing raw runtime endpoints.

Conceptually:

```text
Application
     │
     │ OpenAI-compatible API
     ▼
Pumas Gateway
     │
     ├── llama.cpp
     ├── Ollama
     ├── ONNX runtime
     ├── another plugin
     └── remote Pumas provider
```

Benefits include:

- stable application endpoints;

- runtime replacement without consumer changes;

- runtime restart without permanent endpoint invalidation;

- centralized authentication;

- centralized observability;

- protocol normalization;

- model route changes;

- remote provider routing.

The application sees:

```text
protocol:
    openai.v1

endpoint:
    Pumas gateway
```

rather than a runtime-specific endpoint.

---

# 18. Do not force the gateway into every data path

The gateway should not become a mandatory proxy for high-throughput distributed traffic.

Pumas should support two resolution styles.

## Proxy resolution

```text
Application
    │
    ▼
Local Pumas Gateway
    │
    ▼
Provider
```

Best for simple applications and stable APIs.

## Direct resolution

```text
Application
    │
    │ ResolveCapability
    ▼
Pumas
    │
    ▼
Signed Connection Descriptor
    │
    ▼
Provider Node
```

A direct result could contain:

```text
endpoint
provider identity
protocol
temporary credentials
expiry
resolution generation
```

This avoids unnecessary network hops while preserving centralized discovery and authorization.

---

# 19. Support hard requirements and soft preferences

Capability resolution should distinguish between requirements and ranking preferences.

For example:

```text
ResolveCapability {
    capability: inference.chat

    require {
        model: Qwen3.8-27B
        quantization: >= Q4
        context_length: >= 32768
        streaming: true
    }

    prefer {
        local_node
        already_loaded
        artifact_local
        low_startup_cost
        low_network_cost
    }
}
```

Requirements determine which providers are valid.

Preferences determine which valid provider is preferred.

This prevents every consuming application from implementing its own selection policy.

---

# 20. Make locality an explicit concept

Pumas should understand locality levels such as:

```text
same_process
same_machine
same_LAN
same_cluster
same_region
remote
```

Locality matters because model artifacts can be extremely large.

The resolver may therefore naturally prefer:

```text
already loaded locally
        ↓
local artifact available
        ↓
nearby provider already serving
        ↓
nearby peer owns artifact
        ↓
organization cache
        ↓
remote cache
        ↓
upstream source
```

This fits naturally with Pumas's existing model acquisition and artifact-resolution responsibilities.

---

# 21. Add reactive discovery subscriptions

Consumers should not need to continuously poll discovery state.

Expose an operation conceptually similar to:

```text
WatchCapability(requirement)
```

which can emit:

```text
ADDED
CHANGED
REMOVED
```

For example:

```text
Node B appears
    → provider ADDED

Node A finishes loading model
    → provider CHANGED

Node A becomes unhealthy
    → provider CHANGED

Node A disappears
    → provider REMOVED
```

This mechanism should extend the existing Pumas use of local update streams into capability and cluster discovery.

---

# 22. Return capability handles rather than permanent endpoints

Applications should not permanently store resolved ports.

A resolution should instead produce a handle.

Conceptually:

```text
CapabilityHandle {
    resolution_id
    capability
    provider
    generation
    endpoint
    expiry
}
```

The handle can be:

```text
refreshed
re-resolved
watched
invalidated
```

If the provider changes:

```text
runtime restart
port change
node failure
model migration
provider upgrade
```

Pumas can resolve another provider without forcing the application to rediscover the entire environment.

---

# 23. Version capabilities independently of Pumas releases

Compatibility should not depend on identical Pumas software versions.

Avoid:

```text
Pumas 1.3 requires Pumas 1.3
```

Prefer independently versioned protocols and capabilities:

```text
Pumas version:
    1.8.0

Discovery protocols:
    discovery.v3

Domain protocols:
    model-intent.v2
    artifact-transfer.v4

Capabilities:
    model.ensure@2
    inference.openai@1
```

This allows rolling upgrades and mixed-version clusters.

Protocol negotiation should determine compatibility.

---

# 24. Separate discovery from scheduling

Pumas may expose useful selection information such as:

```text
artifact locality
model loaded state
runtime compatibility
GPU compatibility
activation cost
service health
service pressure
```

However, Pumas should not become a general workload scheduler.

For Pantograph integration:

```text
Pantograph
     │
     │ capability requirement
     ▼
Pumas Resolver
     │
     │ candidate providers
     ▼
Pantograph Scheduler
     │
     │ execution placement
     ▼
Provider
```

Pumas answers:

> Where can this requirement be satisfied?

Pantograph answers:

> Where and when should this workload execute?

This keeps responsibilities clean.

---

# 25. Separate discovery plane, control plane, and data plane

The architecture should explicitly distinguish three concerns.

## Discovery plane

Handles:

```text
node discovery
service discovery
capability advertisements
capability resolution
protocol negotiation
```

## Control plane

Handles:

```text
intent
desired state
observed state
health
authorization
model management
artifact locations
```

## Data plane

Handles:

```text
model inference
artifact transfer
large binary data
streaming
```

Conceptually:

```text
DISCOVERY PLANE
    Who can satisfy this requirement?

CONTROL PLANE
    What state should exist?

DATA PLANE
    Move or process the actual data.
```

These planes may use different transports and optimization strategies.

---

# 26. Preserve current local registry work as a bootstrap/fallback

The existing machine-level Pumas registry remains useful.

It can continue storing information such as:

```text
library identity
PID
transport
endpoint
readiness
version
```

but it should become one discovery source rather than the public discovery contract.

Applications should not be expected to inspect the registry database directly.

Instead:

```text
registry
    │
    ▼
Pumas discovery implementation
    │
    ▼
stable discovery API
```

This allows registry schema and transport details to evolve independently of consumers.

---

# 27. Proposed domain concepts

The capability-discovery domain may eventually contain types conceptually similar to:

```text
NodeIdentity
NodeDescriptor
NodeHealth

ServiceIdentity
ServiceDescriptor

CapabilityId
CapabilityVersion
CapabilityAdvertisement
CapabilityRequirement
CapabilityPreference
CapabilityResolution
CapabilityHandle

ProtocolDescriptor
EndpointDescriptor

DiscoverySource
Locality

Lease
Generation

WatchRequest
CapabilityChange
```

These should remain independent of specific transports.

The same objects should be usable through:

```text
local IPC
Rust API
HTTP
cluster RPC
MCP
CLI
future language bindings
```

---

# 28. Example local workflow

An application needs embeddings:

```text
Application
     │
     │ ResolveCapability
     │ inference.embeddings
     ▼
pumasd
     │
     ├── ONNX plugin: READY
     ├── llama.cpp: capable but not loaded
     └── remote node: READY
     │
     ▼
Resolver policy
     │
     ▼
local ONNX provider selected
     │
     ▼
CapabilityHandle
```

The application never discovers the ONNX runtime's port directly unless the selected protocol requires direct connection.

---

# 29. Example cluster workflow

An application requests:

```text
inference.chat

model:
    Qwen3.8-27B

protocol:
    openai.v1
```

The local resolver sees:

```text
Local Node
    capable
    artifact missing

Node B
    capable
    artifact local
    model unloaded

Node C
    ready
    model loaded
```

Resolution may return:

```text
Node C
```

If Node C disappears, the capability watch emits a change.

Pumas may then resolve:

```text
Node B
```

possibly triggering model activation.

The consumer does not need to understand the underlying topology.

---

# 30. Target architecture

```text
                            APPLICATION
                                 │
                        ResolveCapability
                                 │
                                 ▼
                       Local Pumas Resolver
                            (`pumasd`)
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
              ▼                  ▼                  ▼
          Local State        LAN Peers       Cluster Directory
              │                                     │
              │                              replicated state
              │                                     │
              └─────────────────┬───────────────────┘
                                │
                                ▼
                      Capability Candidates
                                │
                         requirements filter
                                │
                         preference ranking
                                │
                                ▼
                       Capability Handle
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
              Pumas Gateway          Direct Provider
                    │                       │
               local runtime           remote node
                    │                       │
                    └───────────┬───────────┘
                                ▼
                            DATA PLANE
```

---

# 31. Design outcome

With this architecture, Pumas discovery becomes substantially more than port discovery.

It becomes the system responsible for answering:

> **What Pumas-backed capability can satisfy this requirement right now, and how should the caller reach it?**

Applications no longer need to know:

```text
which runtime is installed
which plugin owns the runtime
which port it selected
which process hosts it
whether the model is local
whether the model is already loaded
whether another node is preferable
whether the provider moved
```

Those become Pumas responsibilities.

The resulting abstraction is:

```text
Consumer requirement
        │
        ▼
Pumas capability resolver
        │
        ▼
best valid provider
```

This mechanism should be designed from the beginning to work equally well for one application on one machine and for large distributed Pumas deployments.

The long-term goal should therefore be:

> **Pumas provides a universal capability-resolution layer between model consumers and the infrastructure capable of satisfying their requirements.**
