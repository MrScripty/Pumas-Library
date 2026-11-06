# @pumas/desktop-contract

Generated wire types and decoders for the desktop contract: the payloads that
cross between the Rust backend, Electron main and preload, and the frontend.
The Electron shell and the frontend both import this one copy.

Everything in `generated/` is produced by
`pnpm --dir electron run generate:desktop-contract` from the `pumas-rpc`
contract export and `electron/contracts/`. Do not edit it by hand. CI runs
`pnpm --dir electron check:desktop-contract`, which fails when these files
differ from a fresh export.

The files are checked in so UI builds do not need a Rust toolchain. See
[Electron](../../electron/README.md#generated-catalog-and-download-contract)
for the contract's ownership and validation rules.
