import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ExternalLink } from './ExternalLink';
import { openExternalUrl } from './openExternalUrl';

const { isApiAvailableMock, openUrlMock } = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
  openUrlMock: vi.fn<(_url: string) => Promise<{ success: boolean; error?: string }>>(),
}));

vi.mock('../../api/adapter', () => ({
  api: { open_url: openUrlMock },
  isAPIAvailable: isApiAvailableMock,
}));

describe('ExternalLink', () => {
  const openWindow = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    openUrlMock.mockResolvedValue({ success: true });
    vi.stubGlobal('open', openWindow);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('opens http(s) links through the desktop bridge without navigating or opening a window', () => {
    render(<ExternalLink href="https://huggingface.co/org/model">model card</ExternalLink>);
    const link = screen.getByRole('link', { name: 'model card' });
    expect(link).not.toHaveAttribute('target');
    expect(link).toHaveAttribute('href', 'https://huggingface.co/org/model');

    const click = new MouseEvent('click', { bubbles: true, cancelable: true });
    fireEvent(link, click);

    expect(click.defaultPrevented).toBe(true);
    expect(openUrlMock).toHaveBeenCalledWith('https://huggingface.co/org/model');
    expect(openWindow).not.toHaveBeenCalled();
  });

  it('renders untrusted non-http URLs as plain text', () => {
    for (const href of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,x', 'not a url']) {
      const { unmount } = render(<ExternalLink href={href}>untrusted</ExternalLink>);
      expect(screen.queryByRole('link')).toBeNull();
      fireEvent.click(screen.getByText('untrusted'));
      unmount();
    }
    expect(openUrlMock).not.toHaveBeenCalled();
  });

  it('keeps the click from reaching an enclosing row when asked', () => {
    const rowClick = vi.fn();
    render(
      // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- test fixture row
      <div onClick={rowClick}>
        <ExternalLink href="https://example.test/" stopPropagation>repo</ExternalLink>
        <ExternalLink href="https://example.test/other">other</ExternalLink>
      </div>
    );
    fireEvent.click(screen.getByRole('link', { name: 'repo' }));
    expect(rowClick).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('link', { name: 'other' }));
    expect(rowClick).toHaveBeenCalledTimes(1);
  });
});

describe('openExternalUrl', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
  });

  it('reports whether the system browser accepted the URL', async () => {
    openUrlMock.mockResolvedValue({ success: true });
    await expect(openExternalUrl('https://example.test/')).resolves.toBe(true);

    openUrlMock.mockResolvedValue({ success: false, error: 'refused' });
    await expect(openExternalUrl('https://example.test/')).resolves.toBe(false);

    openUrlMock.mockRejectedValue(new Error('Only http/https URLs are allowed'));
    await expect(openExternalUrl('https://example.test/')).resolves.toBe(false);

    isApiAvailableMock.mockReturnValue(false);
    openUrlMock.mockClear();
    await expect(openExternalUrl('https://example.test/')).resolves.toBe(false);
    await expect(openExternalUrl('ftp://example.test/')).resolves.toBe(false);
    expect(openUrlMock).not.toHaveBeenCalled();
  });
});
