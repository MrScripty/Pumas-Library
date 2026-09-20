# Issues: Torch Image Provider Contract Correction

**Plan:** [plan.md](plan.md)

## TIPC-I01 — Elapsed generation deadlines contradict the product contract

- Finding: the sidecar and Rust image client each enforce 600 seconds. A
  615-second gateway policy remains documented/configured but the current image
  branch bypasses that generic request wrapper.
- Disposition: TIPC-M1 removes operative deadlines and stale image policy while
  retaining independently owned connection/startup/load/install policies.
- Evidence: TIPC-01 and TIPC-02; implementation must trace the real route rather
  than treating every documented timeout as operative.

## TIPC-I02 — Protocol 2 cannot distinguish the corrected lifetime

- Finding: the client, bundled sidecar, and source recipe use protocol `2`.
  Existing compatibility checks precede load and filter listing, but an older
  live protocol-2 sidecar can accept the same request while retaining the old
  deadline; handshake defaults and process replacement also permit ambiguity.
- Disposition: evolve the existing mechanism to strict protocol `3`, bind
  compatibility to current process/profile identity, recheck before admission,
  and retain independent capability and ready-slot checks. Do not add a lifetime
  capability, registry, or cache in parallel.
- Evidence: TIPC-05 and TIPC-06.

## TIPC-I03 — Result documentation is stricter than the current decoder

- Finding: Rust currently accepts additive private result fields, while the
  private protocol document describes a closed result. Required values and the
  whole response still need canonical validation and bounds.
- Disposition: preserve permissive additive decoding, validate all required
  semantics, enforce the bound over the whole payload, and keep unknown private
  data out of public projection and diagnostics.
- Evidence: TIPC-07 and TIPC-08.

## TIPC-I04 — Cancellation request is not resource-safe completion

- Finding: current sidecar structure waits for worker cleanup, but cancellation,
  disconnect, wrapper-drop, and process-failure states require real-route proof;
  socket failure alone cannot establish that external GPU work stopped.
- Disposition: retain worker/device custody through sufficient cleanup, expose
  requested/pending/completed/failure states through existing mechanisms, refuse
  unsafe reuse, and preserve unknown outcomes without replay.
- Evidence: TIPC-03, TIPC-04, and TIPC-11.

## TIPC-I05 — Candidate and user-workflow evidence are external handoffs

- Finding: source recipe `0.1.5` is not present in the installed `0.1.1`–`0.1.4`
  inventory and no Torch runtime asset appears in the reviewed release inventory.
  Tuldok already sends numeric dimensions and defaults to 1280×720, but retains a
  630-second generation deadline and its real browser check used a smaller image.
- Disposition: reserve `torch-runtime-0.1.6`; recheck before building. The Tuldok
  image plan owns construction/install qualification, real GPU cleanup, removal
  of its remaining consumer deadline, and the supported 1280×720 browser flow.
  Distribution consumes the result later under separate authority.
- Evidence: TIPC-04, TIPC-09, and TIPC-10.

## TIPC-I06 — Explicit generation contract takes precedence

- Finding: adopted Resilience guidance requires an end-to-end deadline for
  remote calls, while the explicit Pumas product policy says elapsed time alone
  is never a failure condition for any admitted generation.
- Disposition: `TIPC-GEN-01` records the generation-wide Core precedence. It is
  the normal generation contract, not an image exception. Connection/setup,
  non-generation operations, cleanup, and termination may retain separate
  budgets; bounded admission, cleanup custody, no replay, and authorized process
  stop remain mandatory. The implementation updates the appropriate Pumas
  architecture/contract owner and amends ADR 0002 for the Torch image case;
  Coding-Standards is unchanged by this repository task.
- Evidence: contract/ADR review plus TIPC-01–TIPC-03 and TIPC-11/TIPC-12.

## TIPC-I07 — Shared gateway limits cap non-image generation

- Finding: the shared gateway HTTP client has a total timeout and current
  `/v1/chat/completions` and `/v1/completions` policies add 120-second request
  deadlines. A long connected text generation can therefore fail solely due to
  elapsed time even though generation remains valid. The current proxy buffers
  the provider response body, so a supported non-streaming request may remain
  silent until completion; this correction does not claim new streaming support.
- Disposition: make one generation transport/lifecycle seam connection-bounded
  but duration-unbounded and use it for current and future generation routes.
  Keep models, embeddings, health, startup, loading, installation, cleanup, and
  shutdown on their independently justified policies.
- Evidence: TIPC-12 plus configuration inspection proving no client-wide,
  request-total, response-read, idle, or middleware generation deadline remains.

## TIPC-I08 — Shared transport proof does not prove external provider cleanup

- Finding: Ollama and llama.cpp chat/completions share the generic buffered
  gateway implementation, while Torch uses a dedicated image adapter. A
  controlled provider can prove Pumas transport behavior but cannot prove that
  actual Ollama or llama.cpp workers stopped after disconnect.
- Disposition: TIPC-12 owns only the shared Pumas transport, route wiring,
  disconnect signal, uncertainty, and no-replay claim. The current-route table
  names each provider lifecycle owner. Torch image cleanup remains TIPC-03/04;
  unregistered Torch text remains with broader Torch remediation. Future routes
  inherit the contract only as an admission requirement.
- Evidence: route/registry construction tests plus one controlled long/silent
  non-image transport integration; provider-specific cleanup requires separate
  owner evidence and is not inferred from TIPC-12.

## TIPC-I09 — Native deadline fixture directly conflicts with the correction

- Finding: `torch-server/tests_native/test_image_failures.py` patches the
  600-second constant and expects HTTP 504/`deadline_exceeded` while also
  asserting the valuable cleanup-before-lease-release invariant.
- Disposition: the file is explicitly admitted. Replace its deadline trigger
  with owner cancellation or disconnect and retain the cleanup assertion;
  separately prove continued execution beyond the old boundary with controlled
  time.
- Evidence: focused native-suite execution plus TIPC-01/TIPC-03.

## TIPC-I10 — Required project-scoped Passeur tools are not callable

- Finding: initial intake found no Pumas or Tuldok profile and no Passeur tools
  in the current Codex session. The installed registration helper owns one
  default `muse_bridge` name, while this assignment requires distinct
  project-scoped Pumas and Tuldok servers.
- Disposition: configured separate outside-checkout worktree roots and the named
  `passeur_pumas`/`passeur_tuldok` servers after human model/subscription and
  configuration authorization. Both profiles and registrations pass Doctor and
  `codex mcp get`. A Codex restart remains required before the tools can appear;
  do not substitute native Codex subagents or direct Muse sessions.
- Evidence: Passeur Doctor/profile lookup and current MCP tool inventory. This
  blocks implementation execution only until restart, not plan admission or
  read-only preparation.

Post-restart diagnostic: both configured servers start and list the exact four
schema-version-2 tools through a read-only MCP SDK client, but the resumed
conversation's callable-tool inventory still omits both server namespaces. A
new conversation attachment, rather than another profile/configuration change,
is required before this issue can close.

Second-session diagnostic: both namespaces are now callable. The first Pumas
batch reached worker startup and failed deterministically because Passeur used
`clientInfo.name = "muse-bridge"`, which Muse 1.3 rejects under its
`^[a-z0-9_]+$` machine-identifier rule. Tuldok first rejected a cross-repository
plan path before worker startup, then reached the same client-name failure after
its context list was corrected. A minimal handshake reproduced the failure and
passed with `muse_bridge`; the local Passeur source, probe, regression test, and
ignored built runtime were corrected in local commit `4f4ef0f`, and the full
test suite, typecheck, and build passed. The attached MCP processes retain the
pre-build module, so one further Codex restart is required. All six zero-change
failed allocations were explicitly archived and retired; no product task
remains live.

Third-session diagnostic: after the requested restart, the corrected Passeur
runtime and registrations remain present but the resumed conversation's
callable inventory again omits both namespaces. A sandboxed startup reports the
generic repository-in-use error because it cannot acquire Passeur's external
state lease; host-visible process inspection and state-directory inspection
show no running coordinator or surviving lock. The same configured command,
launched read-only with its required state access, initializes and lists all
four tools. The remaining failure is conversation attachment, not Passeur
source, profile, registration, task state, or a live worker. Start a newly
created conversation that explicitly requests `passeur_pumas` and
`passeur_tuldok`; do not use a custom client that bypasses elicitation.

Fourth-session diagnostic: the user created another conversation and explicitly
named both servers. `codex mcp get` still resolves both enabled registrations
with the four-tool allowlist, and both Doctors pass. Host-visible inspection now
shows one live exact-command process for each server; the Pumas process owns the
repository coordination lease. The model's complete callable inventory still
contains neither namespace. Consequently, a resource-list request and a
bounded read-only SDK handshake start duplicate processes and close; outside
the restricted sandbox the duplicate reports `Another coordinator or offline
mutation owns this repository`. This is expected contention with the healthy
attached owner, not evidence that the profile or lease is stale. Do not kill
the owner or use a custom client. The remaining blocker is host-to-model tool
exposure for the already attached processes.

Material tooling drift observed during this diagnostic: the clean Passeur
source advanced from compatibility commit `4f4ef0f` to `49724a8`, while the
ignored configured `dist/` runtime retains the older CLI surface (for example,
`--version` prints the old usage text). Reconcile and verify that build/runtime
identity after callable attachment is restored and before dispatch.

Official Codex MCP configuration documentation states that optional servers get
a one-second grace while the initial tool catalog is built, unless the global
grace is disabled, while required servers use their startup timeouts. The two
Passeur registrations set a 10-second startup timeout but have no `required`
flag, and the global optional grace is unset. That configuration matches the
observed live-process/missing-catalog shape but remains a hypothesis until an
authorized change marks only these requested servers required and a fresh
session verifies the resulting catalog.
