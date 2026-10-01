# Coding-Standards MCP usability feedback

This report records agent experience using the Coding-Standards MCP during Acquisition Q1 preparation. It is separate from acquisition acceptance: MCP routing and usability do not establish product-code compliance, and product tests do not establish that the MCP workflow was usable.

## Primary integrator

- **Useful calls:** `routing_facts` exposed the valid fact categories and values; an initial route with minimal facts made the router's information model clear. Compact routes with bounded content returned an explicit snapshot, selected standards, and unresolved-fact count. `read_many` let the integrator inspect the commit, planning, implementation, verification, architecture, contract, and documentation obligations. Rerouting after composition seams were clarified confirmed applicability for the actual Q1 boundary.
- **Confusing or redundant steps:** Tool discovery descriptions were verbose. The first broad route requested 32 policy contents and produced output large enough to truncate. A broad `read_many` similarly returned tens of thousands of tokens. A later route displayed policies already read, so applicability and policy reading were not easy to distinguish.
- **Additional final-route experience:** The summary carried the snapshot UUID without its required `snapshot:v1:` prefix; the MCP rejected that exact handle with a clear pattern error, after which the full opaque handle worked. A complete route still required explicit empty values for the framework and workflow-profile categories. Supplying `content.limit: 32` returned all 32 policy bodies at once (about 85,000 output tokens and truncated); a targeted `read_many` of 11 exact selected policies was more useful for checking this candidate.
- **Missing context:** The MCP did not know the repository's current accepted plan status, retained-state population, public consumers, active recovery owner, or actual CI/source evidence. Those facts had to be gathered from the repository, GitHub, and the source tree. The route cannot certify that the source or tests satisfy the selected rules.
- **Smallest sufficient workflow:** Read routing facts once; route the concrete slice without policy contents; read Core plus only the selected standards and workflows relevant to its actual changed boundary in small pages; inspect source and run the required evidence; reroute only when ownership or scope changes; then use the final review operation against the implemented candidate.
- **Recommendations:** Keep route output compact by default and separate applicability reasons from policy bodies. Make content pages token-budgeted, expose previously read policy IDs, and put unresolved facts first. Clarify in the result that a complete route means applicability was resolved, not that implementation compliance was verified. A same-snapshot “changed design” review summary could call out new or removed obligations without replaying all policy text.
- **Smallest sufficient workflow for this slice:** Preserve the complete opaque snapshot handle; call `routing_facts` only when the current fact vocabulary is not already known; route the changed boundary without inline policy text; read the exact selected obligations in a small `read_many`; inspect source and test evidence outside the MCP; reroute the final implementation. Empty framework/profile answers are needed to close applicability when those categories are not relevant.

## Independent architecture reviewer

- **Useful calls:** `routing_facts`, a route including content, and the pinned snapshot made it possible to identify and read Architecture, Rust Async, Persistence, Contract Evolution, Core, and Planning. The final route selected 24 standards with zero unresolved categories; this was a complete applicability result, not a compliance certificate.
- **Confusing or redundant steps:** Initial tool discovery was verbose. The broad route returned roughly 68,000 tokens and truncated, repeated relationship rationales, and later policy reads repeated content already shown inline.
- **Missing context:** The MCP could not reveal the repository's persisted-state population, source/consumer ownership, accepted plan prerequisites, or shutdown composition. The reviewer had to inspect source and plan documents outside the MCP to assess those obligations.
- **Smallest sufficient workflow:** Route concrete facts once, omit inline policy content, read only the relevant policies in small pages, inspect source/evidence independently, and reroute when the design materially changes.
- **Recommendations:** Add an output-size budget, avoid repeating policy prose and relationship explanations, show which policies have already been read, and provide a same-facts design-change summary. Keep route completeness distinct from source/evidence compliance.

## Primary integrator — source API boundary follow-up

- **Useful calls:** I refreshed `routing_facts`, routed the changed public API/async/network boundary, and got a complete route with 34 selected units and zero unresolved categories. Compact single-policy reads provided the specific compatibility and lifecycle rules. The route helped identify that public API compatibility and typed failure behavior mattered even though the source adapter was internal to the repository.
- **Confusing or redundant steps:** The route result still repeated long relationship rationales for each policy. A `read_many` call for nine compact policies returned enough combined policy text to exceed a practical review/output window; focused individual reads were easier to inspect, though several were still large. The route's completeness and the amount of normative text read remained separate facts that needed explicit tracking.
- **Missing context:** The MCP did not know that `GitHubAsset` is publicly re-exported, nor whether downstream crate consumers construct it directly. I had to inspect `network/mod.rs`, search the Rust tree, and inspect Cargo packaging before rejecting a breaking public-field addition. It also could not observe whether the resolver was consumed by the native installer or what current tests had run.
- **Smallest sufficient workflow:** Use current routing facts; route concrete code and contract facts without inline policy bodies; read Core and the exact contract-evolution, Rust API/async, persistence, security, resilience, and verification units needed for the slice in a bounded set; inspect source and run tests separately; re-route when the public boundary/design changes; then check the final source against the selected obligations.
- **Recommendations:** Add a strict token/byte budget to `read_many`; support section-scoped reads with a concise obligation summary and authoritative full-text links; retain a visible list of already-read policy IDs across route continuations; shorten repeated relationship rationales; and state that current consumer inventory and source evidence must be established outside the MCP. Preserve the distinction between a complete route and implementation compliance.

## Native-custody contributor — GPT-6.1 Sol High

- **Useful calls:** `routing_facts`, a complete `route`, and snapshot-bound `read_many` selected and returned the Core, Rust async, concurrency, persistence, contracts, and commit obligations. The first route selected 27 standards; recognizing the metadata-publication boundary required a second route, which selected 29 with no unresolved facts. These calls selected guidance; the contributor used code inspection and tests for implementation evidence.
- **Confusing or redundant steps:** Route content-pagination repeated long policy-selection rationale. Large policy results combined with source diffs exceeded a useful output window; smaller content-only reads worked better. Broad tool discovery included unrelated authoring operations.
- **Missing context:** The MCP did not know the admitted worktree, current plan/write set, actual publication semantics, source ownership, or test environment. The contributor had to inspect source to recognize the metadata boundary even though persistence files were outside its write set.
- **Where the contributor left MCP:** Git state, plan details, source ownership, filesystem behavior, Cargo checks, and fixture outputs all came from repository tools, not the MCP.
- **Smallest sufficient workflow:** One `routing_facts` call, one complete-facts route, then snapshot-bound reads of only the applicable policies; use repository source and tests to assess actual compliance. Standards authoring/revision/publication calls were unnecessary.
- **Recommendations:** Provide continuations that return policy content without repeating route rationales; make verbose relationship detail opt-in; separate implementation-guidance discovery from standards-authoring tools; and make the route surface a concise checklist for indirect boundaries such as durable metadata publication. Retain the explicit distinction between routed guidance and implementation review.

## Native-custody independent reviewer — GPT-6.1 Sol High

- **Useful calls:** `describe_input(route)` clarified the input contract; `routing_facts` exposed the valid fact names and values; a complete route plus snapshot-bound `read_many` selected 27 policies with no unresolved questions. Concurrency, Rust async, persistence, resilience, and verification policies supported the source review.
- **Confusing or redundant steps:** Tool discovery was verbose. The reviewer first searched for a server named Coding-Standards, which was not configured, and initially missed the `standards_engine` tool namespace in `ALL_TOOLS`. `describe_input(route)` added little beyond the exposed declaration. Reading all 27 policies at once produced excessive output and repeated policy-selection rationale.
- **Missing context:** The MCP did not bind the route to the branch, diff, baseline, plan gate, source population, or test evidence. The reviewer had to inspect Git, the plan, the worker diff, Tokio/tempfile behavior, and source to identify shutdown, metadata coordination, pending filesystem I/O, and cleanup-error obligations.
- **Where the reviewer left MCP:** Repository, dependency-source, and test inspection were required for every product finding. No tests were run during the read-only review.
- **Smallest sufficient workflow:** Discover `standards_engine` tools from the provided callable catalog; use `routing_facts`, one complete route with explicit empty categories, then read only focused applicable policies in a bounded page; evaluate the actual diff and evidence in repository tools. `describe_input` is optional when the declared shape is already clear.
- **Recommendations:** Make the tool namespace easier to discover by linking it from the configured MCP catalog; include accepted base and plan/write-set context when the caller supplies it, while clearly distinguishing context from policy authority; offer a focused policy-text continuation without relationship rationale; and provide a concise read-only code-review workflow separate from standards-corpus authoring. Do not imply that a complete route certifies implementation.

## Shared-owner design reviewer — GPT-6.1 Sol High

- **Useful calls:** The reviewer initially searched for a Coding-Standards server and did not find one; after I pointed it to the exposed `standards_engine` tool namespace, it called `routing_facts`, routed concrete planning/verification/persistence/Rust facts to a complete result with zero unresolved questions, and read Architecture, replay, Persistence, Contract Evolution, Code Design, and Rust Async. The pinned snapshot was `snapshot:v1:78677912-3ddc-4107-83a3-c7f83542de1d`.
- **Confusing or redundant steps:** Searching by the product name led to an incorrect `list_mcp_resources` call against a nonexistent server. Tool-callable discovery and MCP-server/resource discovery were not clearly distinguished. Correcting the search resolved the confusion.
- **Missing context:** MCP output identified its interface version and unreviewed exposure but did not provide the current Q1 source ownership, retained-store facts, or plan gate by itself; the reviewer is collecting those from repository files.
- **Smallest sufficient workflow:** Discover callable `standards_engine` methods from `ALL_TOOLS`, call `routing_facts`, complete a slice-specific route, then read the few selected policies required for architecture/persistence/concurrency. Repository inspection remains the source of implementation facts.
- **Recommendations:** Surface configured MCP tool namespaces in initial agent context, distinguish callable tools from MCP servers/resources, and make the bounded read-only workflow discoverable without exposing unrelated authoring operations first.

## Primary integrator — shared-owner extraction preparation

- **Useful calls:** For the new multi-root custody seam, I refreshed `routing_facts`, then routed the launcher/library, persistence/IPC, Rust API/async/cross-platform/dependency/security, concurrency, contracts/evolution, resilience, replay, and verification facts. The route completed with no unresolved facts under snapshot `snapshot:v1:ea1dc4cb-2d14-4a69-866c-ded1b097e278`. Focused snapshot-bound reads of Persistence, Concurrency, Contract Evolution, Architecture, Rust Async, Rust API, Verification, Commit, Rust Cross-Platform, Rust Dependencies, Security, Untrusted Execution, Resilience, Protocols, Schemas, Replay, and Code Design identified the relevant ownership and evidence obligations.
- **Confusing or redundant steps:** The route requested eight policy bodies and produced roughly 63,000 output tokens before truncation, even though only the applicability result and snapshot were needed. `read_many` returns full policy content without a section selector, so I filtered the structured result by headings and relevant sections. Repeating route relationship metadata alongside policy text remained a major output cost.
- **Missing context:** The MCP did not know the existing owner retained only one physical-root grant, that independently opened handles can refer to one root, or that model and native consumers need separate physical roots. Repository inspection and the shared-owner source review established those facts. It also could not report the active PR check because GitHub access failed in the current environment.
- **Where I left MCP:** Root identity semantics, async grant acquisition, the actual two-root test, Rust verification, repository state, and current CI availability were established with source and repository tools. The standards route selected obligations only; the code/test evidence is separate.
- **Smallest sufficient workflow:** Reuse the current fact vocabulary when available; call `routing_facts` only to refresh it; route without inline policy bodies; read a small set of exact selected policies in bounded calls; inspect source and run tests outside the MCP; reroute the final candidate if the owner boundary changes.
- **Recommendations:** Make route content opt-in and strictly byte-budgeted; offer section-scoped policy reads; report which policies were already read; include repository-supplied owner and candidate context as non-authoritative context; and show a compact delta when a slice adds or removes an ownership boundary.

## Native-custody contributor — revised lifecycle review

- **Useful calls:** A new `routing_facts`/`route` cycle selected 28 standards with no unresolved facts under snapshot `snapshot:v1:afd3e35d-bc08-4dc8-b848-597626ae279c`. The contributor reread concurrency, Rust async, persistence, and resilience against the repaired native-custody proposal and then executed source-backed lifecycle fixtures and Cargo checks.
- **Confusing or redundant steps:** Route results again carried substantial edge/handle metadata. Batched full policy content exceeded a useful output window; smaller content-only reads were easier to apply.
- **Missing context:** The MCP still did not identify the exact `VersionState` lifecycle owner or how shutdown observed metadata workers; those were discovered in repository source and through independent review.
- **Where the contributor left MCP:** All filesystem, task-registration, test, and build evidence came from the isolated worktree. The revised route did not establish AQ-HTTP or certify the native implementation.
- **Smallest sufficient workflow:** Refresh facts, route the precise changed boundary, reread only changed obligations, then inspect and test the exact candidate.
- **Recommendations:** Add a compact route with no repeated relationship graph, preserve a visible list of already-read obligations, and present changed-boundary implications without repeating unchanged policy text.

## Native-custody contributor — canceled-worker ownership follow-up

- **Useful calls:** The contributor reused its previously routed snapshot and reread Concurrency and Rust Async after the independent P2. That narrowed the design requirement from “retain the lock” to “retain and observe worker completion through shutdown.” The actual registry implementation and canceled-waiter completion/failure/panic fixtures were verified in the worker worktree.
- **Confusing or redundant steps:** No new discovery or broad reroute was needed. Policy text still lacked source-specific task-owner and state-construction context.
- **Missing context:** The MCP did not identify the existing `VersionState`/`InstallationTasks` owners or manager shutdown order; source inspection established how to register and drain the new mutation workers.
- **Where the contributor left MCP:** The worker registry, manager drain, repeated shutdown result, and tests were all assessed from source and exact-candidate Cargo runs. The MCP supplied obligations only.
- **Smallest sufficient workflow:** Reuse the prior route, reread only concurrency/async obligations implicated by the finding, then implement and test the source lifecycle.
- **Recommendations:** Keep exact prior-read and snapshot context visible, offer section-level policy output, and provide concrete owner/cancellation prompts without pretending the MCP knows repository lifecycle wiring.

## Native-custody independent reviewer — revised candidate follow-up

- **Useful calls:** The reviewer reused the prior registered facts and snapshot, routed the revised metadata/task-lifecycle scope, and read only Concurrency, Persistence, and Rust Async. That focused read directly exposed the missing terminal observer for canceled metadata workers.
- **Confusing or redundant steps:** Route rationale still repeated relation/handle metadata. Focused output improved on the earlier all-policy batch, but exact diff/base/gate facts still had to be supplied and verified externally.
- **Missing context:** MCP did not bind the review to the candidate digest or show that the new workers were omitted from `shutdown_installations`; source and test inspection were necessary.
- **Where the reviewer left MCP:** The P2 finding, exact source locations, changed-diff digest, and review status came from read-only Git/source inspection. No tests were run by this review.
- **Smallest sufficient workflow:** Reuse retained routing facts, route the actual change, read the three relevant policies, inspect source/diff and existing evidence, then report findings independently.
- **Recommendations:** Offer a narrow source-review mode that accepts an exact candidate reference as context, links only relevant policies, and says plainly that tools neither inspected the diff nor approved standards compliance. Keep standards-corpus authoring operations out of the default review entry point.

## Native-custody independent reviewer — shutdown ownership repair verification

- **Useful calls:** The reviewer used a snapshot-bound `route` plus a focused `read_many` of Rust Async and Concurrency. These rules directly separated retaining a metadata lock from retaining a completion observer and shutdown drain.
- **Confusing or redundant steps:** No new fact discovery was needed; the route and two policy reads were sufficient. Source inspection was still needed to confirm the lock, result channel, supervisor receipt, and shutdown guard ordering.
- **Missing context:** The MCP did not reveal the public OllamaVersionManager wrapper or its lack of a drain method; repository search did. No in-repository production caller was found, but the public wrapper remains a supported ownership surface unless its API contract says otherwise.
- **Where the reviewer left MCP:** Exact diff identity, the closed P2 disposition, fixture structure, and remaining wrapper limit came from read-only source/Git inspection. No tests were run by the reviewer.
- **Smallest sufficient workflow:** Reuse retained route facts, route the focused shutdown change, read Rust Async and Concurrency, then inspect the exact diff and tests.
- **Recommendations:** Keep the minimal policy-only workflow easy to repeat and make source review context explicit. A concise owner inventory would help identify wrappers that must delegate to a newly introduced lifecycle owner.

## Native-custody contributor — public Ollama wrapper drain

- **Useful calls:** The contributor reused its retained standards snapshot, routed the wrapper lifecycle boundary to 26 standards with zero unresolved categories, then read Rust Async, Concurrency and Rust API. Those obligations clarified that the public wrapper also had to close admission and retain completion receipts for its owned `VersionState` mutations.
- **Confusing or redundant steps:** No new broad discovery was necessary. The route still carried relation metadata; scoped reads were sufficient.
- **Missing context:** The MCP did not identify that `OllamaVersionManager` owns private state and runs removal/download activity inline. Repository source established the activity and shutdown order.
- **Where the contributor left MCP:** The wrapper receipt, cancellation behavior, file-write settlement, regression tests and exact-candidate Cargo results came from source inspection and execution in the worker worktree. The MCP supplied obligations only.
- **Smallest sufficient workflow:** Reuse facts and snapshot, route the newly discovered owner boundary, read async/concurrency/API, then inspect and test the exact candidate.
- **Recommendations:** Show the exact affected owner boundary and prior relevant reads in a compact view; offer section-scoped policy reads. Keep repository evidence explicitly outside the MCP compliance claim.

## Native-custody independent reviewer — final wrapper candidate

- **Useful calls:** The reviewer reused registered facts, routed the final wrapper scope to 27 standards with no unresolved questions, and read Concurrency and Rust Async. This was sufficient to assess close-admission ordering, completion ownership after waiter cancellation, and shutdown draining.
- **Confusing or redundant steps:** No extra discovery call was needed. Source inspection remained necessary to check the actual wrapper receipt and queued-install cancellation reset.
- **Missing context:** The MCP did not supply the candidate digest, activity registry, or exact call graph; repository/Git inspection did.
- **Where the reviewer left MCP:** Findings, exact source evidence, tests inspected, and the disposition of the previous wrapper limitation came from read-only source/diff review. The reviewer ran no tests.
- **Smallest sufficient workflow:** Reuse the prior route, read the two lifecycle policies, and inspect the exact diff and named regression tests.
- **Recommendations:** Make candidate identity and affected owner paths easy to provide as non-authoritative review context. Keep it clear that policy routing does not inspect or approve source.

## Q1 store-cutover reviewer — source-neutral lifecycle design

- **Useful calls:** The reviewer discovered routing facts, routed the Q1 persistence/concurrency/contract/async/replay/architecture/verification slice, and read focused selected policies. The route completed with 28 standards and no unresolved questions. It helped expose the single-writer, supported-state, no-assumed-compatibility, supervised-effect, and claim-scoped-evidence requirements.
- **Confusing or redundant steps:** Seven whole-policy reads produced excessive output and truncation; the reviewer had to retrieve obscured concurrency/async/replay bodies separately. Text and structured forms repeated content.
- **Missing context:** The MCP did not know the existing v4-to-v5 migration-on-load behavior, model-library marker semantics, hidden custody inventory, mutation-authority reader, or composition boundaries. Repository inspection supplied these facts.
- **Where the reviewer left MCP:** Exact migration populations, unknown deployed-root scope, consumer ownership and proposed cutover were established from source and plan inspection. No files were edited and no tests were run.
- **Smallest sufficient workflow:** One `routing_facts`, one concise `route`, one focused `read_many`; schema discovery only when the tool contract actually requires it.
- **Recommendations:** Keep route output free of repeated relation graphs; support section-scoped policy reads; distinguish applicability from repository compliance; avoid duplicate policy text in structured and textual results; provide examples for selecting routing facts.

## Primary integrator — merged native-custody candidate and shared-owner boundary

- **Useful calls:** For the merged native custody scope, I refreshed `routing_facts`, routed 32 standards with no unresolved categories under snapshot `snapshot:v1:37f8f667-71b0-4580-9084-4d202fdcc04c`, and read focused Architecture, Concurrency, Rust Async, Rust API, Persistence, Contract Evolution, Resilience, Verification and Commit standards. The final route captured the actual app-manager/RPC, async shutdown, persistence-adjacent and verification boundaries. Reading Concurrency/Async and Architecture again after lifecycle review exposed that one supervisor needs consumer-scoped handles rather than sharing the current HF owner unchanged.
- **Confusing or redundant steps:** The route summary is compact only without policy bodies; focused `read_many` still returns whole policy text and repeated handles/relationship context. The first larger batch truncated output, so I split further review into smaller groups.
- **Missing context:** The MCP did not know the current owner scans global task IDs/projections or that HF Drop supplies the owner-wide shutdown callback. Repository source and the exact reviewer identified that coupling. It also did not know that the downloaded model root initializer writes a model library marker.
- **Where I left MCP:** Candidate identities, Git ancestry, staged scope, source ownership, tests, feature checks, and CI state were verified with repository and GitHub tools. MCP routing did not inspect source or supply acceptance evidence.
- **Smallest sufficient workflow:** Discover or reuse the routing facts; route without inline policy content; read only Architecture, Concurrency, Rust Async/API, Persistence/Evolution, Verification, Resilience and Commit obligations implicated by the actual boundary; inspect source and evidence; run a final route on the completed candidate.
- **Recommendations:** Support policy-section reads and compact route deltas; keep candidate/source context clearly marked non-authoritative; expose the already-read standards list; and state explicitly that MCP applicability is neither source inspection nor compliance evidence.

### Primary integrator — shared-custody slice routing

- **Useful calls:** A fresh `routing_facts` snapshot and a concise route selected 34 applicable standards with zero unresolved facts after marking unrelated framework routing known-absent. The selected closure covered Rust core/API/async, acquisition ownership, concurrency, security/trust, persistence-adjacent behavior, verification and commit workflow.
- **Confusing or redundant steps:** Omitting the unrelated `routing.frameworks` fact left it as a required unresolved category; the router needs an explicit known-absent marker. A single `read_many` across 17 selected policies produced a very large repeated result and was truncated by the client, so future reads need smaller batches. Relationship and policy metadata is repeated around full policy text.
- **Missing context:** The MCP did not identify that the current HF supervisor globally scans operation IDs/projections, that HF shutdown closes the whole owner, or that `DownloadDestinationRoot` creates model-specific marker/deletion authority. Source and reviewer inspection supplied those ownership facts.
- **Where I left MCP:** Git ancestry, exact write sets, existing task APIs, migration-on-load behavior, current CI, and all executed test evidence come from Git, repository source, and command results. The route supplies obligations only.
- **Smallest sufficient workflow:** Declare every routing fact, route the actual slice, read focused Architecture, Concurrency, Rust Async/API, Persistence/Evolution, Security, Verification and Commit policies, then inspect/test the exact implementation and route it again before handoff.
- **Recommendations:** Treat omitted facts as unknown only when the fact is genuinely unknown; make known-absent explicit in concise examples. Support policy-section reads and compact incremental route output, and avoid repeating relation graphs in every result. Preserve a clear boundary between applicable standards and source compliance evidence.

## Participation

### Primary integrator — current Q1 shared-store routing

- **Useful calls:** A fresh `routing_facts` snapshot plus a complete route selected 31 standards with zero unresolved categories for the durable store/service boundary. Focused reads returned Persistence, Replay, Concurrency, Contract Evolution, Schemas, Untrusted Execution, Independent Test Oracles, and Platform Verification. Those standards make supported retained-state facts, one-writer semantics, effect cessation before release/publication, version scope, delegated authority and claim-matched evidence explicit.
- **Confusing or redundant steps:** An attempted selection-only call with `content.limit: 0` was rejected because the input contract requires a minimum of one; omitting `content` produced the desired compact route. A single eight-policy `read_many` returned duplicated content/metadata and exceeded the visible output window; the read was repeated in two focused batches.
- **Missing context:** The MCP did not expose the exact `c3052583` candidate, plan gate state, schema-5 hidden custody populations, v4-on-load mutation, model deletion reader, RPC drain order, or local CI state. Repository and GitHub inspection supplied those facts.
- **Where I left MCP:** Candidate identity, supported-state source inventory, tests, current Actions run, and the exact lifecycle call graph came from Git, repository source and GitHub. The MCP supplied obligations only.
- **Smallest sufficient workflow:** Capture facts, route without inline policy content, read two or three relevant policies at a time, inspect the exact source and evidence, then reroute when the implementation boundary changes.
- **Recommendations:** Make route selection-only the obvious default, reject or normalize a zero content limit with a direct hint, return policy bodies once without repeating them in another result representation, and support section-scoped reads with a visible already-read list.

### Independent Q1 next-slice architecture investigator — GPT-6.1 Sol High

- **Useful calls:** One `routing_facts`, one snapshot-bound `route`, and three focused `read_many` calls selected 28 standards with zero unresolved categories. Reads covered architecture/replay, persistence, contract evolution, concurrency, Rust async/API, security/trust, resilience, verification and planning. The route helped surface the one-writer, complete migration preconditions, consumer-specific proof, nested-effect drainage, and scoped-evidence obligations.
- **Confusing or redundant steps:** Broad discovery by “coding/standards/policy” returned unrelated tool descriptions. A route containing several whole policy bodies and an initial six-policy read exceeded the response window; the agent had to re-extract individual bodies. Relationship rationale and continuation metadata repeated around policy content.
- **Missing context:** The MCP did not know the source call graph, `DownloadPersistence` schema 5 and its v4 migration-on-load behavior, hidden admission/quarantine custody, the mutation-authority reader, library marker semantics, public constructors, or shutdown order. Those came from source, plan and contract inspection.
- **Where the agent left MCP:** Exact candidate/base identity, native asset selection semantics, state populations encoded by the source, and the proposed consumer/store composition were established with Git and repository inspection. The investigator edited nothing and ran no tests.
- **Smallest sufficient workflow:** Discover the namespace and fact vocabulary, route explicit facts without policy text, read a few selected standards in small batches, then inspect source and required evidence independently.
- **Recommendations:** Offer a read-only investigation entry point, compact route output with optional explanations, section-scoped policy reads, no duplicate body representations, and a clear indicator that routing does not inspect or certify application code.

### Q1 durable-store implementation admission follow-up — GPT-6.1 Sol High

- **Useful calls:** The worker refreshed routing facts, completed an implementation/verification/commit route with 26 selected policies and no unresolved categories, then read Persistence, Evolution, Replay, Commit, Concurrency, Rust Async, Security and Verification. That supplied the applicable obligations while source inspection located the migration-on-load and strict-schema behavior.
- **Confusing or redundant steps:** Whole-policy batches still exceeded a practical output window. The worker also reached an implementation boundary the MCP could not resolve: it routed migration obligations but did not select the exact operation/API or default builder behavior needed to satisfy them. The integrator had to make and record that product-contract decision before source changes could begin.
- **Missing context:** The MCP did not know the exact accepted schema versions, that v4 migration currently publishes during load, whether older processes might retain cached state, or that live retained roots are explicitly out of bounds. The worker established those facts from source and the plan, then paused for contract ownership.
- **Where the worker left MCP:** The exact schema version, migration authorization, API behavior, and production evidence scope were settled in the shared contract and plan by the integrator. The worker made no source edits and ran no tests before that decision.
- **Smallest sufficient workflow:** One current routing-facts snapshot, one complete route without inline content, focused reads of persistence/evolution/concurrency/async/security/verification/commit policies, then inspect the source and plan. A concrete source migration proposal still requires an owning human/agent to choose the API and rollout policy.
- **Recommendations:** Keep policy batches bounded; add a prompt that distinguishes normative obligations from design choices the caller must resolve; include a direct migration-cutover checklist for old readers/writers, default-open behavior, explicit authorization, and fixture versus deployed-state evidence. Continue to state that routing does not decide product semantics or certify code.

The primary integrator, independent architecture reviewer, native-custody contributor, native-custody independent reviewer, shared-owner design reviewer, and Q1 next-slice architecture investigator used the MCP and are represented above. The bounded source-inventory reviewer did not use it, so there is no MCP usability report from that agent.

The architecture reviewer’s narrow follow-up inspected the repaired current source without making a new MCP call and reused the prior routed obligations; its findings confirm the two identified integrity paths are closed at source level, not that Q1 is accepted. The MCP `review` operation is an authoring workflow for changes to the standards corpus; it does not review application source. Application compliance was checked by final routing, standards reads, source/test inspection, and the independent code review.

## Import-receipt feasibility investigator — GPT-6.1 Sol High

- **Useful calls:** Reused the existing routing facts and selected snapshot for a focused route over persistence, architecture/replay, contract evolution/schema, concurrency, Rust/API/async and verification concerns. Snapshot `snapshot:v1:869927cd-0134-4833-89c9-c545393f1dff` selected 21 standards with zero unresolved facts. Focused reads of Persistence and Architecture informed the separation between the model-owned completion proof and acquisition's exact lease/workspace custody. A final same-snapshot route confirmed the design scope.
- **Confusing or redundant steps:** No confusing or redundant operation was reported beyond long full policy text; source-specific questions were not answered by another MCP call.
- **Missing context:** The MCP did not expose the exact schema-6 partition writer, importer effects, held root grant, or orphan adoption call graph. Those required source inspection.
- **Where the investigator left MCP:** Candidate identity, schema decoder behavior, finalizer publication ordering, and concrete required tests were established in repository source, not by routing.
- **Smallest sufficient workflow:** Reuse the fact vocabulary/snapshot, route the exact slice, read two focused policies, reroute the final design, then verify the actual source boundaries and evidence separately.
- **Recommendations:** Offer section-scoped policy reads and make the distinction between a complete standards route and source/evidence qualification explicit.

## Primary integrator — initial combined receipt/guard routing, later split by source review

- **Useful calls:** I refreshed `routing_facts` at snapshot `snapshot:v1:af0c1e53-2199-4992-9b93-a00d526934fe`, then routed persistence, schema/evolution, replay, concurrency, delegated-execution security, test-oracle, verification, documentation and commit facts. The final same-snapshot route selected 32 standards with zero unresolved facts. Focused reads of Persistence, Contract Evolution, Architecture Replay, Concurrency, Untrusted Execution, Independent Test Oracles and Commit Workflow made conditional publication, exact replay authority, effect-boundary guards and candidate-matched tests explicit.
- **Confusing or redundant steps:** `ALL_TOOLS` keyword discovery returned long descriptions for both app-code routing/read operations and standards-authoring operations. The first route omitted the explicitly required empty framework category and returned `needs-facts`; correcting that on the same snapshot was straightforward. A 21-policy `read_many` produced more text than the visible output window, so I read the seven relevant policies in two smaller batches. Repeating the same complete route to inspect the selected count was unnecessary but provided a precise record.
- **Missing context:** The MCP did not know the current `d0b71b5` source boundary, schema-6 partition decoder, exact `Using` lease, importer side effects, metadata durability, mutation authority, or orphan-adoption entry points. Repository source and the separate exact-candidate design review are establishing those facts. Routing does not decide receipt schema/ownership or prove behavior.
- **Where I left MCP:** Git/PR/CI state, store and importer ownership, publication failure semantics, all adoption paths, and local test evidence came from repository/GitHub inspection. No standards-corpus authoring, attestation, or approval operation was performed.
- **Smallest sufficient workflow:** Capture the fact vocabulary once, route the exact library/persistence/lifecycle/security/verification/commit scope with every required category explicit, read three or four applicable policies at a time, inspect source and tests, and reroute the final candidate after implementation.
- **Recommendations:** Filter initial tool discovery to application-routing/read operations before exposing authoring tools; keep selection-only routing compact; surface required empty applicability facts before policy evaluation; cap `read_many` by a practical byte budget; show the selected count without another call; and provide section-scoped policy reads. Continue to distinguish a complete applicability route from implementation evidence and acceptance.

This is MCP usability feedback only. The Q1 product boundary and its acceptance state are recorded in the acquisition plan and execution ledger, not inferred from this report.

## Primary integrator — current importer/adoption custody-guard slice

- **Useful calls:** A fresh `routing_facts` snapshot (`snapshot:v1:9881b6be-4a0e-4d72-b0cd-4e4307cdc710`) and a complete route selected 26 standards with zero unresolved facts. Focused `read_many` covered Concurrency, Rust Async, Persistence, Rust API, Untrusted Execution, and Verification. This narrower route made effect-boundary ownership, cancellation retention, configured-authority refusal, and negative lifecycle evidence applicable without carrying receipt/output-proof policies into the current implementation slice.
- **Confusing or redundant steps:** The same empty `routing.frameworks` value had to be supplied explicitly despite no framework being used. The earlier combined route and broad 21-policy read remain separate historical discovery for receipt feasibility; neither is needed to implement this guard. The current focused reads were sufficient.
- **Missing context:** Routing did not identify the common importer shortcut, Diffusers delegation, HF use lease, already-held root grant, exact queue admission, or the unconfigured `ModelLibrary::new` path. Source inspection and a separate read-only architecture review supplied those facts.
- **Where I left MCP:** The precise private capability contents, caller cancellation behavior, and stale-scan race are grounded in Rust source and tests. The MCP supplied obligations, not the design choice or evidence.
- **Smallest sufficient workflow:** Refresh the facts once when the slice boundary changes, route the exact guard scope without inline policy text, read the six focused obligations in one bounded call, inspect source and lifecycle tests, then route the final implemented candidate.
- **Recommendations:** Make applicability-empty facts visible before route evaluation; retain the selection-only route and compact policy reads; allow a slice-oriented policy view that separates immediate obligations from later-slice closure; keep route completion distinct from source/test compliance.

This MCP usability report is separate from product acceptance. The guard was subsequently implemented on candidate `8b6c5f70c55a3d124189d0cd88dc85c780f47c84`; it remains under independent review and is not an accepted Q1 gate.

## Primary integrator — revised partial-HF-stage custody guard route

- **Useful calls:** After source inspection changed the guard boundary, I refreshed `routing_facts` and captured a new snapshot (`snapshot:v1:0ed9c25a-060e-4970-a5ca-a7286eda1601`). A complete route for the revised implementation scope selected 28 standards with zero unresolved fact categories. Focused reads included Independent Test Oracles, Code Design and Ownership, Commit Workflow, Verification, Rust, Architecture, and Security; earlier reads on the same guard work covered Concurrency, Rust Async, Persistence, Rust API, and Untrusted Execution. This surfaced the one-owner, exact-effect-boundary, cancellation-custody, typed-unavailable, and decision-fixture obligations relevant to the partial metadata stage.
- **Confusing or redundant steps:** The no-framework fact still needed an explicit `known-absent` value. A seven-policy `read_many` returned roughly 40,000 tokens of full policy text, exceeding the visible output window and truncating the result; compact reads do not provide a practical section-level summary for a narrow code slice. Routing the complete set was still useful, but rereading long whole policies was not.
- **Missing context:** The MCP did not identify the HF partial-stub call before weights, the fact it also indexes, or that index projection can rewrite metadata and custom runtime projections. Repository tracing established those concrete effects and showed that the design needed a separate exact `Transferring` stage capability.
- **Where I left MCP:** Source call chains, queue admission, stage timing, existing owner contexts, root grants, index side effects, tests, and Git/PR state were established from repository and service evidence. The MCP supplied applicable obligations; it did not choose or validate the code design.
- **Smallest sufficient workflow:** Refresh the fact vocabulary at the changed slice boundary; complete one exact route with all required categories; read only the handful of policy sections for ownership, persistence, async cancellation, delegated authority, and verification; inspect the source; then route the final implemented candidate.
- **Recommendations:** Add section-scoped obligation reads with an output-byte budget; show required empty applicability values before route evaluation; make the selected set and unresolved count visible in the initial response; and distinguish immediate slice obligations from downstream closure. Keep source and test evidence explicitly outside the route result.

This MCP usability feedback is separate from product acceptance. The revised partial-stage guard was implemented on candidate `8b6c5f70c55a3d124189d0cd88dc85c780f47c84`; implementation evidence does not certify its acceptance.

## Q1 importer custody-guard implementer — GPT-6.1 Sol High

- **Useful calls:** `describe_input` and `routing_facts` exposed the exact registered fact vocabulary; snapshot-bound routes made the scope reproducible. Focused reads covered Concurrency, Rust Async, Persistence, Rust API, Untrusted Execution and Verification. The worker rerouted after the partial-stage capability and aggregate-effect lifecycle boundaries changed, then routed the final candidate. The final route selected 24 standards with zero unresolved categories.
- **Confusing or redundant steps:** Discovery and schema descriptions were large, and some MCP results duplicated content between textual and structured forms. This was cumbersome; consuming the structured result avoided part of the duplication. No standards-corpus mutation or application-source attestation operation was useful or performed.
- **Missing context:** The MCP did not know the repository call graph, exact `TaskContext` lifecycle, admitted write set or candidate diff. Those were established locally and translated into registered routing facts.
- **Where the worker left MCP:** It used source inspection, tests, Git and local evidence for implementation and verification, returning to MCP when the design boundary materially changed and for final routing. Routing supplied obligations but did not certify the code.
- **Smallest sufficient workflow:** Discover input shape and routing facts once; route known facts on a pinned snapshot; read focused applicable policies; implement and verify locally; reroute when ownership changes and against the final candidate.
- **Recommendations:** Make discovery/results compact, avoid duplicate text and structured bodies, allow concise routing summaries with selected IDs and unresolved questions, and provide a clearly non-authoritative place to associate candidate/tree/write-set context with an evidence record.

## Q1 importer custody-guard independent reviewer — GPT-6.1 Sol High

- **Useful calls:** The reviewer reused `routing_facts` snapshot `snapshot:v1:9881b6be-4a0e-4d72-b0cd-4e4307cdc710`, ran a selection-only route with implementation/verification, library, persistence, Rust/API/async/security/cross-platform, architecture/concurrency/contracts/security/resilience/diagnostics and code-design/evolution/delegated-authority/oracle details, and received 24 selected standards with zero unresolved facts. Two focused `read_many` calls covered Rust Security, Untrusted Execution, Independent Test Oracles, Concurrency, Rust Async and Persistence. These obligations helped assess resource/stage authority, coherent reads, held capabilities, effect drainage, failure ownership and no-effect tests.
- **Confusing or redundant steps:** No MCP errors or schema-discovery calls were needed. Route explanations and repeated continuation suggestions were verbose.
- **Missing context:** The MCP did not expose the repository HF effect envelope or capability-relative metadata publisher. Source inspection was needed to trace those actual boundaries.
- **Where the reviewer left MCP:** Candidate identity, diff, source lifecycle and test claims were checked with Git and repository tools. The reviewer ran `git diff --check`, but no build or tests.
- **Smallest sufficient workflow:** Reuse the pinned snapshot, route the concrete review scope, read focused policies, then inspect the exact diff and tests independently.
- **Recommendations:** Return a compact selected-policy/count view by default and provide task-focused policy excerpts while keeping source and evidence qualification outside the routing result.

The review found no P0–P3 issue in its slice. That source-review conclusion and the writer-reported test evidence remain separate.

## Q1 receipt/reopen design reviewer

- **Useful calls:** The reviewer used one initial route and a final route on snapshot `snapshot:v1:968b380d-c352-496b-9a5f-f522222e09ab`; both selected 21 standards with zero unresolved questions. Two focused `read_many` calls read Persistence and Replay, then Contract Evolution and Schemas. Those obligations clarified that a receipt follows durable output publication, that current projections cannot stand in for exact historical completion, and that the receipt format must be explicit and versioned.
- **Confusing or redundant steps:** No vocabulary rediscovery was needed. Whole-policy reads were long, and the final route repeated the same facts because the slice workflow requires a final route.
- **Missing context:** The MCP could not identify actual schema writers, metadata no-op publication, startup index rebuild, cache fingerprint inputs, custody guard, or cancellation order. Source inspection was necessary and cannot establish deployed retained-state population.
- **Where the reviewer left MCP:** It inspected the repository for each persistence writer, output effect, migration/reopen path, startup rebuild, package-facts cache, and cancellation boundary; no tests or edits were performed.
- **Smallest sufficient workflow:** Route retained facts, read four focused policies, inspect source lifecycle, then route the final design.
- **Recommendations:** Add section-scoped policy reads, compact obligation references, and an explicit result label saying routing is complete while implementation and evidence remain unverified.

This design review is not product acceptance. Its source audit found no existing exact importer completion receipt; schema-7 implementation, cold-reopen tests, and independent final-candidate review remain required.

## Primary integrator — Q1 receipt/reopen slice routing

- **Useful calls:** A fresh `routing_facts` snapshot and route selected 28 standards with zero unresolved categories for persistence/replay, schema evolution, concurrency, Rust APIs/async/security, implementation, verification, planning and commit obligations. Focused reads on the same snapshot covered Persistence, Replay, Schemas, Contract Evolution, Concurrency, Code Design, Rust Async, Untrusted Execution, Verification Oracles, Resilience, Rust API, Commit, Platform Verification, Implementation and Documentation. Explicit policy text made the transaction boundary, old-reader handling, exact publication, platform evidence scope and commit obligations concrete.
- **Confusing or redundant steps:** The first five-policy read response was too large for the visible output window, duplicating full policy text across structured and text results; later small batches were more useful. The complete route is necessary at slice start, but verbose continuation explanations added little.
- **Missing context:** Routing could not tell that schema-6 `legacy` flattening would either drop or contaminate a receipt, that current schema-6 import metadata is capability-published but can no-op without a fresh barrier, or that package-facts fingerprints omit payload bytes and serialize `HashMap` metadata. Repository source and a read-only feasibility review established those facts.
- **Where I left MCP:** Exact reader/writer inventories, output formats, startup rebuild effects, marker order, and current CI/PR state came from source/GitHub tools. The package-facts fingerprint investigation and feasibility review are design evidence, not MCP evidence.
- **Smallest sufficient workflow:** Snapshot-bound route once; read the selected ownership, schema, replay, async, security and oracle policies in small focused batches; inspect exact source; reroute the implemented candidate before handoff.
- **Recommendations:** Add section-scoped reads with a practical byte budget; avoid returning policy bodies twice; surface the selected count and unresolved questions in compact form; keep routing status distinct from implementation evidence and acceptance.

This usability record is separate from product acceptance. At the time of this entry, the local candidate did not yet implement a receipt or reopen path; a later uncommitted worktree update is now being verified.

## Primary integrator — resumed Q1 acquisition and native-consumer route

- **Useful calls:** Reused the accepted snapshot `snapshot:v1:425e276a-fc0b-4282-9889-a76742027c42`, refreshed its fact definitions with `routing_facts`, and completed the route for launcher/library consumers, durable receipts, async lifecycle, security, platform/UI evidence, implementation, planning, release and commit work. The route selected 43 standards with zero unresolved facts. Paginated policy reads returned the full selected set in six bounded batches.
- **Confusing or redundant steps:** The first expanded route remained `needs-facts` until the workflow-profile condition was answered. The valid answer was `known-absent` for concurrent plan integration in this serial Q1 slice. The route accepted that fact and completed on the same snapshot. Policy retrieval is bounded by record count, but each selected item still contains its full policy text; reading 43 policies required six calls and substantial output handling.
- **Missing context:** The standards service did not expose repository state, current acquisition schema, the previously uncommitted receipt implementation, production RPC composition, or the Passeur service failure. These came from Git/source and separate Passeur diagnostics. Routing did not establish any implementation or acceptance claim.
- **Where I left MCP:** The route and policy reads supplied obligations only. Source review, test evidence, GitHub PR/CI state and exact plan-gate status remain separate repository or service observations.
- **Smallest sufficient workflow:** Reuse the accepted snapshot when its authority still applies; refresh facts, state every conditional applicability question explicitly, route, then page selected policy text in small batches. Reroute the completed source/evidence boundary after implementation.
- **Recommendations:** Let callers request focused policy sections from a completed route, preserve the compact selected-ID/count summary while paging, and show the exact missing fact prompt in a directly actionable form. Continue to label routing as obligations rather than implementation certification.

This usability feedback is separate from product acceptance. The schema-7 receipt implementation and native consumer changes remain subject to source review and their required real-consumer evidence.

## Independent Q1 composed reviewer — GPT-6.1 Sol High

- **Useful calls:** The pinned standards snapshot supported fact discovery, routing, and focused reads. The reviewer routed 29 standards with zero unresolved facts. A focused three-policy `read_many` returned complete, usable policy text.
- **Confusing or redundant steps:** Requesting 32 full policy texts exceeded the visible output window. The smaller focused read was easier to use.
- **Missing context:** Routing supplied applicable obligations but did not provide repository-specific source facts or establish implementation compliance or product acceptance.
- **Where the reviewer left MCP:** The reviewer inspected the exact current diff and source independently. It did not run tests or builds.
- **Smallest sufficient workflow:** Reuse the pinned snapshot, route the focused review scope, read a small number of relevant policies, then verify source and evidence separately.
- **Recommendations:** Keep complete policy reads bounded by output size; label a completed route as obligations only, separate from source compliance and acceptance.

This reviewer’s source findings are recorded separately from the MCP observations and do not certify the candidate.

## Shared native-consumer contributor — GPT-6.1 Sol Medium fallback

- **Useful calls:** After correcting the route tool's input shape, the contributor completed a focused standards route and used its selected obligations to inform implementation.
- **Confusing or redundant steps:** `describe_input({tool: "route"})` failed because the operation discriminator was missing and `tool` was an extra field; `describe_input({operation: "route"})` worked. Invented fact keys `language` and `scope` were rejected with a generic `/facts` message that did not identify the invalid keys; the contributor had to discover `routing_facts`. Route output repeated full content in both text and structured results, risking truncation.
- **Missing context:** The route did not identify the repository-specific acquisition service boundary; the contributor found that in source. Its route selected 28 standards and left two facts unresolved; the primary integrator separately completed the broader accepted route with zero unresolved facts.
- **Where the contributor left MCP:** It reported route obligations, not whether source changes complied or passed product acceptance.
- **Smallest sufficient workflow:** Describe the operation using its actual discriminator, retrieve the supported fact vocabulary before routing, resolve all returned facts explicitly, and keep full policy reads bounded.
- **Recommendations:** Make input descriptions work with the public tool name or provide a direct schema index; report each invalid fact key and valid alternatives; avoid duplicating large route bodies in text and structured output; retain the route's unresolved-fact count and distinguish obligations from evidence.

This feedback documents actual MCP usability only; it is not an implementation approval.

## Primary integrator — final Q1 candidate route

- **Useful calls:** Reused the accepted snapshot `snapshot:v1:425e276a-fc0b-4282-9889-a76742027c42`, refreshed its registered fact vocabulary, and routed the final implementation/verification scope. It selected 40 standards with zero unresolved questions. The route remained compact and returned actionable policy IDs.
- **Confusing or redundant steps:** A six-policy compact `read_many` still returned full policy bodies, exceeding the visible output window. Earlier bounded batches and the already-read policies supplied the remaining applicable obligations.
- **Missing context:** The route did not know the final native attempt identity, local HTTP fixture, current CI head, or platform execution results; those were checked in source, test output, and repository state.
- **Where I left MCP:** The route supplied obligations only. Review, exact source checks and test evidence were performed independently.
- **Smallest sufficient workflow:** Refresh facts, complete a snapshot-bound route, read only the relevant small policy subset, and check implementation/evidence separately.
- **Recommendations:** Add section-scoped reads and budget the response by serialized bytes, not number of policy records. Return selected IDs/count without repeating full bodies across structured and text output.

This is final-route usability evidence, not a Coding-Standards compliance or product-acceptance result.

## Primary integrator — resumed exact-candidate route

- **Useful calls:** A fresh route on snapshot `snapshot:v1:10b3e01e-6a86-4185-83e0-65ca7b0db870` selected 44 standards with zero unresolved questions. Two bounded `read_many` calls read the focused `core`, persistence, and concurrency policies.
- **Confusing or redundant steps:** The earlier snapshot handle was unavailable after session restart, so the route facts had to be refreshed and the same scope routed on a fresh handle. The small policy reads returned complete policy text and remained usable.
- **Missing context:** The standards service still supplied obligations rather than the actual Git diff, candidate test output, Passeur service health, or hosted CI status; these were checked separately.
- **Where I left MCP:** The current route and focused policy reads inform the final candidate obligations. They do not establish source compliance or gate acceptance.
- **Smallest sufficient workflow:** Refresh routing facts after a session restart, route the actual implementation/verification scope on the new snapshot, then read only the relevant policies in small batches.
- **Recommendations:** Provide a safe snapshot-refresh path after session loss and retain the clear distinction between standards obligations and independent source/test evidence.

This entry records the resumed candidate route only; implementation review and acceptance evidence remain separate.

## Primary integrator — finite acquisition admission and direct-installer candidate

- **Useful calls:** A fresh snapshot `snapshot:v1:28c08446-d4d8-4bab-bf4c-5122700bbd60` routed the library/launcher Rust implementation, concurrency, persistence, IPC error, compatibility, verification, documentation, and commit scope to 25 standards with zero unresolved fact categories. The 24 normative policy targets were read in one `read_many` call; the remaining route entry was the Router navigation index. Core and Rust profile obligations were available for the implementation review.
- **Confusing or redundant steps:** The first fact set over-reported cross-language binding, platform-target, and release detail; narrowing it to actual IPC/persistence, Rust API/async and resource-lifecycle concerns reduced the route from 35 to 25 while retaining zero unresolved facts. Adding `include_routing: true` to a policy `read_many` rejected the whole batch with `NAVIGATION.ROUTING_TARGET_INVALID` (“Routing definitions are available when reading Router”). Omitting that option allowed policy reads. Expanding all 24 full compact policy bodies still exceeded the visible output window even though the request stayed within the 2 MiB service limit; count limits alone do not bound what the caller can review.
- **Missing context:** The standards service did not know the current source diff, final Sol High review findings, Passeur agent lookup failure, or local command results; those came from source inspection and separate tools.
- **Where I left MCP:** The route supplies obligations, not code compliance or acceptance. A final source repair and review pass remain in progress; Q1 gates stay pending.
- **Smallest sufficient workflow:** Refresh routing facts, route the exact changed boundary, preserve the snapshot handle, read canonical policy bodies without `include_routing`, and focus output on the obligations relevant to the change.
- **Recommendations:** Separate Router navigation reads from canonical policy reads in the input contract; make `include_routing` invalidity specific to the affected target instead of rejecting a multi-policy batch; and add section-scoped or byte-budgeted reads so full route coverage can be reviewed without flooding the response.

This is tool-usability evidence only and does not certify implementation or product acceptance.

## Primary integrator — AC06 cleanup-custody regression route

- **Useful calls:** A fresh snapshot `snapshot:v1:e6fcc079-e95a-4f2e-a39e-47a7e667a20f` routed the Rust service test and durable-cleanup change to 20 standards with zero unresolved facts. Explicitly recording no framework and the observed concurrent-plan-integration condition completed the route. The selected guidance identified the library API, persistence, async custody, concurrency, verification-oracle, implementation, documentation and commit obligations.
- **Confusing or redundant steps:** The initial route returned `needs-facts` for framework applicability and whether outstanding proposals could stale before serial integration. A single `read_many` request for 18 compact policies returned complete policy bodies but exceeded the caller-visible output window and was truncated. Record-count limits do not ensure the bodies can be reviewed together.
- **Missing context:** The standards service did not describe the test harness's actual shared `TaskContext`, local HTTP endpoint, exact retained acquisition state, Passeur preflight error, or test result; those came from repository inspection and execution.
- **Where I left MCP:** The route supplies obligations only. Focused service tests, review of the exact code diff, and plan-gate evidence remain separate.
- **Smallest sufficient workflow:** Route all registered applicability facts explicitly, then read a few canonical policies per call and inspect service source/tests independently.
- **Recommendations:** Offer section-scoped reads or a byte-budgeted result option so a bounded policy count also stays within a reviewable response; keep completed routing clearly separate from implementation and acceptance.

This records MCP usability only; it does not certify implementation compliance or AC06 acceptance.

## Primary integrator — AC06 resume-after-pause route

- **Useful calls:** Snapshot `snapshot:v1:dbfe3a19-c99a-4a48-b98c-1647722d3219` routed the Rust service-test change to 29 standards with zero unresolved facts. Focused reads of concurrency, Rust async, verification oracles, architecture replay, and commit workflow supplied the applicable obligations.
- **Confusing or redundant steps:** One combined `read_many` response repeated results in structured and textual forms and exceeded the useful output window. A `workflow.commit` read returned a transient auto-review timeout message; the tool allowed one retry, and the exact retry succeeded. No approval was rejected.
- **Missing context:** The MCP did not know the current service fixture, test result, candidate diff, independent reviewer findings, or Passeur profile/coordination errors. These were checked with repository and service tools.
- **Where I left MCP:** Routing and policy reads supplied obligations only. Source inspection, `cargo test`, Clippy, formatting, diff checks and independent review provide the separate implementation evidence. None of these close AC06 or AQ-HTTP.
- **Smallest sufficient workflow:** Preserve the full snapshot handle, route the changed implementation/verification boundary, and read only the focused obligations in small batches. Inspect the candidate and run its evidence checks separately.
- **Recommendations:** Avoid repeating full policy results in both structured and text output; keep read results within a caller-reviewable byte budget; make transient auto-review timeout/retry status explicit without obscuring a successful retry.

This records tool usability only; it does not certify source compliance or product acceptance.

- **Final route refresh:** After implementation, the route was refreshed on snapshot `snapshot:v1:b187217e-0e3f-4b16-b101-74eca1357d5b`; it selected 22 standards with zero unresolved applicability facts and matched the admitted pre-edit scope. The route remains guidance only. Sol High's independent review and the serial local test/lint evidence are recorded separately in the execution ledger.

## Primary integrator — AC06 stale-generation boundary regression route

- **Useful calls:** The initial route `snapshot:v1:6e207013-b615-4b8e-85ca-4170c3bc1408` and the final source/evidence route `snapshot:v1:a2e24e74-b8f8-40cb-a3b4-762857343697` each selected 17 standards with zero unresolved applicability facts. Ten focused policy reads were repeated against the final snapshot and completed successfully for Rust, async, implementation, verification, oracle, concurrency, replay, contracts, resilience, and persistence guidance.
- **Confusing or redundant steps:** The first route left framework and concurrent-plan-integration applicability unresolved; explicitly setting both to known-absent produced the complete route. The first expanded route response exceeded the useful output window, so the final read was narrowed to the ten policies relevant to this service-level test.
- **Missing context:** The MCP did not know the actual two-generation fixture, the exact `files_ready` guard, the 8/8 focused test result, Clippy result, or Sol High review limits; these came from repository inspection and independent tools.
- **Where I left MCP:** The final route supplied obligations only. The test, focused checks, and review establish a sequential service boundary regression, not full AC06 acceptance.
- **Smallest sufficient workflow:** State framework and concurrent-integration applicability explicitly, route the exact Rust/persistence boundary, then read only directly applicable canonical policies in a caller-reviewable batch.
- **Recommendations:** Bound route response output by bytes as well as selected-policy count, and retain a concise selection summary when full policy text would exceed the caller's useful review window.

This records tool usability only; it does not certify source compliance or AC06 acceptance.

## Primary integrator — live-consumer and acceptance-record update

- **Useful calls:** A fresh route on snapshot `snapshot:v1:ba87accb-a675-4209-8a1a-2163140b4a96` selected 46 standards with zero unresolved applicability facts. The focused reads covered documentation, commit, release, and verification guidance. They reinforced recording real consumer boundaries and evidence scope, staging only the declared documentation slice, and keeping backend system evidence distinct from packaged/UI acceptance.
- **Confusing or redundant steps:** The first route requested six whole policy reads and returned a response large enough to exceed the useful output window. Repeating the route with one content item preserved the complete route summary and zero-unresolved result; focused reads were then issued separately. Fact refresh and routing worked consistently after the session restart.
- **Missing context:** The MCP did not know the actual HF LFS digest, llama.cpp asset ID/digest, isolated host/root facts, Build #357 retry result, or the transient RPC status error. Those were established through the product RPC, acquisition receipt, filesystem, and GitHub run tools.
- **Where I left MCP:** It supplied documentation, commit, release, and verification obligations only. It did not accept AC03/AC15, certify the Rust code, or decide AQ-HTTP readiness; those statuses are grounded in the recorded product and CI observations and remain scoped.
- **Smallest sufficient workflow:** Refresh routing facts, route the full implementation/evidence scope on a fresh snapshot with a small content page, then read only the policies needed for the final docs/commit/verification boundary.
- **Recommendations:** Return route counts and unresolved facts separately from policy bodies; cap results by response bytes as well as whole-policy count, and avoid duplicating selected policy text across content and structured output.

This records MCP usability only. Product acceptance and source review remain separate.

## Primary integrator — AC02 validator-bound warm-resume candidate

- **Useful calls:** The pinned final route (`snapshot:v1:faff0add-81ef-42b1-a2d1-043b012c9643`) selected 30 standards with zero unresolved applicability facts. Focused reads covered Core, persistence boundary, Rust API/async/security, concurrency, contracts and contract evolution, architecture, resilience, code design, verification, build, documentation, and commit workflow. The routing result and the direct source/test evidence remained separate.
- **Confusing or redundant steps:** The route's initial broad policy page was too large to review as one response. Smaller `read_many` batches were usable, but still returned complete policy bodies; the policies' metadata, `next_operations`, and prose were included together.
- **Missing context:** The MCP did not know the exact candidate tree, the twice-reproduced baseline failure, post-fix Cargo results, the shared target/build serialization, or the loopback llama.cpp integration fixture. Git and the repository test output established those facts.
- **Where I left MCP:** The 30-standard route resolved applicability only. It did not inspect or approve the code, verify test execution, or accept AC02/AQ-HTTP.
- **Smallest sufficient workflow:** Reuse the exact snapshot, read only the selected obligations that apply to the final boundary in small batches, and establish candidate identity, review, tests, and acceptance independently from repository evidence.
- **Recommendations:** Keep route counts and unresolved facts visible without repeating policy-selection rationales; make policy reads section-scoped or byte-budgeted; and make the non-certifying role of a completed route explicit.

This records MCP usability only; it is not evidence of implementation compliance or product acceptance.

## Primary integrator — AC01 desktop revision pinning implementation

- **Useful calls:** Refreshed routing facts and routed the completed Rust library/API implementation and verification boundary on snapshot `snapshot:v1:a37ceadd-a5ff-4805-a3b9-b3362c3ea219`. The route selected 25 standards with zero unresolved applicability facts. Focused reads covered Core, the library and Rust API/async/security profiles, persistence, concurrency, contracts/evolution/protocols, architecture/replay, resilience, verification/oracles, planning, documentation, implementation, proportionality, and commit guidance.
- **Confusing or redundant steps:** A large `read_many` request returned complete policy bodies with metadata and continuation records, beyond the useful output window. Narrowing to nine policies reduced irrelevant material but still produced a large response. The route itself gave a concise selected count and unresolved-fact result.
- **Missing context:** The MCP did not know the `start_hf_download` call path, exact local-server request sequence, missing-SHA fixture, lifecycle-task accounting behavior, test results, or strict Clippy result. Those facts came from source inspection and serial local verification.
- **Where I left MCP:** Routing supplied obligations only. The controlled public-entrypoint tests, closed-lifecycle regression, strict Clippy, formatting, and source diff are the separate implementation evidence. None accepts AC01 or AQ-HTTP.
- **Smallest sufficient workflow:** Refresh facts, route the completed boundary with explicit application, language, topic, workflow, and evidence facts, then read only focused policies in small batches and check the source/test contract independently.
- **Recommendations:** Keep policy reads byte-bounded as well as item-bounded; avoid repeating whole policy bodies together with metadata and continuation records; show the route's compact count and unresolved facts separately from policy text.

This records tool usability only; it does not certify source compliance or product acceptance.

## Primary integrator — AC10 warm-checkpoint retention admission

- **Useful calls:** Refreshed facts and routed the Q1 bounded-retention implementation/evidence scope on snapshot `snapshot:v1:f476feaf-6487-405b-8d2d-7742115e342d`. The route selected 22 standards with zero unresolved fact categories. The selected Rust, lifecycle, contracts, architecture/replay, resilience, performance, code-design, oracle, planning, integration, and commit policies supplied the scope and typed-outcome obligations before source changes.
- **Confusing or redundant steps:** `read_many` correctly returned complete policy bodies, but even a four-policy batch exceeded the useful response window because body text and metadata were bundled. Smaller two-policy batches were readable; section-scoped or byte-budgeted retrieval would reduce review overhead.
- **Missing context:** The MCP did not know the current checkpoint map's ownership, cloned-record size, service-view sharing, pause outcome, or existing byte-zero fallback. Repository source and Sol High's read-only architecture audit established those facts.
- **Where I left MCP:** The 22-standard route and policy reads are guidance only. Source review identified the defect; focused tests, exact diff review, and final code review will provide implementation evidence. This does not accept AC10 or AQ-HTTP.
- **Smallest sufficient workflow:** Refresh facts, route the exact slice, read the selected policies in small batches, and record the candidate's real checks independently.
- **Recommendations:** Keep full policy bodies available, but return only requested sections or enforce a response-byte budget so metadata and continuations do not obscure the normative content.

This records tool usability only; it does not certify source compliance or product acceptance.
