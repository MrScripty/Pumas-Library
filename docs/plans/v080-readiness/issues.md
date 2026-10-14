# Checkpoint findings

| Finding | Disposition / owner / trigger |
| --- | --- |
| Saved checkout predates merged v0.8 integration | Refreshed by new task branch; historical refs preserved |
| Earlier catalogue timing failure | Did not reproduce: complete core headless cohort passed, including 7 catalogue recovery tests. Main also includes the separate fork-inherited local-start lease fixture correction `dceaf642`. |
| Launcher qualification fixture failures | Repaired the 100 ms startup assumption and Linux zombie liveness oracle; all 49 launcher tests passed. Production process supervision unchanged. |
| RPC build prerequisite/resource failures | Missing CMake and later disk exhaustion prevented two attempts before tests. Installed task-local CMake 3.31.6 and removed completed task-built test executables; final RPC run passed 257 tests, with 13 ignored. |
| Changelog/release notes lag current rc.1 manifests | In scope: current release notes and checkpoint QA |
| Installed production audio remains unavailable | Preserve the gate; audio owner must supply the complete admitted model/runtime/read/disposal qualification before advertising production |
| Inference-enabled headless artifacts and pretrained models lack final qualification | Parent/release owner selects accepted tuples and final bytes; controlled fixtures do not close these gates |
| gh API Forbidden; standards application routing unavailable | Use connected GitHub for review/PR when available; report exact remaining capability limits, never claim routing certification |
| GitHub default-branch dependency alert notice | Push reported 52 alerts (27 high, 19 moderate, 6 low). Not independently triaged as shipped exposure; release owner retains the current security-review gate. No unrelated dependency expansion in this slice. |
