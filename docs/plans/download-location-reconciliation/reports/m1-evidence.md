# M1: local recovery implementation evidence

Observed on Linux x86_64, 2026-10-12, in the protected latest-main validation
worktree. Tiny authored fixtures only; no live library, credentials or weights
were copied or repaired. Implementation source is the accompanying recovery commit.

## Storage compatibility before UI expansion

Actual older strict readers from Eidetic's Pumas pin
`a94fd92021f27fdeedb6e2de6e01c41c250ef576` and Lanternwake's pin
`95a0baad2d0aea4650fc36ad4afd969ac9391bf5` each refused an authored HF-6 document
on the new discriminator, leaving metadata unchanged (one test per actual pin).
Their original Cargo lockfiles were retained. The new reader accepts HF-5 and
HF-6; generic acquisition publication round-trips the opaque HF-6 partition.
Offline conversion still emits supported HF-5; explicit local acceptance alone
selects HF-6 and warns the user first.

The actual Lanternwake C# download/serving RPC client files decoded responses
from the current default-feature server on the upgraded fixture. Empty serving
correctly reported model_unavailable. Eidetic's traced consumer uses the direct
Rust library, with no Pumas RPC path. This evidence proves those exact paths,
not older application-wide or inference compatibility. Probe sources, old source
copies and outputs are ignored under original-repository
`tmp/download-resolution-compat/`.

## Exact recovery and refusal evidence

`cargo test --locked --offline -p pumas-library retained_location` and the same
command with `--no-default-features` exercise 2/3/33 selected-file counts through
real catalog, JSON persistence, destination capabilities and restored live claims.
The actual same-process loopback transfer consumes bytes at the original
destination after resolution; stale dormant/generation reservation is refused;
other paused rows and claims remain. This is distinct from a JSON-only assertion.

Focused negatives exercise changed file stats, selected partial/missing files,
ambiguity, missing format consent, wrong attempt, competing candidate admission,
pinned unsupported history, malformed metadata, Unix symlink traversal, native
root contention and deletion of a previewed catalog candidate. Publisher fault
modes cover not-published, visible/parent-sync-unconfirmed and visibility-unknown
results. Until confirmed, both old and candidate targets retain custody; identical
operation retry or the existing startup barrier settles it. Original Error,
SHA evidence, immutable history and released admission survive; no HF receipt
is invented. These focused cases do not close every composed C3/C4/C6 procedure.

## Real developer desktop

`scripts/acceptance/download_location_reconciliation_ui.mjs` drove the actual
renderer/preload/IPC/RPC with Electron 43.7.0 in its normal sandbox and an isolated
fixture/configuration. It verified the 33-file preview, provenance wording,
older-direct-reader warning, unchanged Cancel, native Enter-key confirmation,
immediate removal of the obsolete activity, preserved filesystem identity/stat
and normal window close/reopen with both unrelated paused downloads unchanged.
Both processes exited zero. This uses the available Linux desktop/display session;
it is not a Windows/macOS or virtual-display qualification.

Outputs: original-repository `tmp/pr69-reconciliation-desktop-final.log` and
`tmp/reconciliation-desktop-fixture-6/`. Fixtures contain seven-byte authored
weights only. Inference and the user's actual LLaDA readiness remain untested.
