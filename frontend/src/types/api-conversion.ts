import type {
  ConversionProgressOutcome,
  ConversionProgressResponse,
  ConversionListOutcome,
  ConversionStartedOutcome,
  ConversionCancelledOutcome,
  ConversionEnvironmentOutcome,
  ConversionSetupStartedOutcome,
  ConversionSetupStatusOutcome,
  SupportedQuantTypesOutcome,
  BackendStatusOutcome,
  SuccessOutcome,
} from '../generated/desktop-contract';
export type { QuantBackend } from '../generated/desktop-contract';

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

export type QuantOption = SupportedQuantTypesOutcome['quant_types'][number];
export type StartConversionResponse = ConversionStartedOutcome;

export type GetConversionProgressResponse = ConversionProgressResponse;

export type CancelConversionResponse = ConversionCancelledOutcome;

export type ListConversionsResponse = ConversionListOutcome;

export type ConversionEnvironmentResponse = ConversionEnvironmentOutcome;
export type SupportedQuantTypesResponse = SupportedQuantTypesOutcome;
export type ConversionBackendStatusResponse = BackendStatusOutcome;
export type ConversionSetupResponse = SuccessOutcome;
export type ConversionSetupStartedResponse = ConversionSetupStartedOutcome;
export type ConversionSetupStatusResponse = ConversionSetupStatusOutcome;
