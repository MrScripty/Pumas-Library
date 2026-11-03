# PR #69: audio dependency installation

## Observed failure and cause

[CI run 38095339520](https://github.com/MrScripty/Pumas-Library/actions/runs/38095339520)
at `98eb3baf0ccae6e455bf6d0a7017626cae62bf1c` retained installed
runtime metadata. Comparison against the unchanged source recipe showed the same
68 package names, matching managed Python distribution/provider and 66 identical
dependency artifacts. NumPy resolved to 2.5.4 instead of 2.5.3; narwhals resolved
to 2.27.1 instead of 2.26.0. Their URLs and artifact hashes also differed.

The installer resolved dependencies from indexes and wrote its lock afterward.
Agreement among that report, resolution and lock recorded the selected downloads;
it did not establish agreement with the existing candidate recipe. The rejection
was a reproducible dependency-selection error, not evidence of damaged files.

## Repair and revised policy

The existing source-recipe owner derives version constraints for all 68 entries.
The normal staged installer passes those constraints through the Python resolver
to ordinary index-based pip installation. Before staged package publication, it
requires every artifact's name, version, URL and SHA-256 to match the recipe.
No second pin list or replacement direct-URL dependency installation is used.
This exact recipe applies only to its existing Linux x86_64 GNU CPU/Python 3.12/
cohere-asr/Torch candidate; other selections retain their existing policy.

The user subsequently selected the project-wide
[acquisition checksum policy](../plans/acquisition-checksum-policy.md): verify a
supplied SHA during acquisition, retain the evidence, and check contents later
only on explicit request. This supersedes the earlier proposal to preserve the
final installed-byte gate. Installed interpreter/tree/model SHA checks and the
indirect content-derived model correlation gate have been removed. Path custody,
required members, sizes, package selection, confinement and lifecycle checks remain.
The source recipe and supplied archive download hashes were not loosened.

The qualification example performs a cold installation in an initially empty
private root, retains phase evidence, removes only that managed runtime through
the public version-removal operation, then reinstalls with the same warm cache.
Both phases run the confined dependency probe and must select identical package
and managed-Python identities. CI retains both phases' metadata and probe reports.

## Local cold/warm result

On 2026-10-11, the normal installation and qualification example exited zero in
`tmp/ci-audio-coldwarm-20261011-retry` beneath the original repository. Both phases
selected all 68 recipe artifacts with matching name/version/URL/SHA evidence and
matching managed-Python distribution/provider. Independent metadata comparison
confirmed those identities. Both confined probes reported `status=passed`,
`child_tree_drained=true` and `production_available=false`: Torch 2.10.0+cpu,
Transformers 5.4.0, dependency imports and a CPU operation only. No model inference
or ASR was run. The path contains spaces and is supported by the revised policy.

An earlier attempt ran out of disk while pip acquired Torch. Its failure evidence
was retained in `tmp/ci-audio-coldwarm-20261011`; the user freed space before the
successful fresh-root retry. This was an environment failure, not a code repair.
The earlier 2026-10-10 evidence is also preserved. These directories contain
private runtime dependencies and evidence, not copies of library models.

Before repair, the constraint-forwarding regression rejected the unknown CLI
argument and the publication regression accepted consistently changed metadata.
The revised tests require constraint forwarding and refusal of changed package
name, version, URL or hash, including the observed NumPy drift. Current aggregate
verification and compatibility limits are recorded in the checksum-policy plan.
A hosted rerun remains required; local Linux checks do not establish Windows or
macOS behavior. Download reconciliation remains a separate, pending plan.
