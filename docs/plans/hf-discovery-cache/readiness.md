# Deferred Hugging Face discovery-cache service readiness

Status: planning only, 2026-10-03. Revisit after the current baseline repairs,
S3 acquisition and restricted networked-node milestone. This checklist does not
start search implementation or authorize external resources.

## Source and dependency decisions

The [Hugging Face discovery cache brief](../../breif/hf-cache.md) specifies local
SQLite discovery first, then an optional shared/bulk online cache, with Hugging
Face authoritative for current details. It does **not** select a hosting vendor
or require every client to use an online service. Its concrete dependencies are:

| Capability | Dependency if selected | Evidence in the brief |
| --- | --- | --- |
| Local accumulated catalog and offline search | Existing local SQLite plus FTS; no hosted service | Local Discovery Catalog |
| Authoritative discovery misses and detail hydration | Hugging Face Hub API, with shared request admission, freshness and rate-limit handling | Search and Hydration Separation; Freshness Model |
| Known artifact bytes | Hugging Face resolver/download infrastructure; keep identity verification at acquisition | Hugging Face Resolver Usage |
| Shared cache | A narrow hosted catalog API plus SQL/key-value storage behind it | Shared Online Cache |
| Candidate shared-cache providers | Cloudflare D1, Vercel, Turso/libSQL, another inexpensive database, or a custom Pumas catalog API; alternatives to evaluate, not a purchase list | Shared Online Cache |
| Bulk catalog | A verified/licensed source, catalog-builder job and versioned compressed snapshot distribution | Bulk Hugging Face Index Sources |
| Candidate bulk source | Hugging Face `hub-stats`; suitability, maintenance and licensing still need verification | Bulk Hugging Face Index Sources |
| Client observations | Optional write API, validation, abuse controls and a privacy policy | Shared Online Cache; Development-Plan Questions |
| Supplementary catalog | Only an officially supported machine-readable interface, if one exists | Other Catalog Sources |

The brief mentions LM Studio only as an investigation candidate. An undocumented
endpoint is not an approved dependency. Peer-to-peer observation sharing is
optional and lower priority; the upcoming restricted node transport does not
implicitly authorize search-query sharing or make a P2P catalog necessary.

## Readiness checklist before provisioning

- [ ] Name the service owner, allowed audience, operating region, budget ceiling,
  expected request/storage/egress volume and retention policy. Compare current
  providers and terms when this phase starts; no prices or limits are assumed here.
- [ ] Choose the smallest deployment: shared read API and backing store only if
  local catalog plus downloadable snapshots is insufficient. Identify who owns
  domain/TLS, deployment, backups, recovery, monitoring and recurring updates.
- [ ] Define catalog schema/versioning, record size bounds, provenance and observed
  timestamps. Keep discovered facts separate from authority to download, mutate
  state or execute code. Remote cache records cannot grant those permissions.
- [ ] Verify bulk-source identity, license/redistribution rights, field coverage,
  size, update cadence and deletion/correction behavior. Pin input versions and
  make the builder reproducible before scheduling it.
- [ ] Define immutable snapshot identity, integrity/authenticity verification,
  publication/rollback, client schema compatibility, incremental updates and
  stale/offline behavior. A generic S3 capability is not itself a selected catalog
  host or an approved snapshot bucket.
- [ ] Decide whether clients may contribute observations at all. Default to no
  search-query or private/gated-repository uploads until purpose, user controls,
  minimization and retention are reviewed. Treat contributions as untrusted.
- [ ] Specify read/write authentication and least privilege. Clients must never
  receive privileged database credentials. Keep upstream user tokens local;
  public-catalog collection does not imply permission to reuse private access.
- [ ] Design quotas, request coalescing/debounce, rate-limit backoff and poisoning
  defenses. Test stale-but-usable results during upstream/shared outages and
  hydrate details only when needed.
- [ ] Obtain the explicit account/resource, cost, credential and data-sharing
  approvals required by the selected setup. Record actual endpoint and secret
  ownership through the approved secure setup flow, not repository files.

## Implementation re-entry gate

Before returning to search development, reconcile this checklist with the then
current brief, code and provider documentation. Produce a focused implementation
plan with an owner and acceptance tests for local search, lazy hydration, offline
behavior, reduced upstream request counts, provenance/freshness, migrations and
service trust boundaries. Real hosted-service qualification requires authorized
resources and remains distinct from local fixtures.

No service accounts, credentials, databases, domains, schedulers or buckets have
been provisioned by this readiness document.
