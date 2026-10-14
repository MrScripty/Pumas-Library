# Checkpoint findings

| Finding | Disposition / owner / trigger |
| --- | --- |
| Saved checkout predates merged v0.8 integration | Refreshed by new task branch; historical refs preserved |
| Earlier catalogue timing failure | Did not reproduce: complete core headless cohort passed, including 7 catalogue recovery tests. Main also includes the separate fork-inherited local-start lease fixture correction `dceaf642`. |
| Launcher qualification fixture failures | Repaired the 100 ms startup assumption and Linux zombie liveness oracle; all 49 launcher tests passed. Production process supervision unchanged. |
| RPC build prerequisite | First attempt stopped before tests because CMake was absent. Installed CMake 3.31.6 under task-local `/tmp/pumas-v080-cmake`; rerun pending. |
| Changelog/release notes lag current rc.1 manifests | In scope: current release notes and checkpoint QA |
| Installed production audio remains unavailable | Preserve the gate; audio owner must supply the complete admitted model/runtime/read/disposal qualification before advertising production |
| Inference-enabled headless artifacts and pretrained models lack final qualification | Parent/release owner selects accepted tuples and final bytes; controlled fixtures do not close these gates |
| gh API Forbidden; standards application routing unavailable | Use connected GitHub for review/PR when available; report exact remaining capability limits, never claim routing certification |
