import type {
  BundleComponentManifestEntry,
  BundleFormat,
  HFMetadataLookupResult,
  ImportPathCandidate,
  SecurityTier,
} from '../../types/api';

export type ImportStep = 'classifying' | 'review' | 'lookup' | 'importing' | 'complete';
export type MetadataStatus = 'pending' | 'found' | 'not_found' | 'error' | 'manual';
export type ImportEntryKind = 'single_file' | 'directory_model' | 'external_diffusers_bundle';

export interface ImportEntryStatus {
  path: string;
  originPath: string;
  filename: string;
  kind: ImportEntryKind;
  status: 'pending' | 'importing' | 'success' | 'error';
  error?: string;
  securityTier?: SecurityTier;
  securityAcknowledged?: boolean;
  hfMetadata?: HFMetadataLookupResult;
  metadataStatus?: MetadataStatus;
  shardedSetKey?: string;
  validFileType?: boolean;
  detectedFileType?: string;
  embeddedMetadata?: Record<string, unknown>;
  embeddedMetadataStatus?: 'pending' | 'loaded' | 'error' | 'unsupported';
  suggestedFamily: string;
  suggestedOfficialName: string;
  modelType?: string;
  bundleFormat?: BundleFormat;
  pipelineClass?: string;
  componentManifest?: BundleComponentManifestEntry[];
  containerPath?: string;
}

export interface ShardedSetInfo {
  key: string;
  files: string[];
  complete: boolean;
  missingShards: number[];
  expanded: boolean;
}

export interface DirectoryReviewFinding {
  path: string;
  kind: 'multi_model_container' | 'ambiguous' | 'unsupported';
  reasons: string[];
  candidates: ImportPathCandidate[];
}
