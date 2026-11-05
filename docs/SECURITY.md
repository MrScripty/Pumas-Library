# Security

## Reporting a Vulnerability

Do not open a public issue for a suspected vulnerability. Report it through
the repository's [private vulnerability reporting](https://github.com/MrScripty/Pumas-Library/security/advisories/new)
form, or contact the maintainer privately.
Include affected versions, reproduction steps, impact, and any known mitigation.

Only the latest development/release line receives security fixes unless a
maintainer explicitly announces otherwise.

## Current Trust Boundaries

- The Electron renderer is sandboxed and isolated from Node. Privileged actions
  cross the preload/main IPC boundary.
- Electron validates RPC method names and some request shapes before forwarding
  to the Rust sidecar. Coverage is incomplete; renderer TypeScript types are not
  a security boundary.
- `pumas-rpc` refuses to bind any non-loopback address and limits body size
  and concurrent requests.
- `pumas-rpc` has no LAN mode. Its receiving HTTP boundary rejects
  non-loopback Host values, non-loopback or malformed Origin values, duplicate
  authority/origin headers, and originless cross-site browser requests before
  dispatch. CORS controls response visibility and is not authorization.
- Direct local clients without browser headers remain supported. These checks
  do not authenticate hostile native processes with the same local privileges;
  a future remote node interface needs a separate authenticated/encrypted contract.
- The Torch sidecar is loopback-only by default. Non-loopback use requires both
  `PUMAS_TORCH_ALLOW_LAN=1` and `PUMAS_TORCH_API_TOKEN`; all routes except
  `/health` then require the token.
- Model paths, import roots, URLs, archives, and generated runtime commands are
  privileged inputs and must be validated at their receiving boundary.

## Sensitive Data

Hugging Face tokens and other secrets must never appear in logs, errors, URLs,
or diagnostics. `pumas-rpc` does not log request parameter values, including in
debug mode. Request diagnostics use an allowlisted method label, a numeric
request ID when present, and a stable outcome class/code.

JSON-RPC, typed HTTP and RPC-owned event errors pass through the Rust
producer's public error projection (`pumas-rpc/src/public_text.rs`). Callers
branch on its stable code and class. Its message carries the failure reason so
the local user and consuming applications can act on it (owner decision,
2026-11-02). Messages are bounded to 1,024 characters and single-line;
credential-shaped words, secret `key=value` pairs, URL user info and URL query
strings (which can hold signatures) are redacted; and the home directory is
shown as `~`. Messages may include other local paths and query-free URLs, which
the receiving boundary already restricts to loopback clients of the same user.
Raw subprocess output is not forwarded. New transports must reuse the same
projection rather than serializing errors directly.

## Dependency and Release Evidence

Authoritative dependency inputs are the checked-in Cargo and pnpm lockfiles plus
the Python requirements under `torch-server/`. Managed Torch runtimes are
optional add-ons: each install resolves current package versions when it is
created, and Pumas ships no lock for them, so their advisories belong to the
installed versions rather than to a Pumas release. Audit an installation's
`requirements.txt`, `resolution.json` and `pip-resolution.json`.
`torch-server/runtime/verified-runtimes.json` records verified combinations as
evidence and installs nothing. The one exact pin is the Cohere audio candidate
recipe (`rust/crates/pumas-core/src/runtime_read_source/audio_candidate_recipe.json`):
the confined audio read boundary admits only the files that recipe describes.
`torch-server/requirements.txt` is an unpinned direct-development input, not a
shipped dependency set.

### Dependency scans

The `dependency-audit` job in `.github/workflows/build.yml` runs
`scripts/release/dependency-audit.mjs` on every pull request, push to `main`,
version tag and manual dispatch. It produces the `current-vulnerability-results`
review that [the artifact plan](../scripts/release/artifact-plan.json) requires
for a release. The scanners are pinned: `cargo-deny` by release archive SHA-256,
`pip-audit` by the hash lock `scripts/dev/pip-audit-python.lock`, and `pnpm` by
the root `packageManager`.

| Scan | Input | Gates? |
| --- | --- | --- |
| `cargo-deny` advisories and licenses | `rust/Cargo.lock`: all features, the three release targets, policy in `rust/deny.toml` | Yes |
| `pnpm audit --prod` | `pnpm-lock.yaml`: production dependencies | Yes |
| `pnpm audit` | `pnpm-lock.yaml`: all dependencies | Only for Electron, which is a devDependency but ships in every desktop package; other findings are build-only and recorded |
| `pip-audit` | the exact packages of the Cohere audio candidate recipe | Yes |
| `pip-audit` | `torch-server/requirements-dev.lock` | No: development and CI tooling, not shipped |
| `pip-audit` | package versions in `torch-server/runtime/verified-runtimes.json` | No: install shortcuts that resolve current versions, so they are evidence, not pins |

The scan fails when a gating scan has a finding that is not accepted, when a
gating scan cannot audit one of its packages, or when any scanner fails to run.
On pull requests and pushes to `main` that failure is advisory: the job is
marked failed and the run continues. On version tags and `build_candidates`
dispatches it blocks: `release-candidate` needs the job, and it refuses a
review that did not pass or that scanned a different commit. The results
artifact holds each tool's raw report and a `summary.json` that names the
scanned commit, tool versions, each scan's outcome and every finding.

pnpm reports the `electron/` workspace directory as the npm package `electron`
at the workspace's version. The script drops a finding only when every one of
its paths is a workspace importer at that importer's own version, and lists
each dropped finding with that reason in `summary.json`.

To run the scan locally, install `cargo-deny` and the `pip-audit` lock as the
job does, then:

```bash
node scripts/release/dependency-audit.mjs run --output /tmp/pumas-vuln \
  --cargo-deny /path/to/cargo-deny --python /path/to/venv/bin/python
```

Torch's native test suite (`torch-server/tests_native`) is not run in CI. It
covers Flux 2 and Nunchaku, which need a CUDA Torch build plus diffusers and
model checkpoints. The CPU acceptance install in `torch-native-e2e` does not
include those packages, and a CPU-only subset would only skip.
`pip-audit` covers the Python inputs that Pumas pins; managed installs resolve
their own versions, so audit an installation as described above.

### Accepted advisory exceptions

A gating finding is accepted only with a recorded reason. Rust exceptions live
in `rust/deny.toml` (`[advisories] ignore`). npm and PyPI exceptions live in
`scripts/release/dependency-audit-exceptions.json`. List each one here as well.
Re-confirm each with the release's own scan, and drop it once the dependency
changes.

Last reviewed against `b091ac45` (2026-11-05, cargo-deny 0.20.2, pnpm 10.33.0,
pip-audit 2.10.1). `sprintf-js` 1.1.3 and `http-cache-semantics` 4.3.0 are still
in `pnpm-lock.yaml`, but the scan no longer reports their earlier advisories,
so their rows were dropped.

| Dependency | Advisory | Why it is accepted |
| --- | --- | --- |
| `lru` 0.16.4 | RUSTSEC-2026-0253 (unsound) | Only in `s3` builds, through `aws-sdk-s3` 1.137.0. The trigger is absent: the key is a `String` with no custom `Drop`, and `pop` is never called. Ignored in `rust/deny.toml`. |
| `bincode` 1.3.3, `instant` 0.1.13, `paste` 1.0.15 | Unmaintained notices | Pulled in by UniFFI tooling, `notify` and macro dependencies respectively; no known vulnerability. `rust/deny.toml` does not fail on unmaintained notices. |
| `zip` 2.4.2, `pyo3` 0.23.5 | Dependabot alerts on `pantograph_guard_interop/Cargo.lock` | Test-only harness lock, never built into a release and not scanned by `dependency-audit`. Both come from Pantograph's `inference` crate at the pinned consumer commit, so they move when that pin moves; `pyo3` is only built with `backend-pytorch`, which the harness never enables. |

### Dependency currency

Dependabot (`.github/dependabot.yml`) proposes one grouped minor/patch update
per ecosystem each week (Cargo, npm, pip) and monthly action pins. Each major
version arrives as its own PR and is adopted deliberately, with its own
verification. A lockfile update also needs regenerated release attribution
(`scripts/release/generate-notices.py`).

An older major version is acceptable while it still receives fixes and the
scan reports nothing against it. A gating finding with no fix on the current
major makes the upgrade release-blocking. Known older majors at the last
review: `axum` 0.7, `rusqlite` 0.32, `sysinfo` 0.32, `notify` 7, `governor` 0.7,
`dirs` 5, `uniffi` 0.28 and `rustler` 0.34. The lock also carries several
`rand` and `windows-sys` majors through transitive dependencies.

## Current Security Work

Open security and boundary findings are tracked in the
[current standards audit](audits/current-standards-2026-09-03/README.md).
