# pumas-rpc Tests

## Purpose
This directory contains integration tests for the JSON-RPC server boundary.

## Producer Contract
Tests should exercise externally visible server behavior: request parsing, method dispatch, response shape, startup state, and shutdown/lifecycle behavior when exposed.

## Consumer Contract
These tests may use temporary directories, loopback ports, and public server helpers. They should not require user machine state or existing launcher data.

## Isolation Requirements
Tests that bind ports or mutate process-global state must allocate unique resources per test and document any serialization requirement near the test.

## Non-Goals
Frontend IPC validation is out of scope. Reason: Electron validates renderer payloads before forwarding to this server. Revisit trigger: add a cross-process contract test harness.
