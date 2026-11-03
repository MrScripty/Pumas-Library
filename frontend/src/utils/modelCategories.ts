/**
 * Display rules for model categories and modalities.
 *
 * The backend records one category per model: what a person uses it for.
 * Every input and output it handles is listed separately as a modality.
 * New categories and modalities may appear; unrecognized ones keep their raw
 * name and sort last.
 */

/** Categories a person can choose, in library display order. */
export const CHOOSABLE_MODEL_CATEGORIES = [
  'llm',
  'vlm',
  'decision',
  'embedding',
  'reranker',
  'diffusion',
  'video',
  'audio',
  'vision',
] as const;

export type ChoosableModelCategory = (typeof CHOOSABLE_MODEL_CATEGORIES)[number];

const CATEGORY_LABELS: Record<string, string> = {
  llm: 'Language models',
  vlm: 'Vision-language',
  decision: 'Decision and evaluation',
  embedding: 'Embedding',
  reranker: 'Reranking',
  diffusion: 'Image generation',
  video: 'Video generation',
  audio: 'Audio and speech',
  vision: 'Vision',
  unknown: 'Unclassified',
};

const MODALITY_LABELS: Record<string, string> = {
  text: 'Text',
  image: 'Image',
  audio: 'Audio',
  video: 'Video',
  embedding: 'Vectors',
  decision: 'Decisions',
  document: 'Document',
  mask: 'Mask',
  depth: 'Depth',
  bbox: 'Boxes',
  keypoints: 'Keypoints',
  '3d': '3D',
  tabular: 'Table',
  timeseries: 'Time series',
  any: 'Any',
};

export function modelCategoryLabel(category: string): string {
  return CATEGORY_LABELS[category] ?? category;
}

export function modalityLabel(modality: string): string {
  return MODALITY_LABELS[modality] ?? modality;
}

/** Sort position: known categories in display order, unknown last, others after. */
export function modelCategoryRank(category: string): number {
  const known = (CHOOSABLE_MODEL_CATEGORIES as readonly string[]).indexOf(category);
  if (known >= 0) return known;
  return category === 'unknown' ? CHOOSABLE_MODEL_CATEGORIES.length : CHOOSABLE_MODEL_CATEGORIES.length + 1;
}

export function isChoosableModelCategory(value: string): value is ChoosableModelCategory {
  return (CHOOSABLE_MODEL_CATEGORIES as readonly string[]).includes(value);
}
