# S3-enabled release attribution

This inventory selects `pumas-rpc` with default features plus `s3`, across Linux
x86_64, macOS arm64 and Windows x86_64, using the current locked normal/build
dependency closure. It includes the production JavaScript closure and the same
separately provisioned runtime notice supersets as the default inventory. It is
not evidence that those runtimes are bundled, downloaded during builds or tested.

Reproduce with the locked Cargo cache and installed, lock-matching Node dependencies:

```sh
python3 scripts/release/generate-notices.py --features s3
node scripts/release/check-attribution.cjs --features s3
python3 scripts/release/check-dependency-features.py --s3
```

An S3-enabled distribution must include this profile's `THIRD-PARTY-NOTICES.txt`
and `inventory.json`. The default `0.7.0` inventory alone does not cover S3.
The checker rejects a profile mismatch or missing selected Rust notice. These
files do not certify a packaged distribution, provider, native runtime or license
interpretation. Earlier frozen qualification inventories retain their own scope.
