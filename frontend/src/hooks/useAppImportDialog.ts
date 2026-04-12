import { useCallback, useState } from 'react';
import { getLogger } from '../utils/logger';

const logger = getLogger('useAppImportDialog');

interface UseAppImportDialogOptions {
  onImportComplete: () => Promise<void> | void;
}

export function useAppImportDialog({ onImportComplete }: UseAppImportDialogOptions) {
  const [importPaths, setImportPaths] = useState<string[]>([]);
  const [showImportDialog, setShowImportDialog] = useState(false);

  const handlePathsDropped = useCallback((paths: string[]) => {
    logger.info('Paths dropped for import', { count: paths.length });
    setImportPaths(paths);
    setShowImportDialog(true);
  }, []);

  const handleImportDialogClose = useCallback(() => {
    setShowImportDialog(false);
    setImportPaths([]);
  }, []);

  const handleImportComplete = useCallback(() => {
    logger.info('Import complete, refreshing model list');
    void onImportComplete();
  }, [onImportComplete]);

  return {
    handleImportComplete,
    handleImportDialogClose,
    handlePathsDropped,
    importPaths,
    showImportDialog,
  };
}
