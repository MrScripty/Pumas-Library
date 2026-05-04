import { describe, expect, it } from 'vitest';
import { isModelLibraryUpdateNotification } from './api-package-facts';

describe('model-library update notification contract', () => {
  it('accepts a valid backend notification payload', () => {
    expect(isModelLibraryUpdateNotification({
      cursor: 'model-library-updates:42',
      stale_cursor: false,
      snapshot_required: false,
      events: [{
        cursor: 'model-library-updates:42',
        model_id: 'llm/llama/test',
        change_kind: 'model_added',
        fact_family: 'model_record',
        refresh_scope: 'summary_and_detail',
        selected_artifact_id: null,
        producer_revision: '2026-05-04T00:00:00Z',
      }],
    })).toBe(true);
  });

  it('accepts snapshot notifications without an events array', () => {
    expect(isModelLibraryUpdateNotification({
      cursor: 'model-library-updates:43',
      stale_cursor: true,
      snapshot_required: true,
    })).toBe(true);
  });

  it('rejects malformed or unknown event payloads', () => {
    expect(isModelLibraryUpdateNotification({
      cursor: 'model-library-updates:44',
      stale_cursor: false,
      snapshot_required: false,
      events: [{
        cursor: 'model-library-updates:44',
        model_id: 'llm/llama/test',
        change_kind: 'unknown_change',
        fact_family: 'model_record',
        refresh_scope: 'summary_and_detail',
      }],
    })).toBe(false);

    expect(isModelLibraryUpdateNotification({
      cursor: 'model-library-updates:45',
      stale_cursor: 'false',
      snapshot_required: false,
    })).toBe(false);
  });
});
