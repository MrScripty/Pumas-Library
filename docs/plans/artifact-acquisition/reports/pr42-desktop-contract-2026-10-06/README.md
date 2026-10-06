# PR42 desktop generation repair evidence

Exact tested source is `75d7e426acdb4f2ed25c6063653ae517a56528aa`, tree
`93018997501ae9ec503508028767345298828abc`, based on frozen PR42
`722245514b3bae42511aa5ea188570292ce276d4`. This successor adds evidence only;
source 75d7 remains unchanged. `repair-evidence.json` preserves the original
command/result/source/hash record; `commands.json` identifies observed exits
and retained setup failures. All named hashed files are included unchanged.

The hosted and local stale-check failure and working-directory regression are
retained. Final generation, strict byte parity from root and Electron working
directories, generator 9/9, actual producer Electron conformance 41/41, bundled
preload renderer conformance 48/48, Electron build/lint and syntax/diff checks
passed. All six normally regenerated outputs equal the source base byte for byte.
The checker is unchanged. No dependencies, authentication, network policy, API
or schemas were changed. Local execution and Git push work; shell gh reads were
Forbidden, while authorized GitHub connector reads succeeded. Hosted repair
qualification and PR42 advancement belong to the parent. AC10 remains separate.
