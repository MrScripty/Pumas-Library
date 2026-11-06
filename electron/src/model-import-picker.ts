import type { OpenDialogReturnValue } from 'electron';

import {
  decodeModelImportSelection as decodeGeneratedModelImportSelection,
  type ModelImportSelection,
} from './desktop-contract';

/**
 * Desktop-only selection, not authority to import or mutate model files.
 * Declared in electron/contracts/model-import.schema.json.
 */
export type { ModelImportSelection };

/** Decode a picker result; a malformed result is reported as `invalid`. */
export function decodeModelImportSelection(input: unknown): ModelImportSelection {
  const decoded = decodeGeneratedModelImportSelection(input);
  return decoded.status === 'valid' ? decoded.value : { status: 'invalid' };
}

export async function chooseModelImportPaths(
  choose: () => Promise<OpenDialogReturnValue>,
): Promise<ModelImportSelection> {
  try {
    const result = await choose();
    return decodeModelImportSelection(result.canceled
      ? { status: 'cancelled' }
      : { status: 'selected', paths: result.filePaths });
  } catch {
    // Native errors may contain private paths; expose only operation availability.
    return { status: 'unavailable' };
  }
}
