import { useCallback, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('useModelImportPicker');

type UseModelImportPickerOptions = {
  onModelsImported?: () => void;
};

export function useModelImportPicker({ onModelsImported }: UseModelImportPickerOptions) {
  const [importPaths, setImportPaths] = useState<string[]>([]);
  const [showImportDialog, setShowImportDialog] = useState(false);

  const closeImportDialog = useCallback(() => {
    setShowImportDialog(false);
    setImportPaths([]);
  }, []);

  const completeImport = useCallback(() => {
    logger.info('Import complete, refreshing model list');
    onModelsImported?.();
  }, [onModelsImported]);

  const openImportPicker = useCallback(async () => {
    if (!isAPIAvailable()) {
      logger.warn('open_model_import_dialog API not available');
      return;
    }

    try {
      const result = await api.open_model_import_dialog();
      if (result.success && result.paths.length > 0) {
        logger.info('Import paths selected', { count: result.paths.length });
        setImportPaths(result.paths);
        setShowImportDialog(true);
      }
    } catch (error) {
      logger.error('Failed to open model import dialog', { error });
    }
  }, []);

  return {
    closeImportDialog,
    completeImport,
    importPaths,
    openImportPicker,
    showImportDialog,
  };
}
