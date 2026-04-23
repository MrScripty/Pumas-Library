import { describe, expect, it } from 'vitest';
import {
  buildEntries,
  buildReviewFindings,
  buildShardedSetState,
} from './modelImportWorkflowHelpers';

describe('modelImportWorkflowHelpers', () => {
  it('builds import entries from backend classifications and deduplicates repeated paths', () => {
    const bundleManifest = [
      {
        name: 'unet',
        relative_path: 'unet/model.safetensors',
        state: 'present' as const,
      },
    ];

    const entries = buildEntries([
      {
        path: '/imports/unsafe.pt',
        kind: 'single_file',
        suggested_family: 'qwen',
        suggested_official_name: null,
        model_type: 'llm',
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: [],
        candidates: [],
      },
      {
        path: '/imports/model-dir',
        kind: 'single_model_directory',
        suggested_family: null,
        suggested_official_name: 'Directory Official',
        model_type: 'embedding',
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: [],
        candidates: [],
      },
      {
        path: '/imports/diffusers',
        kind: 'single_bundle',
        suggested_family: 'flux',
        suggested_official_name: 'Flux Bundle',
        model_type: 'diffusion',
        bundle_format: 'diffusers_directory',
        pipeline_class: 'FluxPipeline',
        component_manifest: bundleManifest,
        reasons: [],
        candidates: [],
      },
      {
        path: '/imports/container',
        kind: 'multi_model_container',
        suggested_family: null,
        suggested_official_name: null,
        model_type: null,
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: ['contains multiple importable models'],
        candidates: [
          {
            path: '/imports/container/model-a.gguf',
            kind: 'file_model',
            display_name: 'model-a.gguf',
            model_type: 'llm',
            bundle_format: null,
            pipeline_class: null,
            component_manifest: null,
            reasons: [],
          },
          {
            path: '/imports/container/subdir',
            kind: 'directory_model',
            display_name: 'subdir',
            model_type: 'embedding',
            bundle_format: null,
            pipeline_class: null,
            component_manifest: null,
            reasons: [],
          },
          {
            path: '/imports/container/bundle',
            kind: 'external_diffusers_bundle',
            display_name: 'bundle',
            model_type: 'diffusion',
            bundle_format: 'diffusers_directory',
            pipeline_class: 'StableDiffusionPipeline',
            component_manifest: bundleManifest,
            reasons: [],
          },
          {
            path: '/imports/unsafe.pt',
            kind: 'file_model',
            display_name: 'unsafe.pt',
            model_type: 'llm',
            bundle_format: null,
            pipeline_class: null,
            component_manifest: null,
            reasons: [],
          },
        ],
      },
    ]);

    expect(entries).toHaveLength(6);

    expect(entries.find((entry) => entry.path === '/imports/unsafe.pt')).toMatchObject({
      originPath: '/imports/unsafe.pt',
      kind: 'single_file',
      filename: 'unsafe.pt',
      securityTier: 'pickle',
      securityAcknowledged: false,
      metadataStatus: 'pending',
      suggestedFamily: 'qwen',
      suggestedOfficialName: 'unsafe',
      modelType: 'llm',
    });

    expect(entries.find((entry) => entry.path === '/imports/model-dir')).toMatchObject({
      kind: 'directory_model',
      metadataStatus: 'manual',
      suggestedFamily: 'imported',
      suggestedOfficialName: 'Directory Official',
    });

    expect(entries.find((entry) => entry.path === '/imports/diffusers')).toMatchObject({
      kind: 'external_diffusers_bundle',
      metadataStatus: 'pending',
      bundleFormat: 'diffusers_directory',
      pipelineClass: 'FluxPipeline',
      componentManifest: bundleManifest,
    });

    expect(entries.find((entry) => entry.path === '/imports/container/model-a.gguf')).toMatchObject({
      originPath: '/imports/container',
      containerPath: '/imports/container',
      kind: 'single_file',
      suggestedFamily: 'imported',
      suggestedOfficialName: 'model-a',
    });

    expect(entries.find((entry) => entry.path === '/imports/container/subdir')).toMatchObject({
      originPath: '/imports/container',
      containerPath: '/imports/container',
      kind: 'directory_model',
      metadataStatus: 'manual',
    });

    expect(entries.find((entry) => entry.path === '/imports/container/bundle')).toMatchObject({
      originPath: '/imports/container',
      containerPath: '/imports/container',
      kind: 'external_diffusers_bundle',
      bundleFormat: 'diffusers_directory',
      pipelineClass: 'StableDiffusionPipeline',
      componentManifest: bundleManifest,
    });
  });

  it('keeps only reviewable directory findings', () => {
    const findings = buildReviewFindings([
      {
        path: '/imports/file.gguf',
        kind: 'single_file',
        suggested_family: null,
        suggested_official_name: null,
        model_type: null,
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: ['safe to import directly'],
        candidates: [],
      },
      {
        path: '/imports/container',
        kind: 'multi_model_container',
        suggested_family: null,
        suggested_official_name: null,
        model_type: null,
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: ['contains multiple importable models'],
        candidates: [],
      },
      {
        path: '/imports/ambiguous',
        kind: 'ambiguous',
        suggested_family: null,
        suggested_official_name: null,
        model_type: null,
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: ['classification conflict'],
        candidates: [],
      },
      {
        path: '/imports/unsupported',
        kind: 'unsupported',
        suggested_family: null,
        suggested_official_name: null,
        model_type: null,
        bundle_format: null,
        pipeline_class: null,
        component_manifest: null,
        reasons: ['unsupported extension'],
        candidates: [],
      },
    ]);

    expect(findings).toEqual([
      {
        path: '/imports/container',
        kind: 'multi_model_container',
        reasons: ['contains multiple importable models'],
        candidates: [],
      },
      {
        path: '/imports/ambiguous',
        kind: 'ambiguous',
        reasons: ['classification conflict'],
        candidates: [],
      },
      {
        path: '/imports/unsupported',
        kind: 'unsupported',
        reasons: ['unsupported extension'],
        candidates: [],
      },
    ]);
  });

  it('builds sharded set state only for multi-file groups', () => {
    const state = buildShardedSetState({
      complete: {
        files: ['/models/shard-00001.gguf', '/models/shard-00002.gguf'],
        validation: {
          complete: true,
          missing_shards: [],
          total_expected: 2,
          total_found: 2,
        },
      },
      singleton: {
        files: ['/models/standalone.gguf'],
        validation: {
          complete: true,
          missing_shards: [],
          total_expected: 1,
          total_found: 1,
        },
      },
    });

    expect(state.sets).toEqual([
      {
        key: 'complete',
        files: ['/models/shard-00001.gguf', '/models/shard-00002.gguf'],
        complete: true,
        missingShards: [],
        expanded: false,
      },
    ]);
    expect(state.fileToSetMap).toEqual({
      '/models/shard-00001.gguf': 'complete',
      '/models/shard-00002.gguf': 'complete',
    });
  });

});
