import type { ModelCategory, ModelInfo } from '../types/apps';

export const MODEL_LIBRARY_SNAPSHOT_KEY = 'pumas:model-library:v3';
const RETIRED_SNAPSHOT_KEYS = ['pumas:model-library:v1', 'pumas:model-library:v2'] as const;
const SNAPSHOT_KEYS = new Set(['libraryScopeId', 'writtenAt', 'revision', 'modelGroups']);

/** A persisted display snapshot plus the provenance needed to label it as cached. */
export interface ModelLibrarySnapshot {
  modelGroups: ModelCategory[];
  /** ISO-8601 time the snapshot was written from a successful library load. */
  writtenAt: string;
  /** Write counter for this library scope; increases on every persisted write. */
  revision: number;
}

const DISPLAY_STRINGS = ['format', 'quant', 'date', 'integrityIssueMessage'] as const;
const DISPLAY_NUMBERS = ['size', 'downloadProgress', 'dependencyCount'] as const;
const DISPLAY_FLAGS = ['isPartialDownload', 'hasDependencies', 'hasIntegrityIssue'] as const;
const DISPLAY_KEYS = new Set<string>([
  'id', 'name', 'category', ...DISPLAY_STRINGS, ...DISPLAY_NUMBERS, ...DISPLAY_FLAGS,
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isDisplayString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= 4096;
}

function isScope(value: unknown): value is string {
  return typeof value === 'string' && /^display-v1:[a-f0-9]{64}$/.test(value);
}

function isWrittenAt(value: unknown): value is string {
  return typeof value === 'string' && value.length <= 64 && Number.isFinite(Date.parse(value));
}

function isRevision(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}

function decodeDisplayModel(value: unknown, category: string): ModelInfo | null {
  if (!isRecord(value) || Object.keys(value).some((key) => !DISPLAY_KEYS.has(key)) ||
    !isDisplayString(value['id']) || !isDisplayString(value['name']) || value['category'] !== category) {
    return null;
  }
  if (DISPLAY_STRINGS.some((key) => value[key] !== undefined && !isDisplayString(value[key])) ||
    DISPLAY_FLAGS.some((key) => value[key] !== undefined && typeof value[key] !== 'boolean') ||
    DISPLAY_NUMBERS.some((key) => value[key] !== undefined &&
      (typeof value[key] !== 'number' || !Number.isFinite(value[key]) || value[key] < 0))) {
    return null;
  }
  if (typeof value['downloadProgress'] === 'number' && value['downloadProgress'] >= 1) return null;
  if (typeof value['size'] === 'number' && !Number.isSafeInteger(value['size'])) return null;
  if (typeof value['dependencyCount'] === 'number' && !Number.isSafeInteger(value['dependencyCount'])) return null;
  return { ...value, id: value['id'], name: value['name'], category, provenance: 'cached' };
}

function decodeGroups(value: unknown): ModelCategory[] | null {
  if (!Array.isArray(value)) return null;
  const groups: ModelCategory[] = [];
  const ids = new Set<string>();
  const categories = new Set<string>();
  for (const group of value) {
    if (!isRecord(group) || Object.keys(group).some((key) => key !== 'category' && key !== 'models') ||
      !isDisplayString(group['category']) || !Array.isArray(group['models']) || categories.has(group['category'])) {
      return null;
    }
    categories.add(group['category']);
    const models: ModelInfo[] = [];
    for (const value of group['models']) {
      const model = decodeDisplayModel(value, group['category']);
      if (!model || ids.has(model.id)) return null;
      ids.add(model.id);
      models.push(model);
    }
    groups.push({ category: group['category'], models });
  }
  return groups;
}

function displayOnly(model: ModelInfo): Record<string, unknown> {
  const display: Record<string, unknown> = { id: model.id, name: model.name, category: model.category };
  for (const key of [...DISPLAY_STRINGS, ...DISPLAY_NUMBERS, ...DISPLAY_FLAGS]) {
    if (model[key] !== undefined) display[key] = model[key];
  }
  return display;
}

export function toDisplayOnlyModelGroups(modelGroups: ModelCategory[]): ModelCategory[] {
  return modelGroups.map((group) => ({
    category: group.category,
    models: group.models.map((model) => ({
      ...displayOnly(model),
      id: model.id,
      name: model.name,
      category: model.category,
      provenance: 'cached',
    })),
  }));
}

function getLocalStorage(): Storage | null {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch {
    return null;
  }
}

function removeRetiredSnapshots(storage: Storage): void {
  for (const key of RETIRED_SNAPSHOT_KEYS) storage.removeItem(key);
}

function decodeSnapshot(value: unknown, libraryScopeId: string): ModelLibrarySnapshot | null {
  if (!isRecord(value) || Object.keys(value).some((key) => !SNAPSHOT_KEYS.has(key)) ||
    Object.keys(value).length !== SNAPSHOT_KEYS.size || value['libraryScopeId'] !== libraryScopeId ||
    !isWrittenAt(value['writtenAt']) || !isRevision(value['revision'])) {
    return null;
  }
  const modelGroups = decodeGroups(value['modelGroups']);
  return modelGroups ? { modelGroups, writtenAt: value['writtenAt'], revision: value['revision'] } : null;
}

function readStoredRevision(storage: Storage, libraryScopeId: string): number {
  try {
    const serialized = storage.getItem(MODEL_LIBRARY_SNAPSHOT_KEY);
    const parsed: unknown = serialized ? JSON.parse(serialized) : null;
    return isRecord(parsed) && parsed['libraryScopeId'] === libraryScopeId && isRevision(parsed['revision'])
      ? parsed['revision'] : 0;
  } catch {
    return 0;
  }
}

/** Disposable display cache only: never a source of paths, activity, or action authority. */
export function readModelLibrarySnapshot(libraryScopeId: string | null): ModelLibrarySnapshot | null {
  const storage = getLocalStorage();
  if (!storage) return null;
  try {
    removeRetiredSnapshots(storage);
    const serialized = storage.getItem(MODEL_LIBRARY_SNAPSHOT_KEY);
    if (!serialized) return null;
    const snapshot = isScope(libraryScopeId) ? decodeSnapshot(JSON.parse(serialized), libraryScopeId) : null;
    if (snapshot) return snapshot;
    storage.removeItem(MODEL_LIBRARY_SNAPSHOT_KEY);
  } catch {
    try { storage.removeItem(MODEL_LIBRARY_SNAPSHOT_KEY); } catch { /* Storage may be unavailable. */ }
  }
  return null;
}

export function writeModelLibrarySnapshot(
  modelGroups: ModelCategory[],
  libraryScopeId: string | null,
  writtenAt: Date = new Date(),
): boolean {
  const storage = getLocalStorage();
  if (!storage || !isScope(libraryScopeId) || !Number.isFinite(writtenAt.getTime())) return false;
  try {
    const groups = modelGroups.map((group) => ({ category: group.category, models: group.models.map(displayOnly) }));
    if (!decodeGroups(groups)) return false;
    removeRetiredSnapshots(storage);
    const revision = readStoredRevision(storage, libraryScopeId) + 1;
    if (!isRevision(revision)) return false;
    storage.setItem(MODEL_LIBRARY_SNAPSHOT_KEY, JSON.stringify({
      libraryScopeId,
      writtenAt: writtenAt.toISOString(),
      revision,
      modelGroups: groups,
    }));
    return true;
  } catch {
    return false;
  }
}
