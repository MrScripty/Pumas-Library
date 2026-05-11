# ONNX Runtime Embedding Serving

## Purpose

This directory contains the implementation plan and supporting analysis for
adding first-class ONNX Runtime embedding serving to Pumas Library.

The goal is to let users configure ONNX Runtime profiles, assign those profiles
to compatible local `.onnx` embedding models, serve the models through Pumas,
and expose them to Emily and other local applications through the existing
OpenAI-compatible Pumas `/v1` gateway.

## Contents

| File | Description |
| ---- | ----------- |
| [plan.md](plan.md) | Standards-compliant execution index with objective, scope, required inputs, risks, milestones, done criteria, and traceability links. |
| [inputs-and-standards.md](inputs-and-standards.md) | Standards reviewed, constraints, guardrails, gates, assumptions, dependencies, affected contracts, persisted artifacts, and lifecycle ownership notes. |
| [impact-review.md](impact-review.md) | Codebase blast-radius review, anti-patterns, simplification opportunities, and performance/maintainability implications. |
| [provider-model-and-contracts.md](provider-model-and-contracts.md) | Cleaner provider model design and contract ownership matrix. |
| [risks.md](risks.md) | Full risk table and mitigations. |
| [milestones.md](milestones.md) | Detailed milestone tasks, verification checks, and status fields. |
| [execution-and-coordination.md](execution-and-coordination.md) | Execution notes, commit cadence, optional parallel worker plan, re-plan triggers, recommendations, and completion-summary template. |

## Decision

Execute through [plan.md](plan.md). Supporting files are part of the plan and
must be kept in sync when milestones, contracts, risks, or verification
requirements change.

## Invariants

- Pumas `/v1` remains the supported external facade for Emily and other local
  clients.
- Backend-owned runtime profile and served-model state remain authoritative.
- ONNX is implemented through the cleaned-up provider model, not as a third
  hard-coded provider branch.
- Provider-scoped route identity replaces one-route-per-model semantics.
- Generic ONNX embedding compatibility remains separate from custom ONNX app
  metadata such as KittentTS.
- ONNX dependencies remain owned by the Rust module/crate that executes ONNX
  Runtime.

## Usage

Start with [plan.md](plan.md), then use the supporting files for detailed
implementation constraints:

- Use [impact-review.md](impact-review.md) before editing touched code areas.
- Use [provider-model-and-contracts.md](provider-model-and-contracts.md) before
  changing provider, route, gateway, or frontend contract shapes.
- Use [milestones.md](milestones.md) to execute and update implementation
  status slice by slice.
- Use [execution-and-coordination.md](execution-and-coordination.md) for
  re-plan triggers, worker coordination, and final completion reporting.

## Structured Producer Contract

These files are human-maintained Markdown planning artifacts. Stable producer
fields are headings, milestone checklists, verification lists, risk tables,
contract ownership tables, execution notes, and traceability links.

Implementation updates must preserve traceability from objective to affected
contract, milestone task, verification evidence, execution note, and completion
summary.
