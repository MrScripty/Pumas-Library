import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { InstallationProgress } from './useVersions';
import { useInstallationState } from './useInstallationState';

const progress: InstallationProgress = {
  tag: 'v1.2.3',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'dependencies',
  stage_progress: 45,
  overall_progress: 60,
  current_item: 'torch',
  download_speed: null,
  eta_seconds: null,
  total_size: null,
  downloaded_bytes: 0,
  dependency_count: 4,
  completed_dependencies: 2,
  completed_items: [],
  error: null,
};

describe('useInstallationState', () => {
  it('shows a new active installation once and preserves Back for later progress updates', () => {
    interface StateProps {
      installingVersion: string | null;
      progress: InstallationProgress | null;
    }

    const { result, rerender } = renderHook(
      (props: StateProps) => useInstallationState({
        isOpen: true,
        installingVersion: props.installingVersion,
        progress: props.progress,
      }),
      {
        initialProps: {
          installingVersion: null,
          progress: null,
        } satisfies StateProps,
      }
    );
    const rerenderState = rerender as (props: StateProps) => void;

    rerenderState({ installingVersion: 'v1.2.3', progress });
    expect(result.current.viewMode).toBe('details');

    act(() => {
      result.current.setViewMode('list');
    });
    rerenderState({
      installingVersion: 'v1.2.3',
      progress: { ...progress, overall_progress: 75 },
    });

    expect(result.current.viewMode).toBe('list');
  });

  it('shows the terminal transition once after Back and preserves a later list choice', () => {
    const { result, rerender } = renderHook(
      (nextProgress: InstallationProgress) => useInstallationState({
        isOpen: true,
        installingVersion: 'v1.2.3',
        progress: nextProgress,
      }),
      { initialProps: progress }
    );

    act(() => {
      result.current.setViewMode('list');
    });
    rerender({
      ...progress,
      completed_at: '2026-04-12T00:05:00Z',
      success: true,
    });

    expect(result.current.viewMode).toBe('details');

    act(() => {
      result.current.setViewMode('list');
    });
    rerender({
      ...progress,
      completed_at: '2026-04-12T00:05:00Z',
      success: true,
    });

    expect(result.current.viewMode).toBe('list');
  });

  it('opens on the version list when no installation progress exists', () => {
    const { result, rerender } = renderHook(
      (props: { isOpen: boolean }) => useInstallationState({
        isOpen: props.isOpen,
        installingVersion: null,
        progress: null,
      }),
      {
        initialProps: { isOpen: false },
      }
    );

    expect(result.current.viewMode).toBe('list');

    rerender({ isOpen: true });

    expect(result.current.viewMode).toBe('list');
  });

  it('falls back to list view when details lose their backing progress state', () => {
    interface StateProps {
      installingVersion: string | null;
      progress: InstallationProgress | null;
    }

    const { result, rerender } = renderHook(
      (props: StateProps) =>
        useInstallationState({
          isOpen: true,
          installingVersion: props.installingVersion,
          progress: props.progress,
        }),
      {
        initialProps: {
          installingVersion: 'v1.2.3',
          progress,
        } satisfies StateProps,
      }
    );
    const rerenderState = rerender as (props: StateProps) => void;

    act(() => {
      result.current.setViewMode('details');
      result.current.setHoveredTag('v1.2.3');
      result.current.setCancelHoverTag('v1.2.3');
      result.current.setShowCompletedItems(true);
    });

    expect(result.current.viewMode).toBe('details');
    expect(result.current.hoveredTag).toBe('v1.2.3');
    expect(result.current.cancelHoverTag).toBe('v1.2.3');
    expect(result.current.showCompletedItems).toBe(true);

    const nextProps: StateProps = {
      installingVersion: null,
      progress: null,
    };

    rerenderState(nextProps);

    expect(result.current.viewMode).toBe('list');
    expect(result.current.hoveredTag).toBe('v1.2.3');
    expect(result.current.cancelHoverTag).toBe('v1.2.3');
    expect(result.current.showCompletedItems).toBe(true);
  });
});
