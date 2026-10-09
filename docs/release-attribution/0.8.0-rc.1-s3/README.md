# 0.8.0-rc.1 S3 candidate attribution

This profile selects `pumas-rpc` with default inference features plus `s3`, for
Linux x86_64, macOS arm64 and Windows x86_64. It includes the locked Rust
normal/build closure, production JavaScript closure and checked native-runtime
notice supersets. The default inventory alone does not cover S3 dependencies.

Generate with `python3 scripts/release/generate-notices.py --features s3` and
validate with `node scripts/release/check-attribution.cjs --features s3`.
Inference headless candidates bind this profile's inventory and notice bytes.
The inventory does not certify runtime bundling, native execution, model quality,
signing or release acceptance. Candidate version preparation creates no release.
