import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { OllamaModelInfo } from '../../../types/api';
import { OllamaRegisteredModels } from './OllamaRegisteredModels';
import { formatOllamaModelSize } from './ollamaModelFormatting';

const model: OllamaModelInfo = {
  name: 'llama3:latest',
  size: 4_000_000_000,
  digest: 'digest',
  modified_at: '2026-01-01T00:00:00Z',
};

describe('OllamaRegisteredModels', () => {
  it('renders loaded model state and memory details', () => {
    render(
      <OllamaRegisteredModels
        deletingModel={null}
        isRefreshing={false}
        models={[model]}
        runningSet={new Set([model.name])}
        togglingModel={null}
        vramMap={new Map([[model.name, 2_000_000_000]])}
        onDelete={vi.fn()}
        onToggleLoad={vi.fn()}
      />
    );

    expect(screen.getByText('Ollama Models')).toBeInTheDocument();
    expect(screen.getByText('llama3:latest')).toBeInTheDocument();
    expect(screen.getByText('LOADED')).toBeInTheDocument();
    expect(screen.getByText(/4\.0 GB/)).toBeInTheDocument();
    expect(screen.getByText(/2\.0 GB VRAM/)).toBeInTheDocument();
  });

  it('routes load and delete actions through named buttons', async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    const onToggleLoad = vi.fn();

    render(
      <OllamaRegisteredModels
        deletingModel={null}
        isRefreshing={false}
        models={[model]}
        runningSet={new Set()}
        togglingModel={null}
        vramMap={new Map()}
        onDelete={onDelete}
        onToggleLoad={onToggleLoad}
      />
    );

    await user.click(screen.getByRole('button', { name: 'Load llama3:latest' }));
    await user.click(screen.getByRole('button', { name: 'Remove llama3:latest from Ollama' }));

    expect(onToggleLoad).toHaveBeenCalledWith('llama3:latest', false);
    expect(onDelete).toHaveBeenCalledWith('llama3:latest');
  });

  it('disables actions while a model is toggling', () => {
    render(
      <OllamaRegisteredModels
        deletingModel={null}
        isRefreshing={false}
        models={[model]}
        runningSet={new Set()}
        togglingModel={model.name}
        vramMap={new Map()}
        onDelete={vi.fn()}
        onToggleLoad={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: 'Load llama3:latest' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Remove llama3:latest from Ollama' })).toBeDisabled();
  });
});

describe('formatOllamaModelSize', () => {
  it('formats bytes with the expected display unit', () => {
    expect(formatOllamaModelSize(500)).toBe('500 B');
    expect(formatOllamaModelSize(1_500)).toBe('1.5 KB');
    expect(formatOllamaModelSize(2_000_000)).toBe('2.0 MB');
    expect(formatOllamaModelSize(3_000_000_000)).toBe('3.0 GB');
  });
});
