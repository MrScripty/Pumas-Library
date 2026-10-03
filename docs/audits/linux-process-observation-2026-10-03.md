# Bounded retry of incomplete Linux process observations

A hosted router-lifecycle test failed during owned shutdown when `/proc` task
entries disappeared after enumeration while the unreaped zombie leader was
still present. The existing scanner correctly refused to call this an empty
process group, but the managed child propagated the first incomplete observation
immediately rather than using the caller's remaining drain budget.

Only the two structured missing-task visibility cases now return `WouldBlock`:
a missing task directory with a still-present leader, or an incomplete/empty task
observation while that leader remains. Malformed data, permission errors, other
I/O errors, and group-signal errors remain explicit failures. No diagnostic-string
matching or false-empty fallback is used.

The existing drain loop may retry `WouldBlock` within its original deadline and
poll interval. It retains the unreaped leader/PGID pin and custody throughout.
Exhaustion reports `TimedOut` with the last incomplete observation and leaves the
owner available for retry; only a complete no-live-members observation permits
reaping and custody release. This does not expand the cooperating-process-group
contract to escaped processes, hostile namespaces or external reapers.

Evidence includes a bounded standalone copy of the scanner (only libc's Linux
ESRCH constant shimmed) failing before the classification repair and passing after.
Repository fixtures exercise transient-then-complete drain, zero-budget persistent
uncertainty with retained identity/custody, and malformed-stat refusal. Existing
real process-group tests and the affected router-lifecycle test remain native
Linux CI gates. Full local Rust builds were not attempted; exact-head hosted
qualification and independent review are still required.
