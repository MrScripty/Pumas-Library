# Runtime issues and dispositions — revision 4

All code fixes remain planned. [Source audit](reports/codebase-audit.md) and the earlier delivered r3 package provide the baseline evidence; [acceptance](reports/acceptance-matrix.md) owns the current runtime claims.

| Family | Severity / issue | Owner and disposition | Verification / revisit |
| --- | --- | --- | --- |
| E01/E02/E11 | High: concrete installation versus tag/global process ambiguity | R1 runtime/profile owner | A01/A02/A09/A20/A27 |
| E03/E06/E07 | High: missing adapter-specific admission and multiple live consumer paths | R3 managed execution/host/desktop | A03/A07/A08/A12/A18 |
| E04/E05 | High: requirement identity and additive semantics versus complete-environment policy | R3 core dependency owner | A04/A05/A19/A22; preserve authored old meaning until versioned |
| E08 | High: old-writer profile loss during new schema migration | R1/R5 core/integrator | A10/A19; actual retirement/isolation before retained-state mutation |
| E09/E10 | Medium: fixed bundle inventory and generated/handwritten UI semantics | R2 and each affected producer/consumer owner | A06/A15/A16/A21 |
| E13–E16 | High: native/Python common management and open revision-safe registration | R1/R2 app-manager/host | A20/A21/A23/A24/A27 |
| E17–E19 | High: task assumptions, control responsiveness and executable code authorization | R2/R3 host/security | A22/A25/A26/A28 |
| E23–E28 | High: acquisition boundary and duplicate/layer authority | Acquisition plan owns shared repair; runtime consumes gates | AC claims plus runtime A08/A09/A17/A25/A28/A30 |
| RT-P01 | Blocking dependency: AQ-HTTP not ready | Acquisition Q1; runtime R1 source integration waits | Target-scoped gate record, not an assumed helper API |
| RT-P02 | Blocking dependency for registration install: AQ-PACKAGES not ready | Acquisition Q2; runtime R2 waits after R1 | Existing and then registered-package local-install evidence |
| RT-D01 | Deferred: marketplace, arbitrary task host, native dylib ABI, orchestration | Respective runtime/host owner | Revisit only for explicit product/trust/protocol requirement, not an ordinary adapter addition |
| RT-E01 | Pending required-real image/native/package/UI/old-deployment evidence | Assigned runtime/host/distribution owners | Close only with named A claims and actual environment |

Existing Nunchaku and FLUX.2 qualification remains exact-tuple historical evidence. Native management, a standard loader and standalone Z-Image need new evidence through the new contract. Temporary facades have a real supported-consumer owner and cutover trigger; preserve no alias merely because it existed.
