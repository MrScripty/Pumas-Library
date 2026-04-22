# Script Templates

## Purpose
This directory stores shell templates rendered or copied by launcher and development tooling.

## Producer Contract
Templates must be executable only after their variables have been resolved by an owning script. New placeholders should be documented in the consuming script or adjacent comments.

## Consumer Contract
Consumers must resolve paths safely, quote shell values, and avoid assuming a template is valid for every platform.

## Non-Goals
None. Reason: this directory is entirely structured producer input. Revisit trigger: add a non-shell template type.
