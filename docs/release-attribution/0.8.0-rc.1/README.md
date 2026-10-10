# 0.8.0-rc.1 candidate attribution

Generate this default-profile inventory from the combined candidate source with
`python3 scripts/release/generate-notices.py`. The generator reads the locked
normal/build Rust dependency closure for all three desktop targets and the
installed production JavaScript closure, preserving exact authoritative legal
text. Validate with `node scripts/release/check-attribution.cjs`.

The bundled JPEG codec retains both its license and incorporated IJG terms; the
wrapper MIT text is pinned to the published crate's upstream VCS revision.
Managed CPython archive notices remain a conservative superset, bound to the
checked provider/catalog evidence. These records establish attribution inputs,
not native execution, signing, model quality or permission to publish a release.

An S3-enabled build must use the [S3 profile](../0.8.0-rc.1-s3/README.md).
Regenerate both profiles after changes to dependency or version manifests.
