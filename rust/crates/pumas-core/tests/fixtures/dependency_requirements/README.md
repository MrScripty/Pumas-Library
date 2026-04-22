# Dependency Requirement Fixtures

## Purpose
These JSON fixtures describe dependency-profile scenarios used by model-library requirement tests.

## Producer Contract
Each fixture should encode one scenario: valid resolution, conflict, invalid shape, or unknown profile. File names should match the expected test case.

## Consumer Contract
Tests should parse these files through the same serde path used by production code so schema drift is visible.

## Non-Goals
None. Reason: this directory is entirely fixture data. Revisit trigger: add generated fixtures or binary data.
