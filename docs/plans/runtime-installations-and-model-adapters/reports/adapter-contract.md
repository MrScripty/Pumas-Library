# Adapter definition, implementation and lifecycle

This is the proposed semantic contract, not a finalized wire schema or implemented API. Public names/signatures must be integrated with the current contract owner and generated consumers in R2. The division of responsibilities is binding for this revision; spelling and private module layout are not.

## What an adapter means

An adapter supplies the missing behavior between a model artifact and an execution host. Dependencies describe the software/native/artifact conditions that behavior needs. Environment management satisfies those requirements. Neither a package list nor an import alone describes tokenization, component assembly, specialized model construction, generation invocation or result conversion.

Three supported model cases:

| Case | Representation | New executable adapter required? |
| --- | --- | --- |
| Standard supported artifact and task | Existing standard loader selected from package facts | No |
| Standard loader works after additional dependencies/options | Existing loader plus authored supplements and validated options | No |
| Specialized loader or execution behavior required | Registered adapter implementation with its package and requirements | Yes, when no existing adapter implements that behavior |

A model can reference a preferred adapter revision without embedding the implementation or copying its dependencies into model metadata. An adapter can apply to a family of models. A single compatible installation can support multiple adapters. Incompatible dependency closures normally require separate immutable installations and processes; registration itself does not require one environment per adapter.

## Small definition shape

The owning definition must establish these meanings:

- Identity: stable namespaced adapter ID, immutable revision/content identity, origin and supported adapter-host API range/version.
- Applicability: supported task kinds, runtime family/capabilities, package architecture/format/quantization evidence and explicit artifact/component roles. Repository names may supply provenance, not replace compatibility.
- Construction: either an existing loader ID plus validated options or a pinned distribution/entry point with integrity/provenance. Requirements for a distribution come from its package metadata; adapter-native/model-role constraints remain separately owned.
- Configuration: versioned accepted options and a supported validation representation. The generic UI can expose basic schema-defined options; specialized optional controls do not redefine compatibility.
- Operational behavior: task interfaces implemented, resource/device restrictions, probe entry point where needed, and the documented cleanup/cancellation behavior of the host interface.

This is not a universal manifest for UI layout, installation catalog state, hardware inventory, profile defaults, mutable status or all Pumas policy. Those independent authorities remain referenced, not absorbed.

## Python package mechanism

Prefer conventional Python distribution metadata/entry points for executable registrations. PyPA defines installed entry-point groups/names/object references and leaves collision behavior to the consuming application. Python's `importlib.metadata` exposes metadata separately from `EntryPoint.load()`. See the prior-reviewed [PyPA entry-point specification](https://packaging.python.org/en/latest/specifications/entry-points/) and [Python metadata API](https://docs.python.org/3/library/importlib.metadata.html); the [common source inventory](../../artifact-acquisition/reports/standards-and-sources.md) records the review limits.

Pumas must still own the semantics that packaging does not supply: authorization, schema/host version checks, adapter ID collision policy, immutable revision selection, environment scope, admission, lifetime and readiness.

Inspect a package definition/metadata from its reviewed artifact without importing it. Retain the verified definition reference so requirements can be shown before installation. At installation, verify definition identity and implementation entry point against the exact artifact installed. At runtime, discover only within the admitted installation and only load its selected approved entry point. Do not resolve an entry point by name from an ambient or unrelated environment.

Externally registered Python adapter code is not imported into the core-library or Electron/renderer processes. Python code is loaded in the selected owned Python host process. A native runtime with no Python adapter uses its registered runtime integration and supported native loading protocol instead of launching a Python sidecar for symmetry.

## Proposed operations and outcomes

Use intent-level operations whose names align with existing API conventions:

| Operation | Required behavior |
| --- | --- |
| Register definition | Validate schema, IDs, immutable revision and references; record explicit authority; no model load or implicit package installation. Return registered/already-registered/conflict/invalid/unsupported/unavailable distinctly. |
| Inspect/list definitions | Metadata-only and paginated; include enabled/disabled/invalid/unavailable entries and origin. Availability is a separate installation/process observation. |
| Resolve execution | For exact model/task/profile and optional adapter preference, return the selected definition and missing requirements or compatible installation choices. No hidden installation, binding change or fallback to a different model. |
| Install requirements | Explicit approved resolution/publication; preserve per-requirement provenance and terminal outcomes. Do not mutate a busy environment. |
| Bind and launch | Persist chosen installation/profile relationship; launch or validate exact current process binding with retained use custody. |
| Load/execute/unload | Use admitted adapter revision, targets/options, generation and slot/session. Return task-typed results; preserve cleanup obligations even if caller disconnects. |
| Disable/unregister/update | Affect future admissions as documented; retain references for current sessions. New revision is separate content. Physical deletion waits for references/custody. |

Do not expose a new public planning-session protocol just to coordinate internal calls. A service can carry an immutable admitted value within one operation.

## Selection without accidental winners

Explicit compatible adapter/revision selection wins only after validation. A declared model preference may narrow candidates. Without an explicit choice, select only when an owned deterministic policy establishes a unique suitable candidate; otherwise return ambiguity and the concrete choices. Do not run arbitrary candidate code to discover an adapter during ordinary library listing. Optional trusted probes are a separate execution operation in the selected host.

One adapter reporting unsupported is not permission to swap models, enable remote code, relax pins or start a different provider. Extra protocol capabilities are additive where the contract permits; an unknown operation/host API is explicitly unsupported.

## Lifecycle and invalidation

Keep facts separate rather than one `available` boolean:

1. Definition registered and enabled at a catalog revision.
2. Requirements resolved/compatible for a selected installation.
3. Implementation installed and host/API compatible.
4. Current process has passed relevant adapter preflight.
5. Exact selected model has loaded in a pinned session.
6. A specific task request completed and its result passed the destination contract.

Model metadata/component changes invalidate the model decision. Adapter revision changes invalidate new admissions referencing its old selection policy but do not rewrite active sessions. Environment mutation/recreation invalidates its prior fingerprint evidence. Driver/device observations have their own scope; a driver change is not a new package identity. Bundle/API changes follow their own compatibility rules.

A live generation pins its actual implementation and resources. Disable closes new admission. Ordinary unregister does not uninstall packages under a running process. Immediate revocation requires explicit stop authority and observed terminal cleanup; it cannot claim cleanup merely because a timer expired. Normal failures in a plugin are contained as adapter/session failures; native crashes affect their process and are not described as isolated merely because imports were lazy. Process separation for crash containment is an explicit policy, not a sandbox claim.

## Security and customization

Trust approval belongs to the operator or configured policy, not a centrally compiled list of permitted adapters. Supporting an open adapter population must not require a Pumas maintainer to approve each adapter ID or release new source.

Custom Python code is supported when it is explicitly approved and bound to a pinned package or supported immutable local artifact. A directory/script editable in place must be snapshotted/verified under a declared development mechanism before managed production admission; live mutable source cannot inherit immutable-package guarantees.

Existing loaders' blanket `trust_remote_code=True` must not become the new default. Code trust applies to model-provided files, adapter packages, compiled extensions and install/build steps. Dependency solving and metadata listing must not execute unapproved setup/import code.

## Required extension test

Build Pumas and its host once, record that candidate, then add a separately distributed compatible adapter definition/package that was not enumerated in source. Register it, inspect missing dependencies without import, explicitly install into an isolated managed environment, bind/start, load a suitable model and execute a supported operation through the public consumer. Verify no Pumas source, enum, gateway allowlist, sidecar dispatch list, frontend type union or core bundle file list was changed to add that adapter.

Repeat with two definition revisions, conflicting packages requiring separate environments, duplicate IDs, missing dependencies, disabled/removed definitions, host API mismatch and one failing import. Standard Z-Image in R4 is a real independently shipped extension test, not only a synthetic fixture.

## Acquisition prerequisite

Adapter metadata and implementation artifacts use the [shared acquisition contract](../../../contracts/artifact-acquisition.md). Source resolvers and model-execution adapters are different extension points. The acquisition plan owns byte access, verification and input lifetime; runtime owns requirement/code approval, package installation, catalog revisions and execution. R2 requires AQ-PACKAGES and R1; S3 retrieval additionally requires AQ-S3. A missing source gate is not permission to add an adapter-specific downloader.
