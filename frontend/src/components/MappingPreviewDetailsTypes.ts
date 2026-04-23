export interface MappingApplyResult {
  success: boolean;
  links_created: number;
  links_removed: number;
  error?: string;
}

export interface MappingCrossFilesystemWarning {
  cross_filesystem: boolean;
  warning?: string;
  recommendation?: string;
}

export type MappingPreviewStatus = 'ready' | 'warnings' | 'errors';
