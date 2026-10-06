# Prefix byte-capacity review repair

Independent review identified that transport XML-byte overflow was collapsed into
`S3ReaderError::Protocol`, despite the prefix API's typed capacity outcome.
Original prefix and successful RPC recovery remain preserved at
`b5c97e8e3440e3fc11cfec806bee1af178e024e4`, tree
`96e1df067881873b55f024eca796163b4dd9ae34`. Separate formatting repair
`6f260de60a90f379a5e47db3d1edccc8579e953f`, tree
`8bf31066dbe309c5032880cfec2e7b3b9b5403d5`, applies pinned Ruff0.15.2 only to
the two changed release Python files; their ASTs are unchanged. Full Ruff lint
passes and those two formatting checks pass. Full formatting retains an
unchanged-baseline failure in `qualify-s3-installed.py`; exact base bytes and
failure evidence are recorded, without weakening the CI check.

## Repair and retained behavior

A private static `ListingByteBudgetExceeded` error travels in the successful
listing body stream as its original boxed typed cause. The SDK's existing error
source chain preserves it; a prefix-only mapper returns
`S3PrefixError::Incomplete("XML byte bound exhausted")`. The defensive XML guard
uses the same marker for byte overflow. This does not match error text, expose
provider diagnostics or add mutable side-channel state. Malformed XML and other
reader/protocol errors keep their existing classification. Error-response bodies,
HEAD/range reads, source identity, credentials, retry/timeout policy, verification,
manifest/receipt formats and protected native/ONNX source remain unchanged.

## Regression evidence

Three strengthened regressions fail on the old implementation: completed valid
XML one byte over the per-page bound, a second valid page one byte over the
remaining total bound, and overflow before body EOF. Actual red result: 13 pass,
three fail and one child helper ignored; evidence `prefix-budget-classification-red.log`.

The repaired full S3 unit set passes 30 tests, with two subprocess helper entries
ignored and exercised by parent tests (`prefix-budget-classification-green.log`).
Exact per-page and aggregate boundaries succeed with checked VersionIds and byte
counts; one-over returns typed Incomplete and no HEAD pin/shortened selection.
Malformed XML within capacity remains Reader(Protocol). Unfinished overflow still
closes its connection before the caller deadline. Existing signed/anonymous/token,
TRACE redaction, receipt identity and cancellation regressions remain passed.
All 66 S3 integrations and strict all-target core Clippy pass. Production RPC
Clippy passes with the inherited dead-code allowance; the exact default-plus-S3
RPC suite passes 302 units, 20 integrations and two intent integrations, with
12 existing ignores unchanged. No active qualification remains for this repair.

Logs are under `/workspace/scratch/s3-prefix-main95/`: `prefix-budget-` repair
logs, `release-ruff-format-only.json`, `release-ruff-check-repair.log`,
`release-ruff-format-repair.log` and `release-ruff-unchanged-baseline-failure.log`.
No dependency or source-authentication API change is introduced. Parent retains
PR/review/publication/integration authority. The next independent Q4 candidate
is installed default-plus-S3 build provenance and complete S3 notice packaging;
provider/platform/inference acceptance remains separate.
