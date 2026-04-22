# Torch Server Tests

## Purpose
This directory contains focused Python tests for the Torch sidecar API boundary, app factory, and model-manager state behavior.

## Producer Contract
Tests should prefer lightweight fakes for heavyweight Torch/FastAPI dependencies unless they intentionally verify integration with the real runtime stack.

## Consumer Contract
Developers and CI may run these tests from the repository root or `torch-server/` after installing Python test dependencies.

## Non-Goals
Full model inference validation is out of scope. Reason: model inference requires large external artifacts and belongs behind an explicit integration-test gate. Revisit trigger: add small deterministic model fixtures.
