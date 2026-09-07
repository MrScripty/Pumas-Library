import type { OpenDialogReturnValue } from 'electron';

/** Desktop-only selection, not authority to import or mutate model files. */
export type ModelImportSelection =
  | { readonly status: 'selected'; readonly paths: readonly string[] }
  | { readonly status: 'cancelled' | 'invalid' | 'unavailable' };

/** Closed IPC representation; preserve native path spelling, order and duplicates. */
export function decodeModelImportSelection(input: unknown): ModelImportSelection {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return { status: 'invalid' };
  const fields = Object.getOwnPropertyDescriptors(input);
  if (Object.values(fields).some(field => !('value' in field))) return { status: 'invalid' };
  const status: unknown = fields['status']?.value;
  if (status === 'cancelled' || status === 'invalid' || status === 'unavailable') {
    return Object.keys(fields).length === 1 ? Object.freeze({ status }) : { status: 'invalid' };
  }
  const paths: unknown = fields['paths']?.value;
  if (status !== 'selected' || Object.keys(fields).length !== 2 || !Array.isArray(paths) || paths.length === 0) {
    return { status: 'invalid' };
  }
  const copy: string[] = [];
  for (const path of paths) {
    if (typeof path !== 'string' || path.length === 0 || path.includes('\0')) return { status: 'invalid' };
    copy.push(path);
  }
  return Object.freeze({ status: 'selected', paths: Object.freeze(copy) });
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
