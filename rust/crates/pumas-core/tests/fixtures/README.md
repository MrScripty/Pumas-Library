# pumas-library Test Fixtures

## Purpose
This directory stores static fixtures consumed by `pumas-library` integration tests.

## Producer Contract
Fixtures must be deterministic, minimal, and named for the behavior they cover. Any generated fixture must document its generator or source format before being committed.

## Consumer Contract
Tests should load fixtures read-only and copy them into temporary directories before mutation.

## Fixture Families

- `dependency_requirements/`: versioned dependency-resolution contract fixtures.
- `package_facts/`: versioned model package-facts contract fixtures.

## Non-Goals
Large runtime model artifacts are out of scope. Reason: fixtures must remain lightweight for source control and CI. Revisit trigger: add artifact download fixtures behind an explicit integration-test gate.
