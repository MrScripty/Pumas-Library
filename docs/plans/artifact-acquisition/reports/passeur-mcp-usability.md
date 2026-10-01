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
