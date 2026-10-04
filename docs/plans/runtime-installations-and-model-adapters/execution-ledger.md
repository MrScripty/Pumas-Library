# Runtime execution ledger

## 2026-09-29 — revision 4, separate acquisition prerequisite

The user requested a dedicated acquisition implementation plan alongside this runtime plan. Revision 3 is the input. Its source intent and runtime/adapter decisions are preserved, but generic acquisition S1a is superseded by the separate acquisition Q1. S1b becomes R1, gated by AQ-HTTP. Exact package-file acquisition is acquisition Q2; registered-adapter R2 additionally requires AQ-PACKAGES. R3–R5 retain the existing admission, independent Z-Image extension and qualification objectives.

Created one canonical proposed acquisition contract and linked both plans to it. Acquisition's existing HF/native/package consumers prove prerequisites without waiting for new installation identity or adapter registration. Shared source and generated files have one integrator, and acquisition alone owns the AQ gate status.

Pumas branch baseline is unchanged at `04e7f156`; current standards ref is `39d55dc`. Production acceptance remains pending. This delivery changes only planning artifacts in the working container. No repository write, implementation test, actual runtime/model operation, live migration, public release or independent external review occurred.

The earlier entry-point mechanism probe is historical limited evidence in the previous delivery; it is not re-run, re-packaged as new evidence or used to mark runtime claims satisfied. Earlier delivered packages preserve revision history; this package avoids copies of obsolete active plans.

**Exactly one next runtime slice:** R1 after AQ-HTTP is ready for its actual target. The coordinated program starts with acquisition Q1. Plan-only preparation may continue without opening the runtime source gate.

## 2026-09-29 — Q1 implementation started on current accepted main

The original companion plan commit `a8359512a580aa25fb2f9c9e4cd7e0dd64fd970d` is preserved on the acquisition Q1 branch based on current accepted `main` `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`. The acquisition plan is active; AQ-HTTP remains not ready. R1 has not started because its prerequisite has not been accepted or merged. Current Build and Scorecard success on the base commit do not qualify a runtime or acquisition candidate. No runtime source, profile, process-binding, adapter, dependency or model-admission behavior changed in this slice.
