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
  to Pumas planning; the Pumas contract now states that the general deadline rule
  is inapplicable to generation.
- No production implementation or runtime mutation was performed.
- Validation: TIPC-01 through TIPC-12 each have one evidence owner, reciprocal
  plan references agree, and no prospective Pumas plan assigns an elapsed
  deadline to generation.
