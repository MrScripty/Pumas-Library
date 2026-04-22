# Torch Model Loaders

## Purpose
This directory contains model-format-specific loader implementations for the Torch sidecar.

## Producer Contract
Loaders must expose a narrow function or class surface that accepts validated model paths and returns objects that `model_manager.py` can register in a slot.

## Consumer Contract
`model_manager.py` is the primary consumer. API route modules should not call loaders directly because slot state and concurrency ownership belong to the manager.

## Non-Goals
HTTP request validation is out of scope. Reason: route modules and Pydantic models own network ingress validation. Revisit trigger: add loader-specific request models.
