# Pass 02 - Frontend Function Length Inventory

## Context
The earlier trial for lowering `max-lines-per-function` to 80 effective lines
is superseded. The standards plan now uses a 500-line function ceiling as the
explicit decomposition threshold, matching the project-wide 500-line review
target recorded in Pass 01.

The frontend currently enforces `max-lines-per-function` at 500 effective
lines, with blank lines and comments skipped. No frontend function currently
exceeds that threshold. Further decomposition work should be driven by state
ownership, complexity, coupling, testability, or user-facing behavior risk
rather than pursuing smaller line-count-only ratchets.

## Retired Trial Rule
`max-lines-per-function: ["error", { "max": 80, "skipBlankLines": true, "skipComments": true }]`

## Remaining Findings
None for the adopted 500-line function threshold.
