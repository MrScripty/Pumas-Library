import { beforeEach, describe, expect, it } from 'vitest';
import { MODEL_LIBRARY_SNAPSHOT_KEY, readModelLibrarySnapshot, writeModelLibrarySnapshot } from './modelLibrarySnapshot';

const scope = `display-v1:${'a'.repeat(64)}`;
const writtenAt = new Date('2026-11-05T10:00:00.000Z');

function storedSnapshot(): Record<string, unknown> {
  return JSON.parse(localStorage.getItem(MODEL_LIBRARY_SNAPSHOT_KEY) ?? 'null') as Record<string, unknown>;
}

describe('root-scoped display-only library snapshot', () => {
  beforeEach(() => localStorage.clear());

  it('reopens display fields without model paths or download authority', () => {
    expect(writeModelLibrarySnapshot([{ category: 'llm', models: [{
      id: 'llm/example', name: 'Example', category: 'llm', format: 'gguf',
      modelDir: '/models/example', path: 'llm/example', repoId: 'org/example',
      selectedArtifactId: 'artifact', isDownloading: true, downloadKey: 'download',
    }] }], scope, writtenAt)).toBe(true);
    expect(readModelLibrarySnapshot(scope)).toEqual({
      writtenAt: writtenAt.toISOString(),
      revision: 1,
      modelGroups: [{ category: 'llm', models: [{
        id: 'llm/example', name: 'Example', category: 'llm', format: 'gguf', provenance: 'cached',
      }] }],
    });
    expect(localStorage.getItem(MODEL_LIBRARY_SNAPSHOT_KEY)).not.toContain('/models/example');
  });

  it('records when each write happened and increases the revision per scope', () => {
    const groups = [{ category: 'llm', models: [{ id: 'one', name: 'One', category: 'llm' }] }];
    writeModelLibrarySnapshot(groups, scope, writtenAt);
    const later = new Date('2026-11-05T11:30:00.000Z');
    writeModelLibrarySnapshot(groups, scope, later);
    expect(storedSnapshot()).toMatchObject({ libraryScopeId: scope, writtenAt: later.toISOString(), revision: 2 });

    const otherScope = `display-v1:${'c'.repeat(64)}`;
    writeModelLibrarySnapshot(groups, otherScope, later);
    expect(storedSnapshot()).toMatchObject({ libraryScopeId: otherScope, revision: 1 });
  });

  it('rejects another root, missing scope, and retired v1 or v2 data', () => {
    writeModelLibrarySnapshot([{ category: 'llm', models: [{ id: 'one', name: 'One', category: 'llm' }] }], scope);
    expect(readModelLibrarySnapshot(`display-v1:${'b'.repeat(64)}`)).toBeNull();
    expect(readModelLibrarySnapshot(null)).toBeNull();
    localStorage.setItem('pumas:model-library:v1', JSON.stringify({ modelGroups: [] }));
    localStorage.setItem('pumas:model-library:v2', JSON.stringify({ libraryScopeId: scope, modelGroups: [] }));
    expect(readModelLibrarySnapshot(scope)).toBeNull();
    expect(localStorage.getItem('pumas:model-library:v1')).toBeNull();
    expect(localStorage.getItem('pumas:model-library:v2')).toBeNull();
  });

  it('rejects a snapshot without valid write provenance', () => {
    const modelGroups = [{ category: 'llm', models: [{ id: 'one', name: 'One', category: 'llm' }] }];
    for (const provenance of [
      {},
      { writtenAt: 'not-a-date', revision: 1 },
      { writtenAt: writtenAt.toISOString(), revision: 0 },
      { writtenAt: writtenAt.toISOString(), revision: 1.5 },
    ]) {
      localStorage.setItem(MODEL_LIBRARY_SNAPSHOT_KEY, JSON.stringify({ libraryScopeId: scope, modelGroups, ...provenance }));
      expect(readModelLibrarySnapshot(scope)).toBeNull();
      expect(localStorage.getItem(MODEL_LIBRARY_SNAPSHOT_KEY)).toBeNull();
    }
  });

  it('rejects a snapshot containing an unknown capability field', () => {
    writeModelLibrarySnapshot([{ category: 'llm', models: [{ id: 'one', name: 'One', category: 'llm' }] }], scope);
    const value = { libraryScopeId: scope, writtenAt: writtenAt.toISOString(), revision: 1, modelGroups: [{
      category: 'llm', models: [{ id: 'one', name: 'One', category: 'llm', recoveryToken: 'injected' }],
    }] };
    localStorage.setItem(MODEL_LIBRARY_SNAPSHOT_KEY, JSON.stringify(value));
    expect(readModelLibrarySnapshot(scope)).toBeNull();
    expect(localStorage.getItem(MODEL_LIBRARY_SNAPSHOT_KEY)).toBeNull();
  });
});
