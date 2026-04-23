import type { ImportEntryStatus } from './modelImportWorkflowTypes';
import {
  EmbeddedMetadataDisclosure,
  EmbeddedMetadataStatusMessage,
  MetadataEntriesGrid,
  MetadataSourceToggle,
  NoEmbeddedMetadataMessage,
  NoHfMetadataMessage,
} from './ImportMetadataDetailsParts';
import { getImportMetadataDetailsState } from './ImportMetadataDetailsState';

interface ImportMetadataDetailsProps {
  entry: ImportEntryStatus;
  isShowingAllEmbedded: boolean;
  isShowingEmbedded: boolean;
  onToggleMetadataSource: (path: string) => Promise<void>;
  onToggleShowAllEmbeddedMetadata: (path: string) => void;
}

export function ImportMetadataDetails({
  entry,
  isShowingAllEmbedded,
  isShowingEmbedded,
  onToggleMetadataSource,
  onToggleShowAllEmbeddedMetadata,
}: ImportMetadataDetailsProps) {
  const {
    allEmbeddedEntries,
    canShowEmbedded,
    hasMetadata,
    hiddenEmbeddedCount,
    metadataEntries,
  } = getImportMetadataDetailsState(entry, isShowingAllEmbedded, isShowingEmbedded);

  return (
    <div className="ml-8 border-t border-[hsl(var(--launcher-border)/0.5)] px-3 pb-3 pt-1">
      {canShowEmbedded && (
        <MetadataSourceToggle
          isShowingEmbedded={isShowingEmbedded}
          path={entry.path}
          onToggleMetadataSource={onToggleMetadataSource}
        />
      )}

      <EmbeddedMetadataStatusMessage
        isShowingEmbedded={isShowingEmbedded}
        status={entry.embeddedMetadataStatus}
      />
      <MetadataEntriesGrid
        entry={entry}
        isShowingEmbedded={isShowingEmbedded}
        metadataEntries={metadataEntries}
      />
      <NoHfMetadataMessage
        hasMetadata={hasMetadata}
        isShowingEmbedded={isShowingEmbedded}
        metadataCount={metadataEntries.length}
      />
      <EmbeddedMetadataDisclosure
        hiddenEmbeddedCount={hiddenEmbeddedCount}
        isShowingAllEmbedded={isShowingAllEmbedded}
        isShowingEmbedded={isShowingEmbedded}
        path={entry.path}
        status={entry.embeddedMetadataStatus}
        onToggleShowAllEmbeddedMetadata={onToggleShowAllEmbeddedMetadata}
      />
      <NoEmbeddedMetadataMessage
        allEmbeddedCount={allEmbeddedEntries.length}
        isShowingEmbedded={isShowingEmbedded}
        status={entry.embeddedMetadataStatus}
      />
    </div>
  );
}
