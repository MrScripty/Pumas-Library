# Baseline repair integration

This bounded integration starts at main `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`.
It carries the independently developed milestones below without including the
acquisition branch, changing a real store, or enabling network exposure. The
original slice branches remain available for source and test provenance.

| Order | Source PR and exact head | Included behavior |
| --- | --- | --- |
| 1 | #9 `100552a9b91916944126a5c61d9c83a6d747fd82` | Refuse normalized import collisions, propagate enumeration errors, create exclusively with private Unix permissions, restore source permissions by descriptor |
| 2 | #12 `c748e3922594b54a276f4ac6d539b7f4f0552562` | Validate every explicit shard set and retain full relative paths |
| 3 | #11 `6131fa8a27b50794217e5bd15eed5499c4dc78b7` | Loopback-only CLI and Host/Origin admission, including malformed authority ports |
| 4 | #13 `b2f80213e480d57bc4b2951ec103dee9c7e9d7e1` | Observe Unix termination signals before readiness and wait for existing owned cleanup |
| 5 | #14 `55b77170e441f8ae7c9412c1b01ef25c420e7920` | Refuse implicit execution of custom model repository code |
| 6 | #18 `a805c6f5d6491464d903c52de493255aa3528574` | Trigger the provider timeout fixture after descendant custody is established; refresh unchanged production input attribution fingerprint |
| 7 | #19 `e1517ac3a23114603098e1d0e315f9f8fa4e78d6` | Serialize primary claims at SQLite write admission and fence failed-startup release by claim identity |
| 8 | #20 `88639b47f89d60dc2b0a9bad75cb6c9ef5589500` | Bound WAL-mode busy retries and preserve exhausted/non-busy errors |

Each source head passed its ordinary hosted Build jobs. That evidence does not
replace qualification of this combined tree. The integration adds explicit native
Linux, macOS, and Windows import/shard/registry tests, local RPC admission tests,
and Unix real-process signal tests to the existing matrix. The Unix restrictive
creation subprocess is intentionally absent on Windows; no Windows ACL parity is
claimed. Manual/tag-only release packaging and real Torch inference remain
separate gates, not implied by an ordinary PR Build.

Before integration approval, require the exact combined head's CI results and a
full review of the combined final diff. Preserve the individual milestone commits
and original PRs until the owner chooses how to integrate/supersede them. This
record is a scope and provenance contract, not an assertion that those pending
gates have already passed.

## Remaining baseline boundaries

- Ordinary import temporary-directory ownership/cleanup needs a separate repair;
  this change does not claim that its ignored cleanup errors are resolved.
- The RPC shutdown acknowledgement, HTTP/SSE drain, and Electron child completion
  contract are being repaired separately. The included Unix signal change alone
  does not close that lifecycle audit finding.
- Custom-code-only model loaders remain unavailable until a repository/revision
  scoped trust contract exists; there is no undocumented global bypass.
- PR7 acquisition work and its admission, resume-state, and native-workspace
  custody repairs remain a separate integration. Legacy/mismatched workspace
  recovery and schema migrations must continue to fail closed.
- Cached Hugging Face search infrastructure readiness is a separate planning
  deliverable. S3 and restricted networking follow baseline qualification; no
  real-cluster or real-service qualification is claimed here.
