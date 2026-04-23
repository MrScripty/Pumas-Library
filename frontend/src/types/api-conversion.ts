import type { BaseResponse } from './api-common';

// ============================================================================
// Model Conversion Types
// ============================================================================

export type ConversionDirection = 'gguf_to_safetensors' | 'safetensors_to_gguf';

export type ConversionStatus =
  | 'setting_up'
  | 'validating'
  | 'converting'
  | 'writing'
  | 'importing'
  | 'completed'
  | 'cancelled'
  | 'error';

export interface ConversionProgress {
  conversion_id: string;
  source_model_id: string;
  direction: ConversionDirection;
  status: ConversionStatus;
  progress?: number;
  current_tensor?: string;
  tensors_completed?: number;
  tensors_total?: number;
  bytes_written?: number;
  estimated_output_size?: number;
  target_quant?: string;
  error?: string;
  output_model_id?: string;
}

export interface ConversionSource {
  source_model_id: string;
  source_format: string;
  source_quant?: string;
  target_format: string;
  target_quant?: string;
  was_dequantized: boolean;
  conversion_date: string;
}

export interface QuantOption {
  name: string;
  description: string;
  bits_per_weight: number;
  recommended: boolean;
}

export interface StartConversionResponse extends BaseResponse {
  conversion_id: string;
}

export interface GetConversionProgressResponse extends BaseResponse {
  progress: ConversionProgress | null;
}

export interface CancelConversionResponse extends BaseResponse {
  cancelled: boolean;
}

export interface ListConversionsResponse extends BaseResponse {
  conversions: ConversionProgress[];
}

export interface ConversionEnvironmentResponse extends BaseResponse {
  ready: boolean;
}

export interface SupportedQuantTypesResponse extends BaseResponse {
  quant_types: QuantOption[];
}
