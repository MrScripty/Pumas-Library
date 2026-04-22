import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useModelManagerFilters } from './useModelManagerFilters';

describe('useModelManagerFilters', () => {
  it('tracks local search and category filters', () => {
    const { result } = renderHook(() => useModelManagerFilters());

    act(() => {
      result.current.setSearchQuery('llama');
      result.current.selectFilter('llm');
    });

    expect(result.current.isDownloadMode).toBe(false);
    expect(result.current.hasLocalFilters).toBe(true);
    expect(result.current.isCategoryFiltered).toBe(true);
    expect(result.current.selectedFilter).toBe('llm');

    act(() => {
      result.current.clearLocalFilters();
    });

    expect(result.current.searchQuery).toBe('');
    expect(result.current.selectedCategory).toBe('all');
    expect(result.current.hasLocalFilters).toBe(false);
  });

  it('switches to download mode and selects remote kind filters', () => {
    const { result } = renderHook(() => useModelManagerFilters());

    act(() => {
      result.current.toggleFilterMenu();
      result.current.toggleMode();
    });

    act(() => {
      result.current.selectFilter('image-to-text');
    });

    expect(result.current.isDownloadMode).toBe(true);
    expect(result.current.showCategoryMenu).toBe(false);
    expect(result.current.isCategoryFiltered).toBe(true);
    expect(result.current.selectedFilter).toBe('image-to-text');

    act(() => {
      result.current.clearRemoteFilters();
    });

    expect(result.current.searchQuery).toBe('');
    expect(result.current.selectedKind).toBe('all');
    expect(result.current.isCategoryFiltered).toBe(false);
  });

  it('developer search enters download mode and resets remote kind', () => {
    const { result } = renderHook(() => useModelManagerFilters());

    act(() => {
      result.current.toggleMode();
    });

    act(() => {
      result.current.selectFilter('text-generation');
      result.current.toggleFilterMenu();
      result.current.searchDeveloper('openai');
    });

    expect(result.current.isDownloadMode).toBe(true);
    expect(result.current.searchQuery).toBe('openai');
    expect(result.current.selectedKind).toBe('all');
    expect(result.current.showCategoryMenu).toBe(false);
  });
});
