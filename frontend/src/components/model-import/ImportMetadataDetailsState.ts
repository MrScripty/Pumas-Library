import type { HFMetadataLookupResult } from '../../types/api';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';
import {
  EXCLUDED_FIELDS,
  formatFieldName,
  formatMetadataValue,
  isHiddenGgufField,
  isPriorityGgufField,
  sortMetadataFields,
} from './metadataUtils';

export interface MetadataEntry {
  key: string;
  label: string;
  value: string;
}

export interface EmbeddedMetadataEntry extends MetadataEntry {
  isPriority: boolean;
  isHidden: boolean;
}

export interface ImportMetadataDetailsState {
  allEmbeddedEntries: EmbeddedMetadataEntry[];
  canShowEmbedded: boolean;
  hasMetadata: boolean;
  hiddenEmbeddedCount: number;
  metadataEntries: MetadataEntry[];
}

function buildHfMetadataEntries(entry: ImportEntryStatus, hasMetadata: boolean): MetadataEntry[] {
  if (!hasMetadata || !entry.hfMetadata) {
    return [];
  }

  return sortMetadataFields(
    Object.keys(entry.hfMetadata).filter(
      (key) =>
        !EXCLUDED_FIELDS.has(key) &&
        entry.hfMetadata?.[key as keyof HFMetadataLookupResult] != null &&
        entry.hfMetadata?.[key as keyof HFMetadataLookupResult] !== ''
    )
  ).map((key) => ({
    key,
    label: formatFieldName(key),
    value: formatMetadataValue(key, entry.hfMetadata?.[key as keyof HFMetadataLookupResult]),
  }));
}

function buildEmbeddedMetadataEntries(entry: ImportEntryStatus): EmbeddedMetadataEntry[] {
  if (!entry.embeddedMetadata) {
    return [];
  }

  return Object.entries(entry.embeddedMetadata)
    .filter(([, value]) => value != null && value !== '')
    .map(([key, value]) => ({
      key,
      label: formatFieldName(key),
      value: formatMetadataValue(key, value),
      isPriority: isPriorityGgufField(key),
      isHidden: isHiddenGgufField(key, value),
    }));
}

export function getImportMetadataDetailsState(
  entry: ImportEntryStatus,
  isShowingAllEmbedded: boolean,
  isShowingEmbedded: boolean
): ImportMetadataDetailsState {
  const hasMetadata = Boolean(entry.hfMetadata && entry.metadataStatus === 'found');
  const canShowEmbedded =
    entry.detectedFileType === 'gguf' || entry.detectedFileType === 'safetensors';
  const allEmbeddedEntries = buildEmbeddedMetadataEntries(entry);
  const embeddedMetadataEntries = allEmbeddedEntries
    .filter((candidate) => isShowingAllEmbedded || (candidate.isPriority && !candidate.isHidden))
    .sort((left, right) => {
      if (left.isPriority !== right.isPriority) return left.isPriority ? -1 : 1;
      return left.key.localeCompare(right.key);
    });

  return {
    allEmbeddedEntries,
    canShowEmbedded,
    hasMetadata,
    hiddenEmbeddedCount: allEmbeddedEntries.length - embeddedMetadataEntries.length,
    metadataEntries: isShowingEmbedded
      ? embeddedMetadataEntries
      : buildHfMetadataEntries(entry, hasMetadata),
  };
}
