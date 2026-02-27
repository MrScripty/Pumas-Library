# Anti-Pattern Remediation Tracker

## Scope
This tracker covers anti-patterns identified in the 2026-02-27 code audit.

## Task List
| ID | Task | Status | Commit |
|---|---|---|---|
| T1 | Remove blocking sync launch/stop work from async API call paths and avoid holding async locks during blocking operations. | Completed | `fix(process): move blocking launch/stop work off async runtime` |
| T2 | Fix stale-closure timer management in `useInstallationProgress` so polling reliably stops on completion/unmount. | Completed | `fix(frontend): make installation progress timers ref-driven` |
| T3 | Enforce URL boundary validation in Electron main-process `shell:openExternal` IPC handler. | Pending | Pending |
| T4 | Remove panic-prone `unwrap()` calls in process launcher log wiring and return typed errors instead. | Pending | Pending |

## Standards Improvement Suggestions
| Related Task | Existing Standard | Suggestion |
|---|---|---|
| T1 | `CONCURRENCY-STANDARDS.md` | Add an explicit rule: do not run blocking calls (`std::thread::sleep`, blocking socket/file/process operations) directly in async request paths; require async equivalents or `spawn_blocking`. |
| T2 | `CODING-STANDARDS.md` | Add a React hooks polling guideline: store timer handles in refs, avoid state-backed interval IDs, and require deterministic cleanup tests for polling hooks. |
| T3 | `SECURITY-STANDARDS.md` | Add an Electron IPC boundary section: validate URL/path payloads in `ipcMain.handle` even when preload/renderer already type-checks. |
| T4 | `CODING-STANDARDS.md` | Add a reliability rule: no `unwrap`/`expect` in production paths unless invariant is explicitly documented and guarded. |

## Notes
- Update this file as each task is completed.
- Each task should be committed as a focused atomic change following Conventional Commits.
