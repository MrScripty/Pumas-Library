# Intent IPC fixture readiness publication

The B1 current-main qualification at `8b260b230917057f8ca189e97b8065241168f5a5`
failed Rust quality in [run 37116185871](https://github.com/MrScripty/Pumas-Library/actions/runs/37116185871),
job 111183460501. `intent_ipc_tests.rs:192` parsed an empty readiness file and
reported `EOF while parsing a value`. This was the parent fixture's JSON file,
not an IPC response. The test source was unchanged from qualified main.

The child created the final readiness filename before writing its bytes, while
the parent used file existence as its signal. The repair writes and syncs a
private same-directory temporary, then publishes it with no-clobber semantics.
A deterministic checkpoint asserts that the final path remains absent before
publication; an existing target must retain its original bytes.

A startup guard owns the exact child immediately after spawn, before readiness
read/parse can fail. Ownership transfers to the existing running-fixture owner
only after parsing succeeds. The failure regression launches a test-only
parked child with no Pumas instance or network activity, then proves failed
readiness parsing still observes that child's exit through its retained handle.
The receipt is never inferred from a PID lookup.

This changes fixture code only. No production IPC, registry, model, or process
ownership behavior changes. Formatting and whitespace checks passed locally;
Rust compilation and native fixture execution remain hosted qualification gates.
