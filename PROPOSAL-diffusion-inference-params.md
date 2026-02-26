# Proposal: Additional Diffusion Inference Parameters

## Summary

Extend the `"diffusion"` arm of `default_inference_settings()` with six new parameters
covering image dimensions, negative prompting, scheduler selection, CLIP skip, and batch
size. This is a **non-breaking, additive** schema extension — existing diffusion models
continue to work unchanged; new fields receive sensible defaults.

## Current State

The `"diffusion"` arm (`inference_defaults.rs:97-136`) already exposes:

| Key        | Type    | Default | Range    |
| ---------- | ------- | ------- | -------- |
| steps      | Integer | 20      | 1-150    |
| cfg_scale  | Number  | 7.0     | 1.0-30.0 |
| seed       | Integer | -1      | -1-inf   |

## Proposed Additions

| Key             | Type        | Default  | Range / Allowed Values                      | Description                |
| --------------- | ----------- | -------- | ------------------------------------------- | -------------------------- |
| width           | Integer     | 512      | 64-2048, step 8                             | Output image width         |
| height          | Integer     | 512      | 64-2048, step 8                             | Output image height        |
| negative_prompt | String      | ""       | (free text)                                 | Negative prompt for CFG    |
| scheduler       | String enum | "euler"  | euler, euler_a, ddim, pndm, dpm_solver      | Diffusion scheduler        |
| clip_skip       | Integer     | 1        | 1-12                                        | CLIP layers to skip        |
| batch_size      | Integer     | 1        | 1-8                                         | Images per generation      |

After this change the diffusion arm will expose nine parameters total.

## Schema Impact: `step` Field on `ParamConstraints`

The `width` and `height` parameters require a **step** constraint (multiples of 8).
The current `ParamConstraints` struct (`model.rs:169-178`) only has `min`, `max`, and
`allowed_values` — it has no way to express a step interval.

**Recommended change:** add an optional `step` field to `ParamConstraints`:

```rust
// model.rs — ParamConstraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamConstraints {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// If set, value must be one of these (enum-style dropdown).
    #[serde(default)]
    pub allowed_values: Option<Vec<serde_json::Value>>,
    /// If set, value must be a multiple of this step (e.g. 8 for image dims).
    #[serde(default)]
    pub step: Option<f64>,
}
```

Because the field is `Option` with `#[serde(default)]`, existing serialized schemas
deserialize without error — this is fully backward-compatible.

## Implementation Sketch

New entries appended to the `"diffusion"` vec in `inference_defaults.rs`:

```rust
// --- width ---
InferenceParamSchema {
    key: "width".into(),
    label: "Width".into(),
    param_type: ParamType::Integer,
    default: json!(512),
    description: Some("Output image width".into()),
    constraints: Some(ParamConstraints {
        min: Some(64.0),
        max: Some(2048.0),
        step: Some(8.0),
        allowed_values: None,
    }),
},
// --- height ---
InferenceParamSchema {
    key: "height".into(),
    label: "Height".into(),
    param_type: ParamType::Integer,
    default: json!(512),
    description: Some("Output image height".into()),
    constraints: Some(ParamConstraints {
        min: Some(64.0),
        max: Some(2048.0),
        step: Some(8.0),
        allowed_values: None,
    }),
},
// --- negative_prompt ---
InferenceParamSchema {
    key: "negative_prompt".into(),
    label: "Negative Prompt".into(),
    param_type: ParamType::String,
    default: json!(""),
    description: Some("Negative prompt for CFG".into()),
    constraints: None,
},
// --- scheduler ---
InferenceParamSchema {
    key: "scheduler".into(),
    label: "Scheduler".into(),
    param_type: ParamType::String,
    default: json!("euler"),
    description: Some("Diffusion scheduler".into()),
    constraints: Some(ParamConstraints {
        min: None,
        max: None,
        step: None,
        allowed_values: Some(vec![
            json!("euler"),
            json!("euler_a"),
            json!("ddim"),
            json!("pndm"),
            json!("dpm_solver"),
        ]),
    }),
},
// --- clip_skip ---
InferenceParamSchema {
    key: "clip_skip".into(),
    label: "CLIP Skip".into(),
    param_type: ParamType::Integer,
    default: json!(1),
    description: Some("CLIP layers to skip".into()),
    constraints: Some(ParamConstraints {
        min: Some(1.0),
        max: Some(12.0),
        step: None,
        allowed_values: None,
    }),
},
// --- batch_size ---
InferenceParamSchema {
    key: "batch_size".into(),
    label: "Batch Size".into(),
    param_type: ParamType::Integer,
    default: json!(1),
    description: Some("Images per generation".into()),
    constraints: Some(ParamConstraints {
        min: Some(1.0),
        max: Some(8.0),
        step: None,
        allowed_values: None,
    }),
},
```

## Files Modified

| File | Change |
| ---- | ------ |
| `rust/crates/pumas-core/src/models/model.rs` | Add `step: Option<f64>` to `ParamConstraints` |
| `rust/crates/pumas-core/src/models/inference_defaults.rs` | Append six parameters to `"diffusion"` arm |
| `rust/crates/pumas-core/src/models/inference_defaults.rs` (tests) | Update `test_diffusion_defaults` to assert new keys |

Existing `ParamConstraints` initializers elsewhere gain `step: None` (or rely on
`Default` / struct update syntax).

## Compatibility

- **Serialization**: `#[serde(default)]` on `step` ensures old JSON round-trips cleanly.
- **API surface**: additive only — no existing fields removed or renamed.
- **UI consumers**: should render `step` as a spinner/slider increment when present.
