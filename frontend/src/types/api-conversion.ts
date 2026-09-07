import type { BaseResponse } from './api-common';
import type {
  ConversionProgressOutcome,
  ConversionProgressResponse,
  ConversionListOutcome,
} from '../generated/desktop-contract';

// ============================================================================
// Model Conversion Types
// ============================================================================

export type ConversionProgress = ConversionProgressOutcome;
export type ConversionDirection = ConversionProgress['direction'];
export type ConversionStatus = ConversionProgress['status'];

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

export type GetConversionProgressResponse = ConversionProgressResponse;

export interface CancelConversionResponse extends BaseResponse {
  cancelled: boolean;
}

export type ListConversionsResponse = ConversionListOutcome;

export interface ConversionEnvironmentResponse extends BaseResponse {
  ready: boolean;
}

export interface SupportedQuantTypesResponse extends BaseResponse {
  quant_types: QuantOption[];
}
