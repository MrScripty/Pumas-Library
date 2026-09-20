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
