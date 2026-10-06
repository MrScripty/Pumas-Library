"""Generate an isolated external crate; never alter production Cargo files."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
source = Path(__file__).resolve().parent
root = source.parents[4]
crate = args.output.resolve()
(crate / "src").mkdir(parents=True, exist_ok=True)
shutil.copyfile(source / "acquire.rs", crate / "src/main.rs")
(crate / "Cargo.toml").write_text('''[package]
name = "pumas-offline-catalog-experiment"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
pumas-library = { path = CORE_PATH, default-features = false }
async-trait = "0.1"
reqwest = "0.12.28"
serde_json = "1"
tokio = { version = "1", features = ["rt", "macros"] }
[profile.dev]
debug = 0
incremental = false
'''.replace("CORE_PATH", json.dumps(str(root / "rust/crates/pumas-core"))))
# Preserve reviewed resolved versions while admitting this external root offline.
shutil.copyfile(root / "rust/Cargo.lock", crate / "Cargo.lock")
subprocess.run(["cargo", "build", "--offline", "--manifest-path", str(crate / "Cargo.toml")], check=True)
subprocess.run(["cargo", "build", "--offline", "--locked", "--manifest-path", str(crate / "Cargo.toml")], check=True)
