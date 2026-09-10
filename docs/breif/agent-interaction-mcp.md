# Pumas Library — Agent Interaction and MCP Layer Brief

## Purpose

Pumas should expose an explicit **agent interaction layer** that allows coding agents and autonomous systems to use Pumas through a small, stable, intent-oriented interface.

The agent layer should not expose the full internal Pumas API directly. Agents should not be expected to orchestrate low-level actions such as searching a provider, downloading files, importing them, reconciling metadata, locating model files, or repairing library state.

Instead:

> **Agents describe what model capability or model state they need. Pumas determines how to satisfy that request.**

The agent layer should be implemented as a projection of the same higher-level Pumas domain API used by ordinary applications and distributed Pumas nodes.

MCP is the preferred first transport for agent interaction, but MCP must remain an adapter over the Pumas domain rather than becoming the definition of the Pumas API.

---

## 1. Architectural position of the agent layer

The intended hierarchy is:

```text
Pumas Core Operations
        │
        ▼
Pumas Domain / Intent API
        │
        ├───────────────┬───────────────┐
        ▼               ▼               ▼
    Native API       RPC/HTTP          MCP
        │               │               │
 Applications       Services          Agents
```

The agent layer should depend on the intent/domain API, not directly on internal model-library mechanics.

This gives Pumas one conceptual language regardless of caller:

```text
Application
Agent
Fleet Controller
Pantograph
CLI
Kubernetes Operator
```

All can ultimately express the same kinds of requests.

---

## 2. Agents are clients of the Pumas domain, not its internals

The normal agent workflow should not require tools such as:

```text
search_huggingface
download_file
import_model_directory
move_model
repair_database
reconcile_library
```

Those remain useful low-level or administrative operations, but they should not form the primary agent contract.

The agent-facing vocabulary should express domain intent, for example:

```text
find_model
get_model
ensure_model
inspect_model
get_model_status
list_models
list_libraries
inspect_library
discover_nodes
inspect_node
```

Later distributed capabilities may add operations such as:

```text
ensure_model_on_node
find_model_locations
inspect_fleet
```

The agent should specify the desired result, while Pumas decides whether that requires a local lookup, reconciliation, repair, resume, peer acquisition, cache lookup, or upstream download.

---

## 3. Model requirements should be structured domain inputs

The agent layer should accept structured requirements rather than forcing agents to know specific filenames or implementation details.

For example:

```text
ModelRequirement
    model: Qwen/Qwen3.8-27B
    purpose: text-generation
    format: GGUF
    quantization: Q4_K_M
    minimum_context: 32768
```

The same `ModelRequirement` concept should ideally be usable through:

```text
Rust API
RPC
MCP
Fleet protocol
```

The MCP tool schema is therefore a projection of the domain type rather than an independently invented interface.

---

## 4. Separate high-level agent tools from advanced/admin tools

A normal MCP catalog should remain deliberately small.

A possible initial default catalog is:

```text
get_capabilities
find_model
get_model
ensure_model
get_model_status
inspect_model
list_models
list_libraries
inspect_library
```

Distributed installations could additionally expose:

```text
discover_nodes
inspect_node
find_model_locations
ensure_model_on_node
```

Advanced operations such as raw provider search, manual reconciliation, cache repair, imports, or diagnostics should be placed in a separate advanced/admin catalog if they are exposed to agents at all.

This reduces the chance that agents bypass Pumas' intended domain abstraction.

---

## 5. Expose introspection so agents do not need to guess

An agent should be able to discover what a particular Pumas installation supports.

Useful introspection operations may include:

```text
get_capabilities
get_model_requirement_schema
list_supported_formats
list_supported_quantizations
list_supported_runtime_types
list_known_sources
```

For example:

```text
sources:
    local
    huggingface
    peer

formats:
    gguf
    safetensors
    onnx

runtimes:
    llama.cpp
    torch
    onnx-runtime

operations:
    get_model
    ensure_model
    inspect_model
```

This becomes especially important when Pumas installations differ by version, plugins, runtime support, or deployment environment.

Agents should obtain capabilities from Pumas instead of inventing or assuming them.

---

## 6. Return typed outcomes, not prose-oriented results

MCP responses should use structured result types that agents can reason about reliably.

For example:

```text
Ready {
    model
}
```

or:

```text
Pending {
    state: Acquiring
    progress: 42%
}
```

or:

```text
Unsatisfied {
    reason: NoCompatibleArtifact
    alternatives: [...]
}
```

A possible general result structure could distinguish:

```text
ready
pending
unsatisfied
rejected
error
```

Human-readable explanatory text can be included, but it should not be the authoritative representation of state.

---

## 7. Avoid agent-managed session IDs

The agent should not be required to remember a Pumas session ID.

Relying on an LLM's context to preserve something like:

```text
session_id = 7f2c...
```

across arbitrary calls adds unnecessary fragility.

If session or caller identity is useful internally, it should be attached deterministically by the transport or runtime.

For example:

```text
MCP connection
      │
      ▼
MCP adapter
    adds:
      client identity
      connection identity
      authorization context
      optional session context
      │
      ▼
Pumas domain request
```

The model does not need to know or carry those values.

---

## 8. Do not introduce opaque workflow handles unless the domain requires them

The Coding Standards engine uses handles because its domain contains genuinely multi-step workflows tied to exact snapshots, revisions, review state, and recovery state.

Pumas generally does not have that requirement.

A normal model acquisition is better represented as durable resource state:

```text
Desired:
    Model X = Present

Observed:
    Missing
      ↓
    Resolving
      ↓
    Acquiring
      ↓
    Verifying
      ↓
    Ready
```

The agent can ask:

```text
get_model_status(Model X)
```

rather than having to retain:

```text
operation_handle = op_1234
```

The resource itself should normally be sufficient to recover state.

Stable Pumas identifiers may include:

```text
LibraryId
NodeId
ModelId
ArtifactId
```

These are real domain identities, unlike conversational session handles.

---

## 9. Internal operation IDs may still exist

Pumas may still benefit from internal identifiers such as:

```text
request_id
acquisition_id
transfer_id
trace_id
```

These can support:

```text
logging
observability
debugging
cancellation
tracing
concurrent transfer differentiation
```

But these should not automatically become mandatory agent-facing state.

Expose them only where the user or caller genuinely needs to refer to the operation itself.

---

## 10. Caller identity may matter, but should be code-managed

There may eventually be situations where Pumas needs to know which consumer requires a model.

For example:

```text
Model X
    desired_by:
        application-a
        application-b
```

If one application exits, Pumas should not release the model if another still requires it.

This could make persistent caller identity useful.

However, that identity should be established by the application, daemon, transport, authentication layer, or MCP host—not remembered by the agent.

Conceptually:

```text
Client identity
    stable caller/application identity

Connection/session identity
    current transport interaction

Domain resource identity
    model/library/node/artifact
```

These concerns should remain separate.

---

## 11. Long-running model acquisition should be asynchronous

A call such as:

```text
get_model(requirement)
```

may discover that the requested model is not currently available and must be acquired.

Pumas should not require a tool invocation to remain blocked for the entire duration of a potentially large model download.

The preferred behavior is:

```text
Agent:
    get_model(requirement)

Pumas:
    model unavailable
    acquisition started
    state = pending

Pumas continues:
    resolving
    downloading
    verifying
    indexing

Eventually:
    model = ready
```

The authoritative state belongs to Pumas.

---

## 12. Use MCP Tasks for long-running work where supported

The MCP adapter should map long-running Pumas operations onto MCP's asynchronous task mechanism where appropriate.

Conceptually:

```text
get_model(requirement)
        │
        ▼
Ready locally?
   │           │
  yes          no
   │           │
return       begin acquisition
model           │
                ▼
          MCP task = working
                │
                ▼
            Pumas state
                │
      acquire / verify / index
                │
                ▼
              ready
                │
                ▼
        MCP task = completed
```

This allows Pumas to remain deterministic while the MCP layer exposes progress and completion in a standard agent-compatible way.

The Pumas domain itself should not depend on MCP Tasks.

---

## 13. Use MCP subscriptions for durable state observation

Model state is also naturally representable as a resource.

For example:

```text
pumas://models/<model-id>
```

The model could transition through:

```text
missing
resolving
acquiring
verifying
ready
failed
```

An MCP client can subscribe to relevant resources and receive update notifications when their state changes.

This is useful for observing durable state independently of the original tool call.

A reasonable distinction is:

```text
MCP Task
    represents a particular long-running request

MCP Resource
    represents durable Pumas state
```

---

## 14. Do not make Pumas correctness depend on waking the agent

MCP can notify a connected client that task or resource state changed.

That does not necessarily mean every MCP host will automatically invoke the LLM again.

Therefore:

```text
Pumas state machine
    authoritative

MCP notification
    delivery mechanism

Agent reactivation
    host/client policy
```

Pumas must remain correct even if no agent is actively running when acquisition completes.

For example:

```text
Agent requests Model X
Agent stops

Pumas finishes Model X

Later another client asks:
    get_model(Model X)

Pumas:
    Ready
```

No conversational state is required.

---

## 15. Prefer durable model state over conversational workflow state

The agent interface should be designed around resources and desired state.

For example:

```text
ensure_model(requirement)
```

means:

> This model should be available according to these requirements.

Pumas stores and reconciles that state independently of the original caller's conversational context.

This is preferable to treating model acquisition as:

```text
session
    → operation
    → callback
    → resume conversation
```

because model state belongs to Pumas, not to a particular LLM conversation.

---

## 16. MCP is an adapter, not the state engine

Pumas should own its own deterministic model lifecycle and reconciliation machinery.

Conceptually:

```text
Model Intent
    │
    ▼
Desired State
    │
    ▼
Pumas Resolver
    │
    ▼
Pumas Reconciler
    │
    ▼
Observed State
```

MCP then projects this into agent-oriented concepts:

```text
Pumas state
    │
    ├── MCP tool results
    ├── MCP tasks
    └── MCP resource notifications
```

This allows the same underlying logic to work for:

```text
native applications
pumasd clients
remote Pumas nodes
Pantograph
REST clients
MCP agents
```

---

## 17. Provide an agent skill describing correct usage

Pumas should include an agent skill similar in role to the Coding Standards repository's skill.

Conceptually:

```text
.agents/
    skills/
        pumas/
            SKILL.md
            references/
                models.md
                capabilities.md
                environments.md
                distributed-use.md
```

The skill should teach agents the intended interaction rules.

For example:

```text
Express model requirements through intent-level operations.

Allow Pumas to choose how requirements are satisfied.

Do not directly mutate Pumas-managed files, databases, or metadata.

Do not manually download/import a model when get_model or ensure_model
can express the requirement.

Inspect capabilities rather than assuming optional functionality.

Treat Pumas result status as authoritative.

Do not infer readiness merely because a download request was accepted.

Do not depend on conversational memory for session or caller identity.
```

---

## 18. Agent-facing operations should map directly to the domain contract

Avoid implementing agent tools as unrelated wrapper logic.

For example:

```text
MCP get_model
       │
       ▼
PumasIntentApi::get_model(...)
```

rather than:

```text
MCP get_model
       │
       ├── search Hugging Face
       ├── inspect filesystem
       ├── start downloader
       └── manually assemble response
```

The MCP layer should remain thin.

Any behavior required by both applications and agents belongs below the MCP boundary.

---

## 19. Suggested first-version MCP surface

A practical initial tool set could be:

```text
get_capabilities

find_model
get_model
ensure_model
get_model_status
inspect_model

list_models
list_libraries
inspect_library
```

If distributed Pumas work is available:

```text
discover_nodes
inspect_node
find_model_locations
ensure_model_on_node
```

The first version should resist adding convenience operations until real agent workflows demonstrate that they are required.

---

## 20. Example agent workflow

A normal agent interaction might be:

```text
Agent:
"I need a local text-generation model that fits these constraints."

        │
        ▼

get_capabilities()

        │
        ▼

find_model({
    purpose: text-generation,
    max_vram: ...
})

        │
        ▼

Pumas returns candidates

        │
        ▼

get_model(requirement)

        │
        ├── Ready
        │      └── return model
        │
        └── Pending
               └── acquisition continues

        ...

MCP task/resource notification:
    model ready

        │
        ▼

Agent/host may continue if supported
```

At no point does the agent need to know:

```text
download URL
temporary directory
database schema
reconciliation implementation
library filesystem layout
session ID
```

---

## Target architecture

```text
                         AGENT
                           │
                           ▼
                    Pumas Agent Skill
                           │
                           ▼
                        MCP
                           │
                    MCP Adapter Layer
                           │
                 deterministic caller context
                           │
                           ▼
                  Pumas Intent API
                           │
            ┌──────────────┴──────────────┐
            ▼                             ▼
        Resolver                      Reconciler
            │                             │
            └──────────────┬──────────────┘
                           ▼
                       Pumas Core
                           │
                  Durable Model State
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
   Local Library       Peer/Fleet          Upstream
```

For asynchronous interaction:

```text
                Pumas Durable State
                       │
          ┌────────────┴────────────┐
          ▼                         ▼
      MCP Tasks               MCP Resources
          │                         │
     progress/state            state updates
          └────────────┬────────────┘
                       ▼
                   MCP Client
                       │
                       ▼
                Agent Host Policy
```

---

## Core design principle

The agent layer should follow this rule:

> **The agent expresses domain intent. Pumas owns model resolution, acquisition, reconciliation, and durable state. MCP exposes that state to agents through structured tools, asynchronous tasks, and subscriptions without making conversational memory part of system correctness.**

A secondary principle is equally important:

> **Do not add agent-specific state management unless the Pumas domain itself requires it. Stable resource identity and deterministic transport-managed caller context should be preferred over LLM-managed sessions or workflow handles.**

This brief should be used as architectural input for a development plan. The plan should map the proposed agent layer onto the new intent API, existing Pumas RPC/core boundaries, model lifecycle state, reconciliation machinery, discovery architecture, and future distributed-node protocol before selecting concrete MCP tools or implementation milestones.
