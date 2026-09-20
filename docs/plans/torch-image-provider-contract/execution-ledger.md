# Execution Ledger: Torch Image Provider Contract Correction

**Plan:** [plan.md](plan.md)

## 2026-09-20 — Reconcile planning authority

- Operation: planning reconciliation only; production implementation was not
  admitted or performed.
- Reviewed Pumas `74d4f7f0a5345e2dba476313d9cb409f4fceeb61`, adopted
  Coding-Standards `366c1d90a24bbfb50973f62b155a5f3396c0f107`, and Tuldok
  `6e9e6ec32d4dd719af0e051baafacecd190864c2`.
- Reconciled the user-added standalone draft into the canonical focused plan.
  Chose separate ownership because the active Tuldok image plan is at broader
  distribution acceptance and this correction has an independent bounded write
  set and acceptance gate.
- Inspected the operative 600-second sidecar and Rust-client deadlines, the
  inoperative image branch of the documented 615-second gateway policy, current
  protocol-2 handshake/recipe, permissive Rust result decoding, strict public
  request and safe projection, serving/listing compatibility checks, immutable
  installer/qualifier, installed recipe inventory, and current release inventory.
- Inspected Tuldok's current numeric width/height request, 1280×720 defaults,
  image display/save, request-abort cleanup, explicit-resume/no-automatic-retry
  behavior, and remaining 630-second consumer deadline. These findings narrow
  its companion work; they do not overwrite historical evidence.
- Selected protocol `3` and reserved `torch-runtime-0.1.6`, both subject to a
  collision recheck before implementation/construction. Initially recorded an
  image-only deadline precedence; the clarification below supersedes that scope.
- Assigned every TIPC acceptance claim one evidence owner. TIPC-M1 owns the
  coherent provider correction; the existing Tuldok image plan owns exact-
  candidate, GPU cleanup, and 1280×720 user-workflow evidence.
- Preserved unrelated local `docs/breif/future.md`. No code, tests, runtime,
  lock, installation, activation, publication, commit, or external mutation was
  performed.
- Validation: all relative Markdown links in the changed plan artifacts resolve;
  `git diff --check` passes; the then-current TIPC acceptance claims each have
  one table-row evidence owner; the superseded draft path is absent; repository
  status shows only plan documentation changes plus the preserved unrelated file.

## 2026-09-20 — Make lifetime generation-wide

- User clarification: an admitted generation never fails merely because it has
  run for a long time. The image lifetime is the reusable policy for text and
  future generation, not a narrow exception.
- Source inspection found a shared gateway-client total timeout and 120-second
  per-request deadlines on `/v1/chat/completions` and `/v1/completions`, in
  addition to the already-recorded image limits.
- Replaced `TIPC-EX-01` with generation-wide `TIPC-GEN-01`; added TIPC-12 and
  assigned this plan the single reusable generation transport/lifecycle seam.
  Text shapes, sampling, usage, and other broader remediation remain with their
  existing owner.
- Coding-Standards was not modified because this repository task remains limited
  to Pumas planning; the explicit Pumas generation contract takes precedence
  over the generic remote-operation deadline requirement for generation.
- No production implementation or runtime mutation was performed.
- Validation: TIPC-01 through TIPC-12 each have one evidence owner, reciprocal
  plan references agree, and no prospective Pumas plan assigns an elapsed
  deadline to generation.

## 2026-09-20 — Tighten proof and handoff boundaries after review

- Reviewed committed reconciliation `c93a648c6b96785ab82dd63c47452002b784d23b`
  and current source; unrelated untracked `docs/breif/future.md` remains untouched.
- Kept the generation-wide policy and separated its Pumas transport proof from
  provider-native cleanup. Current chat/completions registration is Ollama and
  llama.cpp through one generic buffered handler; Torch remains image-only at
  the public registry. One controlled transport case may cover the shared
  implementation only when route tests prove every registered path uses it.
- TIPC-12 no longer claims Ollama/llama.cpp worker cleanup. Their serving owners
  retain that obligation; Torch image cleanup remains TIPC-03/04; unregistered
  synchronous Torch text remains with broader Torch remediation.
- Restored acceptance kind, environment, and mode; marked TIPC-M1 `Planned` and
  composed-design applicability `applicable`; added independence reasoning and
  supporting Rust, Ruff, Python unit, and native gates.
- Explicitly admitted `torch-server/tests_native/test_image_failures.py` and
  directed its obsolete deadline trigger to become cancellation/disconnect
  cleanup evidence, separate from the controlled-clock lifetime regression.
- The diffusion plan now preserves original M3 as completed within its recorded
  scope, marks `M3-TIPC` planned, and returns each TIPC handoff as soon as its
  own evidence passes rather than waiting for M5 or publication-dependent A1.
- Planning documentation only; no production source, tests, runtime, or release
  state changed.

## 2026-09-20 — Admit TIPC-M1 and prepare parallel Passeur allocation

- Canonical admission: `docs/plans/torch-image-provider-contract/plan.md`,
  operation `start`, milestone `TIPC-M1`. The plan and milestone moved from
  `Planned` to `Active`; acceptance remains pending.
- Integration owner: local branch
  `refs/heads/integration/torch-image-provider-contract`, created from exact
  Pumas base `74214cc9ed755356ead124cd72fb04f0eb84fda4`.
- Shared contract supplied to both initial Pumas assignments: Torch protocol
  `3`; strict closed numeric requests; required canonical result fields with
  unknown additive private result fields allowed inside the whole-payload
  bound; no elapsed, total, read, or idle generation deadline; disconnect is a
  cancellation request rather than proof of stopped work; resources remain
  owned through sufficient cleanup; transport loss is uncertain and never
  authorizes automatic replay. Non-generation budgets remain independently
  owned.
- Planned assignment `TIPC-M1-R` (Pumas): Rust generation transport,
  TorchClient handshake/result/error interpretation, serving/listing/admission,
  installer consistency, focused Rust tests, and public/generation contract
  documentation. Primary write set:
  `rust/crates/pumas-app-manager/src/torch_client.rs`,
  `rust/crates/pumas-app-manager/src/version_manager/installer/{torch.rs,torch_tests.rs}`,
  `rust/crates/pumas-rpc/src/handlers/{serving_torch.rs,openai_gateway.rs,openai_gateway_images.rs,openai_gateway_tests.rs}`,
  `rust/crates/pumas-rpc/src/server.rs`, `docs/ARCHITECTURE.md`,
  `docs/contracts/{generation-lifetime.md,image-generation.md}`, and
  `docs/adr/0002-torch-image-provider-protocol.md`. Read-only context includes
  the canonical plan, private protocol, sidecar sources, repository guidance,
  and routed standards. It must not edit the plan directory, Torch sidecar,
  dependency locks, runtime defaults, Tuldok, or unrelated provider semantics.
  Deliverable: ordinary committed Muse contribution with focused app-manager
  and RPC evidence for TIPC-01/02/05–08/11/12 within the Rust/public boundary.
- Planned assignment `TIPC-M1-P` (Pumas): Python sidecar lifetime,
  cancellation/cleanup custody, protocol-3 advertisement, bundled recipe/live
  qualification, controlled-clock and native cleanup regressions, private
  protocol and sidecar documentation. Primary write set:
  `torch-server/{image_api.py,serve.py,validate_runtime.py,README.md,runtime/runtime.json,tests/**,tests_native/test_image_failures.py}`
  and `docs/contracts/torch-provider-protocol.md`. Read-only context includes
  the canonical plan and Rust client/gateway owners. It must not edit the plan
  directory, requirements lock, Rust source, public runtime defaults, Tuldok,
  or production installations. Deliverable: ordinary committed Muse
  contribution with Ruff, Python unit, controlled lifetime, and available
  native cleanup evidence for the sidecar portions of TIPC-01/03/05–07/11.
- `TIPC-M1-R` and `TIPC-M1-P` are independent after the recorded protocol
  agreement and are intended for one Pumas batch at exact implementation base
  `96f859443460ad8e4799d563528aaba113ccff21`, targeting
  `refs/heads/integration/torch-image-provider-contract`. Serial integration
  order is R then P unless returned commit assumptions require a repair
  assignment. Shared plan lifecycle, ledger, issues, final contract
  reconciliation, candidate construction, GPU allocation, and qualification
  roots remain coordinator-owned and serialized.
- Planned companion assignment `M3-TIPC-T` (Tuldok): governing Pumas plan
  `docs/plans/torch-diffusion-serving/plan.md`, operation `continue`, milestone
  `M3-TIPC`; exact Tuldok base
  `6e9e6ec32d4dd719af0e051baafacecd190864c2`; target
  `refs/heads/integration/torch-image-provider-contract`. Write set is `image_generation.py`,
  `tests/test_image_generation.py`, `tests/browser_images_real.cjs`, and
  relevant Tuldok user documentation. Preserve numeric dimensions, 1280×720,
  socket/watcher cancellation, and no replay; remove the 630-second generation
  deadline and closed metadata examples. Exact-candidate browser/GPU evidence
  waits for integrated Pumas source and exclusive qualification ownership.
- Planned secondary assignment `FE-GET-APP-STATUS` (Pumas): governing
  `docs/plans/current-standards-remediation-2026-09-03/frontend-and-ui/plan.md`,
  operation `continue`, only the admitted `get_app_status` producer/generated
  response, actual preload, and `usePlugins` consumer correction. Its exact
  generated/shared write set must be inventoried before delegation; it is not a
  Torch prerequisite and will not overlap initial product workers.
- Passeur intake: installed Passeur revision
  `b0c162d779f52b1c7bb19274013d56d634020513` and Muse Code/SDK `1.3.0`; profile
  defaults are two active workers and eight queued. The only discovered profile
  belongs to the Passeur repository itself. No Pumas or Tuldok project profile
  exists, and this Codex session exposes none of `delegate_to_muse`,
  `delegate_to_muse_batch`, `muse_result`, or `muse_finalize`. No worker was
  dispatched. Separate project registrations and a Codex restart are required;
  the missing Tuldok server does not broaden or block independent Pumas scope,
  while the missing Pumas implementation server prevents source execution.
- Because Passeur rejects any dirty configured source checkout and the primary
  Pumas checkout intentionally retains unrelated untracked
  `docs/breif/future.md`, the coordinator created the clean, repository-linked
  source worktree `/media/jeremy/OrangeCream/passeur_cache/pumas-coordinator-source`
  on `refs/heads/passeur/pumas-coordinator` at the admitted implementation base.
  It belongs to this orchestration only, is not an integration target, and must
  remain clean and retained until Passeur resources are finalized; afterward
  its exact head must remain reachable before ordinary safe worktree/branch
  retirement. No existing `.muse` or other-owner worktree was changed.
- Preserved unrelated untracked `docs/breif/future.md`. No product/test source,
  runtime installation, candidate, GPU process, launcher root, publication, or
  remote state changed.
- After completing all independent intake and allocation work available in the
  current session, set the plan and `TIPC-M1` to `Blocked`: creating verified
  project profiles requires the user's subscription confirmation, named Codex
  MCP registration changes require configuration authority, and newly
  registered tools cannot appear until Codex restarts. The blocker does not
  authorize direct implementation or another delegation mechanism.

## 2026-09-20 — Configure project-scoped Passeur servers

- User confirmed `muse-spark-1.3-contributor` is the intended model and uses the
  intended subscription, and authorized Pumas/Tuldok profiles plus named Codex
  registrations.
- Configured Pumas profile
  `/home/jeremy/.config/muse-bridge/projects/57bbd005b8a2f399ae95fd99.json`
  for clean coordinator source
  `/media/jeremy/OrangeCream/passeur_cache/pumas-coordinator-source` and task
  root `/media/jeremy/OrangeCream/passeur_cache/pumas-tasks`.
- Configured Tuldok profile
  `/home/jeremy/.config/muse-bridge/projects/00ba9d9685e64d372f3403fe.json`
  for the canonical Tuldok checkout and task root
  `/media/jeremy/OrangeCream/passeur_cache/tuldok-tasks`.
- Both profiles enable implementation, use Muse Code/SDK `1.3.0`, retain the
  documented two-worker/eight-queue limits and 30-minute task budget, and carry
  current `user_confirmed` subscription provenance. Doctor maps them to Pumas
  Git common-directory identity `6aaae9e5ae2b753918ac7478` and Tuldok identity
  `6240622ff4d89463e085c67b`, respectively.
- Added distinct Codex servers `passeur_pumas` and `passeur_tuldok`. Both enable
  exactly `delegate_to_muse`, `delegate_to_muse_batch`, `muse_result`, and
  `muse_finalize`, with 10-second startup and 2100-second tool timeouts. Existing
  MCP registrations and the Passeur repository's own profile were preserved;
  the pre-field-update Codex configuration is backed up at
  `/home/jeremy/.codex/config.toml.passeur-multi-backup-20260920`.
- `codex mcp get` and both Doctor runs passed. Doctor is non-inference evidence:
  it does not prove provider parallelism, elicitation, sandbox enforcement,
  hooks, signing, or a live Muse turn. The current Codex process still exposes
  none of the newly registered tools, so the plan remains `Blocked` pending the
  documented restart rather than attempting direct implementation.

## 2026-09-20 — Post-restart MCP attachment diagnostic

- The user reported restarting the session and authorized `continue`.
- The active conversation runtime still exposes neither `passeur_pumas` nor
  `passeur_tuldok`; its complete callable-tool inventory contains none of the
  four Passeur operations.
- A read-only MCP SDK diagnostic started the configured Pumas server, advertised
  elicitation capability, listed tools, and closed it without invoking a task.
  The server successfully returned the installed schema-version-2 definitions
  for `delegate_to_muse`, `delegate_to_muse_batch`, `muse_result`, and
  `muse_finalize`. This proves server startup and tool registration only; it
  does not prove a live Muse turn, subscription execution, approval routing, or
  parallelism.
- Therefore the remaining blocker is Codex conversation attachment, not the
  Passeur profile, MCP server, repository route, or schema. Direct Muse shell
  execution or a custom client that accepts approval prompts would bypass the
  required human elicitation path and remains prohibited. The plan stays
  `Blocked` until a newly created Codex conversation exposes both registered
  namespaces.

## 2026-09-20 — Live delegation reveals Passeur/Muse identity incompatibility

- The new conversation exposes both `passeur_pumas` and `passeur_tuldok`, each
  with the expected four schema-version-2 operations. The prior attachment
  blocker is therefore resolved.
- Rechecked the recorded bases and allocations before dispatch. Pumas
  coordinator source remains clean at
  `96f859443460ad8e4799d563528aaba113ccff21`; Tuldok remains clean at
  `6e9e6ec32d4dd719af0e051baafacecd190864c2`; protocol `3` and
  `torch-runtime-0.1.6` have no new source allocation. The primary Pumas checkout
  still contains only the preserved unrelated `docs/breif/future.md`.
- Dispatched Pumas batch keys `TIPC-M1-R` and `TIPC-M1-P`. Both stopped before a
  Muse session with `SS1.4.1`: Passeur supplied `clientInfo.name` as
  `muse-bridge`, while Muse 1.3 requires `^[a-z0-9_]+$`. Dispatched Tuldok key
  `M3-TIPC-T`; it stopped before worker start because its `context_files`
  incorrectly named the cross-repository Pumas plan. All three allocations had
  zero commits and zero changed files and were explicitly archived/retired.
- Applied the diagnosing-bugs loop to the tooling boundary. A direct handshake
  reproduced the exact error with `muse-bridge` and passed with `muse_bridge`.
  Local Passeur commit `4f4ef0f` now uses `muse_bridge` in the adapter and
  `muse_bridge_probe` in the probe, with a focused adapter regression. The
  ignored `dist/` runtime was rebuilt. The full Passeur suite passed (53 core
  tests and 35 Vitest tests), TypeScript check passed, build passed, and
  `git diff --check` passed. The pre-existing unrelated
  `passeur-parallel-workers-changes.zip` remains untouched.
- Retried with fresh Pumas keys `TIPC-M1-R-v2`/`TIPC-M1-P-v2` and Tuldok key
  `M3-TIPC-T-v2`, limiting Tuldok `context_files` to files present in that
  repository. The already-attached MCP server processes retained the old
  Passeur module and reproduced the same initialization failure. These three
  allocations also had zero commits and zero changed files and were explicitly
  archived/retired.
- No custom MCP client was used to delegate, no approval path was bypassed, and
  no product source, test, runtime installation, candidate, GPU process,
  launcher root, publication, or remote state changed. `TIPC-M1` remains
  `Blocked` until Codex restarts the two attached MCP processes and the recorded
  assignments are retried against the rebuilt Passeur runtime.

## 2026-09-20 — Corrected runtime passes; resumed thread omits namespaces

- The user reported restarting and authorized continuation. Pumas remains at
  plan commit `63863942`, its clean coordinator source remains at
  `96f859443460ad8e4799d563528aaba113ccff21`, and Tuldok remains clean at
  `6e9e6ec32d4dd719af0e051baafacecd190864c2`. The unrelated untracked Pumas
  `docs/breif/future.md` and Passeur
  `passeur-parallel-workers-changes.zip` remain untouched.
- Both named registrations remain enabled with the expected commands, profiles,
  tool allowlists, startup timeout, and task timeout. Passeur remains at local
  compatibility commit `4f4ef0f`, and its ignored built adapter contains the
  corrected `muse_bridge` identity.
- This resumed conversation exposes neither Passeur namespace in its callable
  tool inventory. A forced sandboxed resource/startup probe closed during MCP
  initialization; a minimal sandboxed reproduction reported Passeur's generic
  repository-in-use error while attempting to acquire its external state lease.
- Host-visible process inspection found no Passeur or Muse process, and state
  inspection found no surviving repository lock. A read-only MCP SDK handshake
  against the exact configured Pumas command with normal state access
  initialized successfully and listed `delegate_to_muse`,
  `delegate_to_muse_batch`, `muse_result`, and `muse_finalize`.
- Official Codex documentation confirms that configured stdio MCP servers and
  their enabled-tool allowlists are loaded through Codex configuration; it does
  not provide a supported mechanism for injecting a missing namespace into an
  already-running conversation. A custom client remains prohibited because it
  would bypass Passeur's human approval elicitation.
- No task was dispatched, no task resource or lease remains live, and no product
  source or Tuldok state changed. `TIPC-M1` remains `Blocked` until a newly
  created conversation explicitly naming both configured servers exposes their
  callable tools.

## 2026-09-20 — Named servers run, but their operations remain unexposed

- The user created another conversation and explicitly requested the configured
  `passeur_pumas` and `passeur_tuldok` servers. The complete callable-tool
  inventory still omits both namespaces and all eight task operations.
- `codex mcp get` resolves both enabled stdio registrations with the recorded
  project/profile arguments, four-tool allowlists, 10-second startup timeout,
  and 2100-second tool timeout. Both project Doctors pass; they remain
  non-inference diagnostics.
- Host-visible process inspection found one live exact-command Passeur process
  for Pumas and one for Tuldok, both parented by the Codex host. Passeur
  inspection shows only the four previously archived, retired, zero-change
  Pumas attempts. No Muse worker or product task is live.
- Built a sub-second deterministic feedback loop with a bounded read-only MCP
  SDK initialization against the exact registered Pumas command. In the managed
  sandbox it closes with MCP `-32000` and empty stderr. Outside that sandbox the
  same duplicate startup closes with `Another coordinator or offline mutation
  owns this repository`, proving that the already attached Pumas process holds
  the repository coordination lease. Resource listing fails for the same
  duplicate-start reason; it is not a substitute for the omitted task tools.
- The diagnosis rules out a missing profile, dead registered process, stale
  orphan lock, or SDK-version-only failure. The blocker is the Codex
  host-to-model callable inventory: the server processes are attached and own
  their resources, but their task operations were not supplied to this model
  turn. Killing those owners or using a custom client would be destructive or
  bypass required elicitation and was not attempted.
- Official Codex MCP documentation records a one-second default grace for
  optional servers during initial tool-catalog construction; servers marked
  required instead use their startup timeouts. The configured Passeur entries
  specify 10-second startup timeouts but are not required, and no global grace
  override is present. This is the leading host-side explanation for live
  processes with omitted tools. Changing the global configuration and starting
  another session requires explicit authority and remains untested.
- Material external tooling drift was recorded before dispatch: the clean
  Passeur source is now `49724a8` (`feat: add discoverable startup runtime`),
  while the ignored configured `dist/` still presents the older CLI surface;
  its `--version` invocation prints the old usage text. The compatibility fix
  commit `4f4ef0f` remains in its ancestry. Rebuild/registration reconciliation
  is required after callable attachment is restored and before fresh task keys
  are allocated.
- Pumas remains at `05855f1b`, its clean coordinator source remains at
  `96f859443460ad8e4799d563528aaba113ccff21`, and Tuldok remains clean at
  `6e9e6ec32d4dd719af0e051baafacecd190864c2`. The unrelated untracked
  `docs/breif/future.md` remains untouched. No delegation, product edit,
  candidate, GPU/browser action, publication, or remote mutation occurred;
  `TIPC-M1` remains `Blocked` pending exposure of the existing named servers'
  callable operations.
