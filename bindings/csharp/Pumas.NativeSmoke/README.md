# Pumas.NativeSmoke

## Purpose
This C# project is a smoke harness for validating that generated C# bindings can load and call the native Pumas library.

## Producer Contract
The project should exercise a minimal supported call path and document which native library artifact it expects beside or ahead of the managed executable.

## Consumer Contract
Release verification may run this harness after generating bindings and copying native libraries into the expected location.

## Non-Goals
Comprehensive domain testing is out of scope. Reason: the smoke harness verifies packaging and host loading. Revisit trigger: add a host-language conformance suite.
