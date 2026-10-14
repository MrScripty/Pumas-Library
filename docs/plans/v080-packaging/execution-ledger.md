# Execution ledger

## 2026-10-14 — scoped admission

Accepted checkpoint/main is `055222931100eeedc43307271dbf7c504787888d`;
worktree clean, original readiness branch and receipts retained. Created the
separate scoped branch. Read current release policy, artifact plan, affected
staging/verifier/metadata tools and current canonical standards before source edits.
The parent explicitly admits this implementation under `docs/plans/v080-packaging/plan.md`.
The design is one serial preparation slice; independent review remains parent-owned.

Confirmed full desktop jobs do not provision ORT, the existing verifier is
Linux-specific (copy command, environment, mapping, signal), and the old metadata
helper covers only two full Linux installers. Existing tag-only jobs can be
reused behind one explicit manual build-only boolean rather than redesigned.

Official Nomic model card declares Apache-2.0; verified public weight pointer
at commit `ac6fcd72429d86ff25c17895e47a9bfcfc50c1b2`: FP16 is 273,859,028 bytes,
SHA-256 `cf5b5a86edb00f895561803cfc04729090a958340b8ca2ad76c143f565f6bb04`.
Executor metadata request to HF failed with proxy tunnel 403 before any model
bytes. Browser public-source observations are metadata evidence only; no model
download or inference is claimed.

Before editing the no-inference packager, admitted its directly affected config:
it inherits full native-resource filters, so a stale developer staging directory
could otherwise copy an unused runtime into that variant. Restrict that variant's
backend resource entry to the two platform RPC filenames. This preserves the
existing backend/variant contract and prevents the same packaging failure family.

Before extending dispatch guards, bounded the new build-only mode to the actual
sixteen-artifact plan. Separate review archives/dependency probes would duplicate
its native producer or qualify unavailable speech. Existing manual Torch E2E
provisions a non-shipped sidecar cohort and is not an assembly dependency. Omit
those three jobs only in the new candidate mode; retain ordinary PR/manual/tag
semantics. This avoids unrelated runtime downloads without redesigning jobs or
changing release artifact/acceptance support.

## 2026-10-14 — implementation and local verification

Implemented explicit official ORT staging/probing in full desktop backend jobs,
including exact pinned notices and no-inference resource exclusion. Staging
validates all native inputs before replacing its generated output. No-inference
configuration retains its other shared resources and variant names.

Extended the existing positional packaged verifier with Python file copying,
isolated native environments, Windows process-group/CTRL_BREAK shutdown and
unconditional disposal after failed signals. Linux retains device/inode mapping
proof. Windows PSAPI and macOS lsof record the narrower native module path/current
byte observation under a distinct result label; mapped-file-object identity and
complete system closure stay unqualified. Microsoft PSAPI function signatures
were read from official documentation. Old macOS CI job 115744179824 reports
`macos-15-arm64` and Rust 1.92.0 aarch64; that resolves runner architecture for
design selection, not acceptance of the new implementation.

Reused the existing native inference matrix for explicit untrained model
lifecycle execution in candidate/tag runs. Ordinary native QA now runs the source
contracts including actual Windows/macOS OS-observer self-process checks. This
Linux executor's skip is explicit. No trained weights are downloaded by CI here.

The complete candidate metadata generator creates sixteen artifact-level SPDX
inventories, sixteen provenance subjects, combined source/native notices, project
license and checksums covering every other generated final file. SPDX does not
claim extracted-file or exhaustive native component analysis. The old two-Linux
extracted-file helper remains usable separately. The synthetic 16-file test
produces 35 checksummed files; those bytes are non-installable test fixtures.

Validation:

- Python focused suite: **20 passed, 1 native Windows/macOS-only skip**, 0 failures.
  Corrupt staging preserves a prior sentinel; missing/ambient modules, forced
  stop/signal errors, absent/extra/empty/symlink artifacts and stale attribution
  cannot produce accepted output. Native OS module enumeration is not mocked
  when its self-process test runs on the selected host.
- Full Node release suite: **37 passed**, 0 failures/skips. Initial run was
  32 passed/1 failed: its old no-inference assertion required the full native
  resource filters. Replaced that obsolete expectation with exact RPC-only
  exclusion plus preservation of all other shared resources. Added actual
  source-guard execution against the real Git commit and wrong/missing inputs.
- Ruff 0.15.2 (the existing dev requirement): release-script check and formatting
  passed. Checksum-pinned actionlint 1.7.8 passed with shellcheck/pyflakes disabled
  locally because those plugins are unavailable; CI retains its full plugin gate.
- Both canonical default/S3 notice inventories regenerated. Notice texts remain
  byte-identical; only the Electron package source hash changed. Freshness,
  dependency ownership and version alignment (0.8.0-rc.1) passed.
- Main remained `055222931100eeedc43307271dbf7c504787888d`; no open Pumas PR was
  reported before creating this slice's draft. Both lockfiles and the two earlier
  candidate/qualification receipts retain their exact hashes.

Logs and hashes are indexed in [the receipt](reports/verification.json).
Full native artifacts, trained execution, CI outcome and independent cumulative
review are not invented. Parent owns review of the concrete candidate and exact
build-only dispatch; this task has created no tag, release or full rehearsal run.

During staged self-review, strengthened native notice coverage by role and every
target pin. That exposed a real representation difference: Linux/macOS LICENSE
and third-party texts use LF, Windows uses CRLF. The stricter first run had
18 passed/1 error/1 skip, retained in the receipt's earlier attempts. Verified
that CRLF reconstruction has exactly the Windows published sizes and digests
(1,094 and 331,175 bytes). The generator now emits every distinct exact pinned
representation, rejects a project license substituted for third-party terms,
and refuses any unrepresented platform's notice pin. No legal wording is guessed
or edited; each emitted variant is authenticated by its own role/target digest.
The complete suite then passed as above. One intermediate Ruff invocation used
the wrong working directory and failed before formatting/tests; the corrected
root invocation passed. The 09:22 environment warning did not lose work: a fresh
09:23:29 UTC git/date/read check succeeded, followed by these actual tests.
