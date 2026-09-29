# Coding-Standards MCP usability feedback

This report records agent experience using the Coding-Standards MCP during Acquisition Q1 preparation. It is separate from acquisition acceptance: MCP routing and usability do not establish product-code compliance, and product tests do not establish that the MCP workflow was usable.

## Primary integrator

- **Useful calls:** `routing_facts` exposed the valid fact categories and values; an initial route with minimal facts made the router's information model clear. Compact routes with bounded content returned an explicit snapshot, selected standards, and unresolved-fact count. `read_many` let the integrator inspect the commit, planning, implementation, verification, architecture, contract, and documentation obligations. Rerouting after composition seams were clarified confirmed applicability for the actual Q1 boundary.
- **Confusing or redundant steps:** Tool discovery descriptions were verbose. The first broad route requested 32 policy contents and produced output large enough to truncate. A broad `read_many` similarly returned tens of thousands of tokens. A later route displayed policies already read, so applicability and policy reading were not easy to distinguish.
- **Missing context:** The MCP did not know the repository's current accepted plan status, retained-state population, public consumers, active recovery owner, or actual CI/source evidence. Those facts had to be gathered from the repository, GitHub, and the source tree. The route cannot certify that the source or tests satisfy the selected rules.
- **Smallest sufficient workflow:** Read routing facts once; route the concrete slice without policy contents; read Core plus only the selected standards and workflows relevant to its actual changed boundary in small pages; inspect source and run the required evidence; reroute only when ownership or scope changes; then use the final review operation against the implemented candidate.
- **Recommendations:** Keep route output compact by default and separate applicability reasons from policy bodies. Make content pages token-budgeted, expose previously read policy IDs, and put unresolved facts first. Clarify in the result that a complete route means applicability was resolved, not that implementation compliance was verified. A same-snapshot “changed design” review summary could call out new or removed obligations without replaying all policy text.

## Independent architecture reviewer

- **Useful calls:** `routing_facts`, a route including content, and the pinned snapshot made it possible to identify and read Architecture, Rust Async, Persistence, Contract Evolution, Core, and Planning. The final route selected 24 standards with zero unresolved categories; this was a complete applicability result, not a compliance certificate.
- **Confusing or redundant steps:** Initial tool discovery was verbose. The broad route returned roughly 68,000 tokens and truncated, repeated relationship rationales, and later policy reads repeated content already shown inline.
- **Missing context:** The MCP could not reveal the repository's persisted-state population, source/consumer ownership, accepted plan prerequisites, or shutdown composition. The reviewer had to inspect source and plan documents outside the MCP to assess those obligations.
- **Smallest sufficient workflow:** Route concrete facts once, omit inline policy content, read only the relevant policies in small pages, inspect source/evidence independently, and reroute when the design materially changes.
- **Recommendations:** Add an output-size budget, avoid repeating policy prose and relationship explanations, show which policies have already been read, and provide a same-facts design-change summary. Keep route completeness distinct from source/evidence compliance.

## Participation

The primary integrator and the independent architecture reviewer used the MCP and are represented above. The bounded source-inventory reviewer did not use it, so there is no MCP usability report from that agent.
