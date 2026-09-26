import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { StatusFooter } from './StatusFooter';

describe('StatusFooter', () => {
  it('shows Torch setup download speed while retaining stage and progress, with a fallback when speed clears', () => {
    const progress = {
      tag: 'v2.14.0',
      started_at: '2026-04-12T00:00:00Z',
      stage: 'setup' as const,
      stage_progress: 40,
      overall_progress: 95,
      current_item: 'Installing Torch wheel',
      download_speed: 12 * 1024 * 1024,
      eta_seconds: null,
      total_size: null,
      downloaded_bytes: 0,
      dependency_count: null,
      completed_dependencies: 0,
      completed_items: [],
      error: null,
    };

    const { rerender } = render(<StatusFooter installationProgress={progress} />);
    expect(screen.getByText('Finalizing setup · 12.0 MB/s · 95% complete')).toBeInTheDocument();

    rerender(<StatusFooter installationProgress={{ ...progress, download_speed: null }} />);
    expect(screen.getByText('Finalizing setup · 95% complete')).toBeInTheDocument();
  });
});
