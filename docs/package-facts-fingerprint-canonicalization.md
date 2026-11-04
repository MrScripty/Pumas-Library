# Canonical package metadata fingerprints

Package metadata includes a `HashMap` model card. Serializing that map directly
into a source fingerprint makes otherwise identical observations depend on map
insertion order and each process's random hash seed. A pinned import can therefore
produce facts and immediately reject those same facts as stale when issuing its
completion receipt.

The metadata hash input now uses the existing recursive canonical JSON SHA-256
projection. Object keys are sorted at every depth; array order, nulls, keys and
values are retained. Descriptor, dependency binding, selected file, size,
timestamp, package contract and GGUF inspector revision inputs are unchanged.

Previously stored raw-JSON fingerprints are deliberately incompatible. They
remain stale and must be re-observed by the normal authority-owning producer.
This change does not migrate, relabel or approve a cache row or completion
receipt. Pinned-output validation and acquisition custody are unchanged.

The fingerprint retains its existing file size/timestamp observation contract;
it is not a content hash and cannot detect same-size tampering that also restores
the timestamp. Managed payload SHA-256 verification remains a separate step.

## Tests

Run the normal deterministic regressions without ONNX binary downloads:

```sh
ORT_SKIP_DOWNLOAD=1 cargo test --locked --offline -p pumas-library \
  --no-default-features --features test-support fingerprint_tests
```

The opt-in full managed importer test accepts an already verified local copy of
the official Qwen Q4_K_M payload and its metadata snapshot. It checks the exact
491400032-byte payload SHA-256 and immutable revision before copying the payload
into a new fixture. It seeds pinned publisher metadata, replaces all source
endpoints with a rejecting loopback server, and asserts zero source requests.
Pumas itself verifies, parses, produces facts, validates the pinned receipt and
settles adoption. No runtime or inference is started.

```sh
PUMAS_VERIFIED_GGUF_FIXTURE=/path/to/copied/qwen2.5-0.5b-instruct-q4_k_m.gguf \
PUMAS_VERIFIED_METADATA_FIXTURE=/path/to/copied/metadata.json \
PUMAS_QUALIFICATION_OUTPUT=/path/to/new/qualification-directory \
ORT_SKIP_DOWNLOAD=1 cargo test --lib --locked --offline -p pumas-library \
  --no-default-features --features test-support \
  verified_qwen_full_managed_importer_qualification -- --ignored --nocapture
```

The output directory must not already exist. It is preserved, including failed
custody and `qualification.json`, for independent review. The supplied metadata
snapshot must use schema 2 and the same pinned revision. The test checks actual
GGUF v3 Qwen parser output, package-facts contract 3, model-ref contract 1, repeated
read-only currentness, and refusal of unsupported package contract/cache records.
This demonstrates the controlled importer path; it does not recover a historical
failed acquisition or establish runtime inference behavior.
