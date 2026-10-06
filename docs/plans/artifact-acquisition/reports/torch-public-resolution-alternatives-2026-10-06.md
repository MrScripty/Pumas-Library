# Q2 supported-public-tooling resolution alternatives — 2026-10-06

**Recommendation:** first implement the existing finite qualified recipe as
verified, preflighted exact local wheels. Retain public subprocess pip for local
consumption. Do not restore unrestricted automatic/retained-preview resolution
until its candidate admission design is implemented and qualified. No private
hook activation or contract exception is authorized or recommended here.

## Starting point and disposition

Research branch `docs/torch-public-resolution-alternatives-1abcf960` starts at
`1abcf960ba9bf0e4504aef505d24f9b78a9e89ca`, tree
`8816ef0e6124a60d3ed8acc38ceeacf5d98a41ee`. Production source, original private
prototype/evidence, contract, native cleanup, recipes/pins and dependencies stay
unchanged. Parent explicitly retained contract §11. This successor records
research and local controls, not a completed source repair. The two exposed
resolver modes remain unqualified. No package provider or synthetic external
URL was probed, no source archive inspected/prepared, and no hook/backend or
installation executed. Documentation was retrieved from primary maintainers.

## Supported foundations and their limits

Pip supports invocation as a subprocess; its documentation explicitly excludes
internal Python APIs from the supported integration surface. There is no
supported pip candidate-admission or resolver Python API in that contract.
[Pip programmatic-use contract](https://pip.pypa.io/en/stable/user_guide/#using-pip-from-your-program).

A wheel is a ZIP with Core Metadata in `.dist-info/METADATA`. Read it as bounded
data without extraction, import, entry-point loading or backend execution.
Standalone `packaging.metadata.Metadata.from_email(..., validate=True)` validates
fields; public `Requirement.url` distinguishes direct references. Use public
name/version/specifier/tag/marker APIs for interpreter compatibility and closure.
These parsers do not retrieve packages or solve dependency graphs.
[Wheel format](https://packaging.python.org/en/latest/specifications/binary-distribution-format/),
[metadata API](https://packaging.pypa.io/en/stable/metadata.html),
[requirements API](https://packaging.pypa.io/en/stable/requirements.html),
[tags API](https://packaging.pypa.io/en/stable/tags.html),
[marker API](https://packaging.pypa.io/en/stable/markers.html).

Simple Repository APIs advertise filenames, hashes, Requires-Python, yanking and
optional PEP658/714 metadata sidecars. Sidecars can screen candidates without
loading distributions. They are optional, and their hashes authenticate the
sidecar bytes, not the whole wheel. Before offering a wheel to pip, inspect the
actual acquired wheel's metadata and bind it to the screened metadata/identity.
When a sidecar is absent, acquire only an explicitly approved wheel and read its
ZIP metadata as data; never fall back to source metadata preparation.
[Simple API](https://packaging.python.org/en/latest/specifications/simple-repository-api/#serve-distribution-metadata-in-the-simple-repository-api),
[PEP658](https://peps.python.org/pep-0658/).

`importlib.metadata` supplies distribution metadata access, not index discovery
or a solver. Its ordinary APIs target discoverable installed distribution data;
installing a candidate to inspect it would violate the ordering needed here.
Bounded ZIP reads plus packaging are the direct pre-install route.
[Python metadata API](https://docs.python.org/3/library/importlib.metadata.html).

Existing helpers import `pip._vendor.packaging`. That import path is not the
standalone public packaging distribution. A new public-only preflight must use
an explicitly provisioned/bundled standalone tooling dependency or an admitted
maintained equivalent; it cannot silently rely on pip's private vendored path.
The recipe's eventual packaging installation does not make preflight tooling
available before installation. This research adds no dependency/provisioning.

## Alternatives and proportionality

| Alternative | What remains with maintained tooling | Pumas responsibility / cost | Disposition |
| --- | --- | --- | --- |
| Existing finite recipe, exact local wheels | Packaging validates metadata/requirements; public pip consumes exact files with `--no-deps` | Materialize the unchanged pin/hash set, preflight every selected wheel, validate marker/extras closure; no general solver | Smallest bounded next source slice |
| Finite prevalidated wheelhouse, public pip resolution | Pip retains dependency solving/backtracking on named dependencies | Validate every offered wheel and all input requirements before pip; construct and bound the complete candidate universe | Good when a genuinely finite curated catalog already exists; dynamic discovery is additional work |
| Standalone resolvelib with metadata-only provider | Public Resolver owns backtracking; packaging owns syntax/semantics primitives | Provider owns candidate discovery, source admission, ordering, tags, extras/markers, yanking/prereleases, hashes, bounds and provenance | Supported API, but larger maintenance scope; not the minimal repair |
| Public `uv pip compile` CLI | Maintained resolver documents direct-URL binary enforcement | New pinned executable/platform/tool custody, config/environment isolation, output adapter and selection-policy qualification | Worth a separate bounded evaluation for dynamic selection; not a drop-in parity repair |

### 1. Finite qualified recipe: avoid unnecessary resolution

The unchanged lock has **66 entries: 63 exact `==` named pins and three direct
wheel roots** (Torch, torchvision, Nunchaku). Every entry has SHA256 allowlist
material. Its exact bytes are 84,913; SHA256
`05c4ba8556998179cac8db5798177500e1154caca3d9164e00455240dab4a07e`.
Recipe scope is CPython3.12/Linux x86_64/Torch2.9.1+cu130. This inventory is source
inspection; provider wheel availability and actual METADATA remain unexamined.
See [inventory](torch-public-resolution-alternatives-2026-10-06/recipe-inventory.json).

Generate a candidate catalog for only these pins: approved compatible wheel
filenames/URLs and lock-allowed hashes, including the three already pinned roots.
Use the existing acquisition owner for wheel bytes. Inertly validate each wheel
before any pip resolver invocation, reject all dependency direct references
(including inactive marker/extra branches), and check the fixed selected versions
satisfy active named dependencies/extras to a fixed point. Preserve the original
lock/recipe and source catalog as provenance; do not treat the lock's multiple
hashes as proof of one uniquely selected artifact. If several allowed builds
remain, preserve documented wheel ranking or report ambiguity; do not invent a
new selection or silently substitute a same-name/version wheel.

Once the exact closed set is accepted, the existing acquired-input/local helper
can consume it using explicit local file/hash requirements and `--no-deps`.
That flag is valid because Pumas has already verified the full fixed closure;
it is not used to conceal unresolved dependencies. An optional public local
`--dry-run --no-deps --report` can give a version-checked pip identity report for
those explicit inputs; it does not independently prove closure. Retain this
local report as local evidence and the source catalog/lock separately. Do not
fabricate a remote pip report or rewrite original recipe provenance.

This route needs no general dependency solver. It cannot silently stand in for
other releases, platforms, builds or automatic interpreter selection. Keep
current bundled GPU/live-sidecar validation, acquired child leases, receipts,
no-fallback and post-probe publication checks. New implementation admission must
cover exact catalog materialization, standalone tooling availability, total
resource bounds and acquisition lifetime before changing source.

### 2. Prevalidated wheelhouse: the precondition does the work

`--no-index` disables package index lookup; it does **not** revoke explicit
references. `--find-links` can itself point to remote HTML or contain source
archives. A wheelhouse is closed only if every possible candidate is validated
before pip runs and the finder inputs expose only owned validated wheel files.
[Pip CLI options](https://pip.pypa.io/en/stable/cli/pip_install/#cmdoption-no-index).

Required preconditions: canonical owned directory, finite bounded regular wheel
files; retained custody and unchanged bytes; validated wheel/METADATA identities,
tags and Requires-Python; no dependency URLs anywhere; only validated named or
exact local-wheel root inputs; no requirements includes/editables/projects/VCS/
remote find-links; existing child-only configuration isolation; an owned empty
cache or public `--no-cache-dir`, and disabled version checks. Run public pip
`install --dry-run --ignore-installed --no-index --find-links <owned-directory>
--only-binary=:all: --report <owned-report>`. Validate every selected report entry
against catalog bytes, then pass the accepted closure to the existing final
local installation path. This is application source closure, not OS egress proof.

Validating just the Torch root or final selection is insufficient: pip may inspect
an unselected candidate while backtracking. For an unrestricted release, a
complete finite candidate catalog is not supplied today. Recursively discovering
and validating every reachable candidate through index/PEP658 metadata introduces
catalog expansion limits, cycles, alternatives, missing metadata and ordering
semantics. Bounded refusal must stay inconclusive rather than masquerade as proof
that an older interpreter/build is required. This is the main cost of extending
the wheelhouse approach beyond the qualified recipe.

Public `pip download` performs resolution too. A whole-closure download does not
solve the original gap. `download --no-deps` on prevalidated explicit wheel roots
can be wheel-only, but named dependency traversal then belongs elsewhere; it is
not an automatic-closure resolver. It also does not replace the existing shared
acquisition owner for final payload.
[Pip download contract](https://pip.pypa.io/en/stable/cli/pip_download/).

### 3. Resolvelib: public seam, larger package-owner responsibility

Standalone `resolvelib.Resolver(provider, reporter).resolve(requirements)` is the
intended public integration. Provider methods include identify, preference,
find_matches, is_satisfied_by and get_dependencies. A provider can refuse source/
VCS/direct requirements before any retrieval because its candidates are inert
validated metadata, without pip monkey-patching.
[Maintainer README](https://github.com/sarugaku/resolvelib#intended-usage),
[provider contract](https://github.com/sarugaku/resolvelib/blob/main/src/resolvelib/providers.py).

Resolvelib does not supply PyPI/Torch discovery or pip package semantics. Those
remain provider responsibilities, including candidate ranking and backtracking
state for extras. Its maintained wheel-provider example explicitly leaves
compatibility-tag handling incomplete; it is an illustration, not a production
provider to copy. Use packaging APIs and a qualified provider if choosing this
route. Do not import `pip._vendor.resolvelib`. A maintained dependency pin/license/
platform decision and semantic regression suite would be required before use.
[Maintainer wheel example](https://github.com/sarugaku/resolvelib/blob/main/examples/pypi_wheel_provider.py).

### 4. uv: stronger documented flags, still needs scoped qualification

uv documents enforcement of `--only-binary :all:` on direct URL dependencies,
unlike pip. Its CLI documents `--no-build` as an alias, with editable exceptions
and reuse of previously built cached wheels. Reject editable/source root inputs
and keep cache policy explicit. `--no-sources` only ignores the tool.uv.sources
table; it is not a blanket prohibition on requirement URLs. These are documented
CLI semantics, not an experiment performed here.
[uv compatibility](https://docs.astral.sh/uv/pip/compatibility/#only-binary-enforcement),
[uv compile options](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile).

uv also documents different index and resolution behavior and restrictions on
transitive URL requirements. Changing resolver can change a valid selected
closure, so preserve policy explicitly rather than assert pip-equivalent
identity/fallback. The lock was generated by uv during recipe qualification,
but that does not admit provisioning uv into managed runtime selection.
No uv source/VCS command or provider request was run in this research. A public
CLI evaluation is smaller than maintaining a complete custom provider, but the
exact wheel-source boundary and output/custody integration still need fixtures
and explicit admission.

## Direct references and metadata integrity

Recommend initially **rejecting every dependency Requirement.url** at preflight,
matching current final local consumption. Do not replace `name @ URL` with
`name==version`, drop markers or edit METADATA inside the wheel: those operations
change requirement/artifact identity. Constraints and `--no-index` do not map
an original URL to an owned file.

A future mapping requires an explicit source-to-local artifact binding with
original URL, digest, wheel name/version/tags and retained provenance. Public pip
with dependencies enabled still sees the original URL; it cannot be assumed to
use a local mapped artifact. Mapping would need owner-validated exact closure
plus `--no-deps`, or an explicitly defined standalone provider, and a contract
extension because the current local helper refuses these references. Direct
sdists/VCS/editables remain unsupported; never execute them to infer a mapping.
[Dependency-reference grammar](https://packaging.python.org/en/latest/specifications/dependency-specifiers/).

## Controls, evidence and remaining qualification

Five controls pass on Python3.12.14, standalone packaging26.3, pip26.2.1:

- Real public pip follows an inert local direct-reference wheel outside the
  wheelhouse despite `--no-index`; no remote or source dependency is involved.
- Inert METADATA source/VCS/local/inactive URLs refuse before a subprocess call.
- A prevalidated named-dependency closure resolves wholly from local wheels.
- Missing named closure fails with no report and no alternative source offered.
- Explicit finite local inputs produce a public version1 report with `--no-deps`.

Fixtures contain only METADATA/WHEEL/RECORD, no importable code. The first run's
inactive-reference fixture omitted required whitespace before its marker;
packaging rejected that malformed string before pip. The fixture was corrected
and failed/final logs retained. These controls demonstrate the public boundary
and the necessary preconditions, not production preflight, actual recipe wheel
closure, egress denial, resource/custody integration, receipt settlement or real
Torch/provider/platform/AQ-PACKAGES acceptance. No Rust tests/builds were needed
for this evidence-only change. Ruff and evidence hash validation also pass.

[Tests/logs/inventory manifest](torch-public-resolution-alternatives-2026-10-06/evidence.json)
retain the exact local controls. The public report format is supported and
version-checked evidence, not a lockfile or remote provenance substitute.
[Pip report contract](https://pip.pypa.io/en/stable/reference/installation-report/).

Next action: admit the finite recipe/catalog/preflight source slice with the
unchanged lock and existing shared lifecycle, qualify it, and decide separately
whether unrestricted selection uses a prevalidated catalog, standalone resolvelib
provider or explicitly qualified public uv CLI. Keep the exposed modes blocked
from safety/acceptance claims throughout. No private-tooling exception is needed
for the recommended route; tooling provisioning and candidate admission remain
concrete design inputs before implementation.
