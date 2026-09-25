# Target and Official Torch Wheel Research

Date checked: 2026-09-24.

## Pumas desktop targets

[`scripts/release/artifact-plan.json`](../../../../scripts/release/artifact-plan.json#L58)
is the current product target authority. It lists `x86_64-unknown-linux-gnu`,
`x86_64-pc-windows-msvc`, and `aarch64-apple-darwin`, with native Ubuntu,
Windows, and macOS release runners. The Electron packaging config also ships
Windows x64 and macOS arm64, not Windows ARM or Intel Mac.

## Official PyTorch examples

The [official CPU Torch index](https://download.pytorch.org/whl/cpu/torch/)
contains Torch 2.14.0 `+cpu` wheels tagged `win_amd64` and `win_arm64`, and
macOS arm64 wheels named with the plain release version and
`macosx_14_0_arm64`. CPython 3.10–3.13 artifacts are listed for the macOS
arm64 tag; the exact wheel tag sets the minimum supported macOS version for
that release. The Pumas target matrix supports only the macOS arm64 variant.

The [official CUDA 13.2 Torch index](https://download.pytorch.org/whl/cu132/torch/)
contains Torch 2.14.0 Windows x64 wheels for CPython 3.10–3.15, as well as Linux
wheels. This establishes package availability, not a Windows driver floor,
device execution, model compatibility, or image-generation support.

PyTorch's [MPS backend documentation](https://docs.pytorch.org/docs/stable/notes/mps.html)
describes Metal as a runtime backend. Therefore MPS is probed as a capability
of an installed native Torch runtime, not selected as an official wheel build
channel.

## Local support boundary inspected

- [`torch-server/resolve_runtime.py`](../../../../torch-server/resolve_runtime.py)
  runs wheel-tag inspection using the candidate interpreter but currently
  gates it to Linux x86_64 and assumes every build is represented by a
  `+<build>` distribution version.
- [`torch_alternatives.rs`](../../../../rust/crates/pumas-app-manager/src/version_manager/torch_alternatives.rs)
  is the host's current discovery owner but uses Linux PCI paths and
  Linux-only driver policy.
- [`torch_preview.rs`](../../../../rust/crates/pumas-app-manager/src/version_manager/torch_preview.rs),
  [`installer/torch.rs`](../../../../rust/crates/pumas-app-manager/src/version_manager/installer/torch.rs),
  and [`state.rs`](../../../../rust/crates/pumas-app-manager/src/version_manager/state.rs)
  contain Linux executable paths and/or gates.
- [`process_owner.rs`](../../../../rust/crates/pumas-core/src/runtime_profiles/process_owner.rs)
  rejects non-Linux owned binary-profile startup. Existing generic process
  helpers do not by themselves provide the Linux process group's retained
  generation evidence or a Windows Job Object lifetime.

These sources establish a clear code gap, not target acceptance. Required
Windows and macOS acceptance remains native and pending.
