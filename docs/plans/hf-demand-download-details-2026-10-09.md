# Demand-driven Hugging Face download details

## Acceptance goal and scope

The [HF cache brief](../breif/hf-cache.md) separates lightweight discovery from expensive repository hydration. Ordinary interactive model search now requests `hydrate_limit = 0` through the existing search contract. Search can reuse existing local download details, and a discovery cache miss can still make its ordinary upstream search request. Opening one repository's download menu requests that repository's missing details; simply rendering a search result does not request its file tree or config.

This follows the [explicit cached-search presentation](hf-cached-search-ui-2026-10-08.md). Cached mode remains offline and offers no new download choices. There is no new search method, transport, importer, cache publisher or download owner. Existing native/RPC contracts and all Rust sources are unchanged. S3 remains a byte transport and retains the shared format-independent importer.

## Selected detail behavior

Discovery size totals and quant tags alone do not prove downloadable file choices. The hook and menu share one predicate for usable existing options. A successful selected response records completion separately from sizes, so unknown-size options are usable and an empty response displays “No downloadable files were found” without fabricating quant or all-files choices.

Loading suppresses download choices. A failed or repository-mismatched response preserves the discovery row and exposes an error plus “Retry download details”. Closing and reopening the failed menu preserves that error; only an explicit retry initiates another request. A successful response replaces its selected row's options and total, preserving file-group identities and other repositories. Existing acquisition still performs its own live selection, integrity and package checks.

Per-repository in-flight requests coalesce within a search generation. Registration precedes transport invocation, including a synchronous transport failure, so the failed request cannot strand a settled promise and disable retry. Successful completion is retained synchronously as well as in UI state. Retained callbacks therefore reuse completed empty or unknown-size results. Admission and response application check the current query/source context and generation; old responses, callbacks after a source switch, and delivery after unmount cannot hydrate the new view. Visible-result references are installed in the layout phase before interaction.

These are renderer request and response-delivery rules. Ignoring an old reply does not cancel already admitted server or filesystem work. The existing backend remains responsible for effect ownership and cancellation.

## Local qualification

The following final-source checks passed:

- Full frontend suite: **912 passed, one skipped, 136 files**. The skipped case requires a launched RPC process and was executed separately below.
- Focused hook and mounted-workflow regressions cover zero eager hydration, selected-repository identity, coalescing, failed reopen, explicit retry, wrong-repository refusal, unknown sizes, empty options, synchronous failure followed by retry, retained callbacks, query/source changes and unmount.
- TypeScript `--noEmit`, scoped ESLint with zero warnings, Vite library-only build, Python byte compilation and whitespace checks.
- `qualify-hf-cached-search-ui.py --demand-details`: **four demand workflows** (three controlled adapter-response cases and one mounted actual-RPC failure/retry case), followed by **three existing mounted actual-RPC cached-search workflows**. The runner also checks actual RPC envelopes, cached provenance/visibility, ordinary failures and absence of published models or download jobs.

The launched process used the previously qualified RPC binary with SHA-256 `de49db710e5fd57dbcb9245023fb44577111c73656f1fdd87feb56a481babb12`. All tracked Rust source and dependency blobs are unchanged from its qualified source. No new Rust build or core-suite run is claimed. No build-time ONNX download occurred.

The owned exact-search fixture contains two rows without download options. Discovery causes **zero** upstream detail attempts. Selecting the second row and explicitly retrying its failure causes **two** refused upstream attempts, both initiated through that row's actual detail RPC. No model bytes are downloaded. Positive detail/menu cases use controlled adapter responses; the rejecting loopback proxy is not a Hugging Face or S3 provider.

The initial new mounted regressions failed against the previous behavior. During implementation, the broader suite exposed a selected-click race before a passive latest-results update; the layout-phase update repairs it. Initial fixture mock/typing issues and lint failures were corrected before the final runs. No unresolved failure remains in the checks executed for this slice. Existing broader core registry baseline failures are documented in the [local-discovery report](hf-local-detail-discovery-2026-10-08.md) and were not rerun here.

## Reproduction and limits

Run the existing frontend test, type, lint and library-build commands. Execute the launched-process case explicitly:

```sh
python3 scripts/tests/qualify-hf-cached-search-ui.py \
  --rpc "$PUMAS_RPC" --demand-details --output "$QUALIFICATION_OUTPUT"
```

Use a fresh owned output directory, anonymous configuration and the supported local RPC build. The driver refuses to read or alter an ambient CLI token. Its proxy rejects upstream access; fixture records are inserted only into the owned test database.

This qualifies the described mounted React/jsdom and loopback RPC paths, not native Electron, privileged IPC, visual acceptance, real online-provider success, import of a new model or inference. Successful real-provider detail hydration may paginate or fetch selected config metadata; this slice does not impose a one-request budget on that operation. It does not change default hydration behavior for other native/RPC callers, HF access policies, rate-limit policy or the persistent cache schema. Shared/bulk catalogs, broader request admission and real-provider/native acceptance remain separate roadmap work.
