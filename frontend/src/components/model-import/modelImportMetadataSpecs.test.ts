import { describe, expect, it } from 'vitest';
import {
  buildEmbeddedMetadataMatch,
  buildImportBatchSpecs,
  extractEmbeddedRepoId,
} from './modelImportWorkflowHelpers';

describe('modelImportMetadataSpecs', () => {
  it('extracts embedded Hugging Face repo ids from repo URLs and quantized metadata fallbacks', () => {
    expect(extractEmbeddedRepoId({
      'general.repo_url': 'https://huggingface.co/Qwen/Qwen3-8B-GGUF',
      'general.quantized_by': 'ignored',
      'general.name': 'ignored',
    })).toBe('Qwen/Qwen3-8B-GGUF');

    expect(extractEmbeddedRepoId({
      'general.quantized_by': 'bartowski',
      'general.name': 'Qwen3-8B-GGUF',
    })).toBe('bartowski/Qwen3-8B-GGUF');

    expect(extractEmbeddedRepoId({
      'general.name': 'Qwen3-8B-GGUF',
    })).toBeNull();
  });

  it('builds embedded metadata matches and import batch specs from reviewed entries', () => {
    const embeddedMatch = buildEmbeddedMetadataMatch({
      path: '/imports/model.gguf',
      originPath: '/imports/model.gguf',
      filename: 'model.gguf',
      kind: 'single_file',
      status: 'pending',
      suggestedFamily: 'qwen',
      suggestedOfficialName: 'Qwen3 8B',
    }, 'Qwen/Qwen3-8B-GGUF');

    expect(embeddedMatch).toEqual({
      repo_id: 'Qwen/Qwen3-8B-GGUF',
      official_name: 'Qwen3 8B',
      family: 'qwen',
      match_method: 'filename_exact',
      match_confidence: 0.9,
      requires_confirmation: false,
    });

    const specs = buildImportBatchSpecs([
      {
        path: '/imports/flux',
        originPath: '/imports/flux',
        filename: 'flux',
        kind: 'external_diffusers_bundle',
        status: 'pending',
        securityAcknowledged: true,
        suggestedFamily: 'imported',
        suggestedOfficialName: 'FLUX.1 dev',
        modelType: 'llm',
        hfMetadata: {
          repo_id: 'black-forest-labs/FLUX.1-dev',
          official_name: 'FLUX.1-dev',
          family: 'flux',
          model_type: 'ignored',
          subtype: 'dev',
          tags: ['text-to-image'],
        },
      },
      {
        path: '/imports/model.gguf',
        originPath: '/imports/model.gguf',
        filename: 'model.gguf',
        kind: 'single_file',
        status: 'pending',
        securityAcknowledged: true,
        suggestedFamily: 'imported',
        suggestedOfficialName: 'Qwen3 8B',
        modelType: 'llm',
        hfMetadata: {
          repo_id: 'Qwen/Qwen3-8B-GGUF',
          official_name: 'Qwen3-8B-GGUF',
          family: 'qwen',
          model_type: 'vlm',
          tags: ['multimodal'],
        },
      },
      {
        path: '/imports/manual.gguf',
        originPath: '/imports/manual.gguf',
        filename: 'manual.gguf',
        kind: 'single_file',
        status: 'pending',
        securityAcknowledged: false,
        suggestedFamily: 'manual-family',
        suggestedOfficialName: 'Manual Name',
        modelType: 'embedding',
      },
    ]);

    expect(specs).toEqual([
      {
        path: '/imports/flux',
        family: 'black-forest-labs',
        official_name: 'FLUX.1-dev',
        repo_id: 'black-forest-labs/FLUX.1-dev',
        model_type: 'diffusion',
        subtype: 'dev',
        tags: ['text-to-image'],
        security_acknowledged: true,
      },
      {
        path: '/imports/model.gguf',
        family: 'qwen',
        official_name: 'Qwen3-8B-GGUF',
        repo_id: 'Qwen/Qwen3-8B-GGUF',
        model_type: 'vlm',
        tags: ['multimodal'],
        security_acknowledged: true,
      },
      {
        path: '/imports/manual.gguf',
        family: 'manual-family',
        official_name: 'Manual Name',
        model_type: 'embedding',
        security_acknowledged: false,
      },
    ]);
  });
});
