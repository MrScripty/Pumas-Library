import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useHfAuthPrompt } from './useHfAuthPrompt';

function renderPrompt(downloadErrors: Record<string, string>) {
  const isAuthRequiredError = vi.fn((message: string) => message.includes('401'));
  return renderHook(
    ({ errors }) => useHfAuthPrompt({
      downloadErrors: errors,
      isAuthRequiredError,
    }),
    { initialProps: { errors: downloadErrors } }
  );
}

describe('useHfAuthPrompt', () => {
  it('opens when a new download error requires authentication', () => {
    const { result, rerender } = renderPrompt({});

    rerender({ errors: { 'org/private-model': 'HTTP 401 Unauthorized' } });

    expect(result.current.isHfAuthOpen).toBe(true);
  });

  it('does not open for existing auth errors on rerender', () => {
    const { result, rerender } = renderPrompt({
      'org/private-model': 'HTTP 401 Unauthorized',
    });

    expect(result.current.isHfAuthOpen).toBe(true);

    act(() => {
      result.current.closeHfAuth();
    });
    rerender({ errors: { 'org/private-model': 'HTTP 401 Unauthorized' } });

    expect(result.current.isHfAuthOpen).toBe(false);
  });

  it('ignores non-auth download errors', () => {
    const { result, rerender } = renderPrompt({});

    rerender({ errors: { 'org/public-model': 'HTTP 500 Internal Server Error' } });

    expect(result.current.isHfAuthOpen).toBe(false);
  });

  it('supports explicit open and close actions', () => {
    const { result } = renderPrompt({});

    act(() => {
      result.current.openHfAuth();
    });
    expect(result.current.isHfAuthOpen).toBe(true);

    act(() => {
      result.current.closeHfAuth();
    });
    expect(result.current.isHfAuthOpen).toBe(false);
  });
});
