export interface MappingAction {
  model_id: string;
  model_name: string;
  source_path: string;
  target_path: string;
  link_type?: string;
  reason: string;
  existing_target?: string;
}

export interface MappingPreviewResponse {
  success: boolean;
  error?: string;
  to_create: MappingAction[];
  to_skip_exists: MappingAction[];
  conflicts: MappingAction[];
  broken_to_remove: Array<{
    target_path: string;
    existing_target: string;
    reason: string;
  }>;
  total_actions: number;
  warnings: string[];
  errors: string[];
}
