# Passeur MCP usability feedback

This report records observed contributor-tool usability, separately from
artifact-acquisition implementation and acceptance evidence. It does not claim
that Passeur has been notified or that any proposed improvement has shipped.

## 2026-09-30 — repository service unavailable

### Reproduction

From the Pumas-Library MCP session:

1. `passeur_agents({ limit: 100 })` and
   `passeur_tasks({ schema_version: 1, limit: 100 })` return argument
   validation errors. The backend reports maximum limits of 4 and 16,
   respectively, while the published tool declarations do not state those
   bounds.
2. Retry with `passeur_agents({ limit: 4 })`. It returns
   `PATH_NOT_FOUND` from `profile.open` without identifying the missing
   profile resource.
3. Retry with `passeur_tasks({ schema_version: 1, limit: 16 })`. It returns
   `SERVICE_PROFILE_CONFLICT` with “Use the same approved profile path for
   clients of this repository service”.
4. Call `passeur_status({})`. The frontend is running, but the repository
   service is unavailable with `SERVICE_PROFILE_CONFLICT`.

No task was submitted or started during this reproduction. Agent availability
could not be inspected because the valid agent-list request failed.

### Suggested improvements

- Publish the actual maximums in the tool schemas so callers can validate
  requests before sending them.
- Make `PATH_NOT_FOUND` identify which profile lookup failed and give a safe,
  concrete recovery action.
- Make profile-conflict diagnostics identify the conflicting approved profile
  selection, or provide a non-sensitive correlation identifier and a supported
  way for the operator to find the required value. The current message says to
  use the same path but does not reveal which path the service expects.
- Return one consistent service-unavailable diagnosis from agent discovery,
  task listing, and status. In this run those calls produced two different
  errors for what status reported as an unavailable repository service.

### Evidence limits

These observations are from one repository/session configuration. They show
the errors returned by the tools in that session; they do not establish the
cause of the approved-profile mismatch or whether another configured profile
would work.

## 2026-09-30 — retry after reported service recovery

After the user reported that Passeur should work, the current session retried
`passeur_agents({ limit: 4 })`,
`passeur_tasks({ schema_version: 1, limit: 16 })`,
`passeur_status({})`, `passeur_prepare({})`, and the coordination identity
read. Agent discovery again returned `PATH_NOT_FOUND` from `profile.open`;
task listing, prepare, and identity returned `SERVICE_PROFILE_CONFLICT`.
Status now reports the repository service as unavailable with that conflict.
No task was submitted. Because the available tools could not provide an agent
ID or bind a coordinated request, this implementation continued through the
authorized GPT-6.1 Sol Medium fallback instead of guessing an agent or
submitting to an unverified worker.

## 2026-09-30 — status after session restart

After the user restarted the session, `passeur_status({})` reported a running
frontend with a new process/build identity, but the repository service still
returned `SERVICE_PROFILE_CONFLICT` and “Use the same approved profile path for
clients of this repository service”. No agent discovery or task submission was
attempted in this check. The user authorized GPT-6.1 Sol implementation until
the repository service becomes available, so the cancellation recovery repair
continues under that fallback.

## 2026-09-30 — fresh repository-service check

In the resumed session, `passeur_status({})` still reports the repository service
unavailable with `SERVICE_PROFILE_CONFLICT`. A valid `passeur_agents({ limit: 4 })`
request still fails at `profile.open`, now including the generic next action to
check configured path/access and preserve the existing namespace; it still does
not identify the unavailable profile. An initial request above the documented
maximum (`limit: 50`) was rejected by validation, confirming the agents bound is
4. No task was submitted, and implementation remains on the user-authorized
GPT-6.1 Sol fallback until the repository service becomes available.

## 2026-09-30 — coordinator connected, contributor discovery unresolved

A later status check changed from `unavailable` to `not_checked`.
`passeur_prepare({})` connected the repository service with open admission and a
ready coordination authority. Task listing succeeded and returned zero tasks;
the coordination identity read also succeeded. However,
`passeur_agents({ limit: 4 })` still failed at `profile.open` with
`PATH_NOT_FOUND`, and connected status reported execution profile/provider/
approval as `not_checked`. No agent ID for the requested Muse Spark contributor
could be verified, so no implementation task was submitted or started. The
service can coordinate/read metadata, but this session still cannot safely
select the requested execution profile.

## 2026-09-30 — resumed check after the Passeur update

The current installed build connected through `passeur_prepare({})` with open
admission and ready coordination authority. `passeur_status({})` reports the
execution profile, provider, and approval as `not_checked`; a valid
`passeur_tasks({ schema_version: 1, limit: 16, offset: 0 })` returned zero
tasks. `passeur_agents({ limit: 4, offset: 0 })` still fails with
`PATH_NOT_FOUND` at `profile.open`, without a contributor identity. No task
was submitted. Implementation therefore continues using the user's GPT-6.1
Sol fallback until contributor discovery works.

This confirms that metadata coordination is ready while execution-profile
discovery is not. The smallest useful recovery signal would identify the
unavailable profile lookup (or expose a safe configured-agent status) so a
caller can distinguish a missing profile from a service-wide failure without
guessing an agent ID.

## 2026-09-30 — updated frontend with contributor lookup still unavailable

The installed frontend changed build identity to
`6a43daa6eec45ddceadea0ba2e2a623d7fc8e81e330237c7762a7f1c63c33f63` and
attached to repository service generation `269decac-59b5-4028-9bac-8d09ee9a35c2`.
`passeur_prepare({})` reports open admission and ready coordination authority;
task listing succeeds with zero tasks. `passeur_status({})` still reports the
execution profile, provider, and approval as `not_checked`.

A valid `passeur_agents({ limit: 4, offset: 0 })` request again fails at
`profile.open` with `PATH_NOT_FOUND`, without returning a configured agent ID.
No assignment was submitted. The requested contributor cannot be selected
reliably from metadata readiness alone. The already-authorized GPT-6.1 Sol
fallback completed the bounded implementation slice; final review then found
three source defects that are being corrected before verification. Passeur has
not yet contributed source work in this session.

This update narrows the earlier feedback: coordination reads now work, but
contributor discovery still needs to expose which configured profile lookup
failed or provide a safe status/readiness result, and execution-profile,
provider, and approval readiness need explicit values before binding a task.

## 2026-09-30 — contributor availability rechecked after user update

After the user reported that Passeur was available and requested Muse Spark 1.3
Contributor implementation, this session ran `passeur_status`,
`passeur_prepare`, `passeur_agents({limit:4,offset:0})`,
`passeur_tasks({schema_version:1,limit:16,offset:0})`, and coordination identity
and status reads. The installed frontend remains build
`6a43daa6eec45ddceadea0ba2e2a623d7fc8e81e330237c7762a7f1c63c33f63`; the repo
service connects with open admission and `passeur_prepare` reports coordination
ready/held. Identity reads succeed and the task list is empty. However,
`passeur_agents` still returns `PATH_NOT_FOUND` at `profile.open`, and the
expected project profile path
`/home/jeremy/.config/muse-bridge/projects/6aaae9e5ae2b753918ac7478.json` does
not exist. Separately, `passeur_coordination` status reports `not_enabled`,
which conflicts with the `passeur_prepare` readiness report. Execution profile,
provider, and approval remain `not_checked`. No agent ID was returned and no
task was submitted or started.

The readiness output should distinguish repository-service connection,
metadata-coordination enablement, project-profile existence, and contributor
execution/provider readiness. Agent-list failure should identify the missing
profile resource (or expose a safe status field) instead of returning only
`profile.open`; the current state is not sufficient to choose an agent safely.

## 2026-10-01 — coordinated contributor preflight

The service status still reports connected/open admission and ready/held
metadata coordination. A valid agent-list request
`passeur_agents({limit:4,offset:0})` again returned `PATH_NOT_FOUND` at
`profile.open`; the task list returned zero tasks. `passeur_prepare({})`
completed, but did not enable coordination for execution: after correcting a
caller-side `target_ref` validation error (the request must use a
`refs/heads/...` ref), `passeur_preflight` returned
`COORDINATION_NOT_ENABLED` with “Coordination has not been explicitly
initialized.” No assignment was submitted. The tool guidance reserves
initialization for the operator CLI, so the caller cannot safely recover this
state by guessing an agent ID or writing profile/coordination data.

The current errors expose two distinct setup problems: configured contributor
discovery cannot open the project profile, and coordinated admission says the
coordination metadata is uninitialized despite status/prepare reporting it
ready. Readiness should make these states consistent, and preflight should
identify the supported operator recovery path without implying that successful
prepare enabled execution.

## 2026-10-01 — status ready, but contributor discovery still fails

After the user requested Sol 6.1, I rechecked the installed Passeur frontend
build `6a43daa6eec45ddceadea0ba2e2a623d7fc8e81e330237c7762a7f1c63c33f63` and
repository service generation `e1e32585-e963-495d-b46b-05102aff09f1`.
`passeur_status({})` reports the service connected, repository binding valid,
and coordination ready/held. The valid task-list request
`passeur_tasks({schema_version:1,limit:16,offset:0})` succeeds with zero tasks.
The valid agent-list request `passeur_agents({limit:4,offset:0})` still fails
with `PATH_NOT_FOUND` at `profile.open`; execution profile/provider/approval
are not reported ready by this endpoint. There is no returned Muse Spark agent
identity and no active Passeur task, so no Passeur subagent is working on the
implementation. No task was submitted. Work continued directly under the
user's Sol 6.1 instruction.

The failure has narrowed to configured-agent discovery: service connectivity
and task inspection work, but `profile.open` prevents safe agent selection.
Keep these readiness states separate in status output and return the missing
profile resource or a safe configured-agent status instead of a generic path
failure. No product correctness or acceptance conclusion follows from this
MCP observation.

## 2026-10-01 — Muse contributor lookup during parallel implementation

Seven GPT-6 Luna XHigh CLI orchestrators checked Passeur while handling
separate Q1 write sets. A valid `passeur_models` request returned the model
`muse-spark-1.3-contributor`, but valid `passeur_agents({limit:4,offset:0})`
requests repeatedly failed with `PATH_NOT_FOUND` at `profile.open`. Coordinated
submission also reached that profile failure after the caller corrected its
`target_ref` to a full `refs/heads/...` ref. No Passeur task was admitted or
started; implementation used the authorized GPT-6.1 Sol Medium fallback.

This confirms that model-catalog visibility is not evidence that a configured
contributor profile is discoverable or spawnable. Passeur should expose these
states separately and return a safe, actionable agent-discovery result that
identifies the missing profile resource or its readiness state. A caller should
not have to infer whether the model is unavailable, the profile is missing, or
the execution provider is not ready from the generic `profile.open` path error.

## 2026-10-01 — bounded recovery-fixture assignment attempt

For the orphan-partial recovery fixture, the current session retried the
contributor route after the user had authorized Muse Spark 1.3 Contributor for
implementation. `passeur_tasks({schema_version:1,limit:16})` returned zero
tasks; agent discovery failed at `profile.open` with `PATH_NOT_FOUND`; and a
scope-aware coordinated submission for `work/acquisition-q1-http` failed with
the same profile error. No Passeur task was admitted or started. The bounded
fixture/documentation change therefore used the authorized GPT-6.1 Sol Medium
fallback and was independently reviewed by GPT-6.1 Sol High.

This is a separate contributor-profile failure from task-list availability:
the task list is readable and empty, but execution assignment cannot resolve a
configured contributor. Keep the model catalog, project profile, coordination
readiness and execution-provider readiness distinct in diagnostics. A
successful task-list read must not imply that a requested contributor can be
selected or that a coordinated task was accepted.

## 2026-10-01 — coordinated Muse task and terminal-disposition recovery

After the user reported Passeur availability, the operator CLI initialized the
coordination metadata and `passeur_agents` exposed the configured Muse agent.
The model catalog reported `muse-spark-1.3-contributor`; coordinated preflight
and submission then admitted the AC06 service-test task
`b22767c4-48ac-4cc3-99e4-c91d5e5e51b2` against one allowed source file. The
worker changed only that file and returned a bounded diff artifact, but its
delivery was incomplete because the worker had not made a commit. It ran no
checks; the integrator separately applied and tested the change.

The first completed native turn did not include the required terminal
`PASSEUR_MESSAGE` v2 assignment-disposition envelope. Passeur surfaced the same
generic clarification (“Supply an explicit continuation instruction, or cancel
the task”) twice without exposing the parser's missing-marker/schema reason.
Providing the documented `PASSEUR_MESSAGE` marker and exact final JSON shape in
the clarification let the native task complete. The adapter should include the
disposition contract in the initial native instructions and return the concrete
bounded parse failure (missing marker, invalid JSON, or schema mismatch) in the
clarification. Repeating a generic continuation prompt made it unclear what
response would end the task.

The first `muse_result` request used `limit: 12000` and was rejected because the
runtime maximum is 8192 bytes; retrying at 8192 succeeded. Expose this maximum
in the tool schema/description so clients can page correctly without a failed
call.

These observations come from one coordinated Muse 1.3 implementation task.
The worker report is not code verification: it had no checks, the Passeur
delivery was uncommitted, and acceptance remains the integrator's responsibility.
