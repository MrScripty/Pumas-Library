import { useId, useRef, type RefObject } from 'react';
import type { ModelInfo } from '../types/apps';
import type { ConversionDirection, ConversionStatus } from '../types/api-conversion';
import { isConversionTerminal, useModelConversionWorkflow } from '../hooks/useModelConversionWorkflow';
import { ModalDialog } from './ui';

const statusLabels: Record<ConversionStatus, string> = {
  setting_up: 'Setting up', validating: 'Validating', converting: 'Converting',
  writing: 'Writing output', importing: 'Adding to library', completed: 'Completed',
  cancelled: 'Cancelled', error: 'Failed', building_toolchain: 'Building tools',
  generating_f16_gguf: 'Generating F16 GGUF', computing_imatrix: 'Computing importance matrix',
  quantizing: 'Quantizing', calibrating: 'Calibrating', training: 'Training',
};

type FormatConversionDirection = Extract<ConversionDirection, 'gguf_to_safetensors' | 'safetensors_to_gguf'>;

export function formatConversionDirection(format: string | undefined): FormatConversionDirection | null {
  if (format === 'gguf') return 'gguf_to_safetensors';
  if (format === 'safetensors') return 'safetensors_to_gguf';
  return null;
}

interface ModelConversionDialogProps {
  model: ModelInfo;
  direction: FormatConversionDirection;
  onClose: () => void;
  onCompleted?: () => void;
  restoreFocusFallbackRef?: RefObject<HTMLElement | null>;
}

const buttonClass = 'rounded border border-[hsl(var(--launcher-border))] px-3 py-2 disabled:opacity-50';

export function ModelConversionDialog({ model, direction, onClose, onCompleted, restoreFocusFallbackRef }: ModelConversionDialogProps) {
  const titleId = useId();
  const closeRef = useRef<HTMLButtonElement>(null);
  const workflow = useModelConversionWorkflow({ modelId: model.id, direction, onCompleted });
  const active = workflow.conversions.some(item => !isConversionTerminal(item.status));
  const target = direction === 'gguf_to_safetensors' ? 'Safetensors (F16)' : 'GGUF (F16)';
  return (
    <ModalDialog isOpen ariaLabelledBy={titleId} onClose={onClose}
      dismissDisabled={workflow.busy} initialFocusRef={closeRef} shouldCloseOnBackdrop={false}
      restoreFocusFallbackRef={restoreFocusFallbackRef}
      contentClassName="w-full max-w-xl rounded-xl border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))] p-6 text-[hsl(var(--launcher-text-primary))]">
      <div className="flex items-center justify-between gap-4">
        <h2 id={titleId} className="text-lg font-semibold">Convert model format</h2>
        <button ref={closeRef} className={buttonClass} disabled={workflow.busy} onClick={onClose}>Close</button>
      </div>
      <div className="mt-4 max-h-[65vh] space-y-4 overflow-y-auto">
        <p className="break-words">{model.name}</p>
        <p>Output: {target}. The source model is kept.</p>
        {direction === 'gguf_to_safetensors' && <p>Dequantization does not restore precision lost during quantization. Output may require substantially more disk space.</p>}
        <p>Closing this dialog does not cancel a conversion. Reopen it to check backend progress.</p>
        {workflow.error && <p role="alert">{workflow.error}</p>}
        {workflow.loading && !workflow.busy && workflow.conversions.length === 0 && <p role="status">Checking conversion status…</p>}
        {workflow.awaitingProgress && <p role="status">Conversion accepted. Waiting for backend progress…</p>}
        {workflow.busy && <p role="status">Waiting for the backend… Tool setup may take several minutes.</p>}
        {workflow.ready === false && <div className="space-y-2">
          <p>Conversion tools are not ready. Setup downloads and installs Python conversion dependencies in this library’s launcher data. This uses network access and disk space.</p>
          <button className={buttonClass} disabled={workflow.busy || workflow.loading || active || workflow.setupUncertain} onClick={() => { void workflow.setup(); }}>Install conversion tools</button>
        </div>}
        <div className="flex flex-wrap gap-2">
          <button className={buttonClass} disabled={workflow.busy || workflow.loading} onClick={workflow.refresh}>Refresh status</button>
          <button className={buttonClass} disabled={workflow.ready !== true || workflow.busy || workflow.loading || active || workflow.awaitingProgress || workflow.startUncertain || workflow.error !== null}
            onClick={() => { void workflow.start(); }}>Start conversion</button>
        </div>
        {workflow.conversions.map(item => <section key={item.conversionId} className="space-y-2 rounded border border-[hsl(var(--launcher-border))] p-3" aria-label={`Conversion ${item.conversionId}`}>
          <p role={isConversionTerminal(item.status) ? 'status' : undefined}>{statusLabels[item.status]}</p>
          {item.progress === null
            ? <p>Progress unavailable</p>
            : <><progress className="w-full" aria-label="Conversion progress" max={1} value={item.progress} /><p>{Math.round(item.progress * 100)}%</p></>}
          {item.pipelineStepLabel && <p>{item.pipelineStepLabel}</p>}
          {item.error && <p role="alert">{item.error}</p>}
          {item.outputModelId && <p className="break-all">Output model: {item.outputModelId}</p>}
          {!isConversionTerminal(item.status) && <button className={buttonClass} disabled={workflow.busy || workflow.loading} onClick={() => { void workflow.cancel(item.conversionId); }}>Cancel conversion</button>}
        </section>)}
      </div>
    </ModalDialog>
  );
}
