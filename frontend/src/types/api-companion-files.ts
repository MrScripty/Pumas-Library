/**
 * Wire types for a library model's stored companion files: its model card,
 * license documents and code. The backend owns discovery, backfill and
 * verification; these types only carry its results to the UI.
 */

export interface ModelDocument {
  path: string;
  size_bytes: number;
  sha256?: string;
  source_url?: string;
  content: string;
}

export interface ModelCodeFile {
  path: string;
  size_bytes: number;
}

export interface ModelDocumentsResponse {
  model_id: string;
  model_card?: ModelDocument;
  licenses: ModelDocument[];
  code_files: ModelCodeFile[];
  requires_custom_code: boolean;
  custom_code_sources: string[];
  backfill_available: boolean;
}

/** Unknown future values are tolerated and rendered as text. */
export type CompanionBackfillOutcome =
  | 'planned'
  | 'fetched'
  | 'up_to_date'
  | 'skipped'
  | 'failed'
  | (string & {});

export interface CompanionBackfillFile {
  path: string;
  kind: string;
  size_bytes: number;
}

export interface CompanionBackfillModelReport {
  model_id: string;
  outcome: CompanionBackfillOutcome;
  reason?: string;
  message?: string;
  repo_id?: string;
  revision?: string;
  revision_binding?: string;
  files: CompanionBackfillFile[];
}

export interface CompanionBackfillReport {
  dry_run: boolean;
  generated_at: string;
  models: CompanionBackfillModelReport[];
  file_count: number;
  total_bytes: number;
  skipped_models: number;
  failed_models: number;
}
