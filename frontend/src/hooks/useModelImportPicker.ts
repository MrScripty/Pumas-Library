import { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('useModelImportPicker');

type UseModelImportPickerOptions = {
  onModelsImported?: () => void;
};

export function useModelImportPicker({ onModelsImported }: UseModelImportPickerOptions) {
  const [importPaths, setImportPaths] = useState<string[]>([]);
  const [showImportDialog, setShowImportDialog] = useState(false);
  const [pickerError, setPickerError] = useState<string | null>(null);
  const [isPicking, setIsPicking] = useState(false);
  const mounted = useRef(false);
  const current = useRef<object | null>(null);
  // Native dialogs cannot be cancelled through this bridge. Keep observing the
  // pending request after close, but revoke its authority to open import UI.
  const pending = useRef(false);
  const isMounted = useCallback(() => mounted.current, []);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; current.current = null; };
  }, []);

  const closeImportDialog = useCallback(() => {
    current.current = null;
    setPickerError(null);
    setShowImportDialog(false);
    setImportPaths([]);
  }, []);

  const completeImport = useCallback(() => {
    logger.info('Import complete, refreshing model list');
    onModelsImported?.();
  }, [onModelsImported]);

  const openImportPicker = useCallback(async (): Promise<'selected' | 'cancelled' | 'invalid' | 'unavailable' | 'busy' | 'superseded'> => {
    if (!isMounted()) return 'superseded';
    if (pending.current) return 'busy';
    setPickerError(null);
    if (!isAPIAvailable()) {
      logger.warn('open_model_import_dialog API not available');
      setPickerError('Model file picker unavailable. Try again.');
      return 'unavailable';
    }
    const invocation = {};
    current.current = invocation;
    pending.current = true;
    setIsPicking(true);
    try {
      const result = await api.open_model_import_dialog();
      if (current.current !== invocation) return 'superseded';
      if (result.status === 'selected') {
        logger.info('Import paths selected', { count: result.paths.length });
        setImportPaths([...result.paths]);
        setShowImportDialog(true);
      } else if (result.status !== 'cancelled') {
        setPickerError(result.status === 'invalid'
          ? 'Model file picker returned an invalid selection. Try again.'
          : 'Model file picker unavailable. Try again.');
      }
      return result.status;
    } catch {
      if (current.current !== invocation) return 'superseded';
      logger.error('Failed to open model import dialog');
      setPickerError('Model file picker unavailable. Try again.');
      return 'unavailable';
    } finally {
      pending.current = false;
      if (isMounted()) setIsPicking(false);
    }
  }, [isMounted]);

  return {
    closeImportDialog,
    completeImport,
    importPaths,
    pickerError,
    isPicking,
    openImportPicker,
    showImportDialog,
  };
}
