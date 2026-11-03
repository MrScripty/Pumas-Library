# Unexplained vision fixture request count

The first RPC package run during deletion-scope validation failed the
`seventh_descriptor_requires_selected_semantics_supported_profile_and_live_props`
case `managed_without_session`. Its expected availability refusal passed, but
the fake backend's request collection was nonempty. The initial assertion did
not include the URI or whether the request preceded the capability query.
The exact request origin is therefore unavailable.

The production vision validator, transport-session guard and fixture setup
were unchanged by the deletion repair. That excludes an edit to those files;
it does not prove that timing or other changed behavior had no effect. Possible
origins considered were background fixture work, profile setup before the query,
or an improperly ordered capability probe. No cause was established and the
failure is not classified as flaky.

The focused invocation passed (one test, 6.03 seconds), the labelled invocation
passed, and its concurrent vision cohort passed eight tests. A final full RPC
package run passed 438 tests with 12 existing opt-in skips, including all 410
binary unit tests. These are observations of those runs, not proof that the
earlier counterexample is resolved. Logs remain in original-repository
`tmp/pr69-delete-scope-rpc-all.log`, `pr69-delete-scope-vision-repro.log`,
`pr69-delete-scope-vision-labelled.log`, `pr69-delete-scope-vision-cohort.log`
and `pr69-delete-scope-rpc-labelled.log`.

The only test change snapshots requests immediately before the capability
query and includes that snapshot plus all observed request URIs/bodies in the
existing failure assertion. The cases, expected availability reasons, zero-call
requirement and positive live-probe requirement remain intact. All captures
come from synthetic localhost fixtures. No sleep, retry inside the test,
background shutdown, assertion relaxation or production validator change was
introduced. Preserve this as an unresolved request-origin issue if it recurs;
the added evidence will distinguish the candidate origins. Native model
inference remains untested. This diagnostic is separate from model deletion
and download-location reconciliation.
