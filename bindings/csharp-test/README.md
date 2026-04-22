# C# Binding Test Artifact

## Purpose
This directory contains generated C# binding output used for local smoke checks.

## Producer Contract
Generated files must be reproducible from the UniFFI binding generation workflow and should not be manually edited as source of truth.

## Consumer Contract
Smoke tests may compile or inspect this output to verify generated API shape. Product code should consume a packaged binding artifact instead of this scratch output.

## Non-Goals
None. Reason: this directory is generated binding test data. Revisit trigger: replace it with a checked-in package fixture or remove it from source control.
