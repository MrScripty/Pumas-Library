import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LinkHealthStatus } from './LinkHealthStatus';
import type { LinkHealthResponse } from '../types/api';

const healthy: LinkHealthResponse = {
  success: true, status: 'healthy', total_links: 2, healthy_links: 2,
  broken_links: [], orphaned_links: [], warnings: [], errors: [],
};

const {
  getLinkHealthMock,
  isApiAvailableMock,
  cleanBrokenLinksMock,
  removeOrphansMock,
} = vi.hoisted(() => ({
  getLinkHealthMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  cleanBrokenLinksMock: vi.fn(),
  removeOrphansMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_link_health: getLinkHealthMock,
    clean_broken_links: cleanBrokenLinksMock,
    remove_orphaned_links: removeOrphansMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

describe('LinkHealthStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
  });

  it('renders degraded backend status without crashing', async () => {
    getLinkHealthMock.mockResolvedValue({
      success: true,
      status: 'degraded',
      total_links: 3,
      healthy_links: 2,
      broken_links: ['/models/bad.gguf'],
      orphaned_links: [],
      warnings: [],
      errors: [],
    });

    render(<LinkHealthStatus activeVersion="v1.0.0" />);

    await waitFor(() => {
      expect(getLinkHealthMock).toHaveBeenCalledWith('v1.0.0');
    });

    expect(screen.getByText('Link Health')).toBeInTheDocument();
    expect(screen.getByText('Issues detected')).toBeInTheDocument();
    expect(screen.getByText('3 links')).toBeInTheDocument();
  });

  it('shows initial read failure and recovers through an accessible retry', async () => {
    getLinkHealthMock.mockRejectedValueOnce({ code: 'unavailable', message: 'Link health unavailable' });
    getLinkHealthMock.mockResolvedValueOnce({
      success: true,
      status: 'healthy',
      total_links: 0,
      healthy_links: 0,
      broken_links: [],
      orphaned_links: [],
      warnings: [],
      errors: [],
    });

    render(<LinkHealthStatus />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Link health unavailable');
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
    const user = userEvent.setup();
    await user.tab();
    await user.tab();
    expect(screen.getByRole('button', { name: 'Retry link health' })).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(await screen.findByText('All links healthy')).toBeInTheDocument();
  });

  it('does not apply an older version read after returning to that version', async () => {
    let finishOld: ((report: LinkHealthResponse) => void) | undefined;
    getLinkHealthMock.mockReturnValueOnce(new Promise(resolve => { finishOld = resolve; }));
    getLinkHealthMock.mockResolvedValueOnce(healthy);
    getLinkHealthMock.mockRejectedValueOnce({ code: 'unavailable' });
    const { rerender } = render(<LinkHealthStatus activeVersion="v1" />);
    expect(screen.getByRole('status')).toHaveTextContent('Checking link health');
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
    rerender(<LinkHealthStatus activeVersion="v2" />);
    expect(await screen.findByText('All links healthy')).toBeInTheDocument();
    rerender(<LinkHealthStatus activeVersion="v1" />);
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    await act(async () => { finishOld?.(healthy); });
    expect(screen.getByRole('alert')).toHaveTextContent('Link health unavailable');
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
  });

  it.each(['Clean Broken', 'Remove Orphans'])('does not refresh a new scope after old %s completes', async (action) => {
    let finishCleanup: ((result: { success: boolean; cleaned: number; removed: number }) => void) | undefined;
    const cleanup = new Promise(resolve => { finishCleanup = resolve; });
    cleanBrokenLinksMock.mockReturnValue(cleanup);
    removeOrphansMock.mockReturnValue(cleanup);
    getLinkHealthMock.mockResolvedValueOnce({ ...healthy, status: 'degraded', broken_links: ['/broken'], orphaned_links: ['/orphan'] });
    getLinkHealthMock.mockResolvedValue(healthy);
    const { rerender } = render(<LinkHealthStatus activeVersion="v1" />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: /Link Health Issues detected/ }));
    await user.click(screen.getByRole('button', { name: action }));
    rerender(<LinkHealthStatus activeVersion="v2" />);
    expect(await screen.findByText('All links healthy')).toBeInTheDocument();
    await act(async () => { finishCleanup?.({ success: true, cleaned: 1, removed: 1 }); });
    expect(getLinkHealthMock.mock.calls).toEqual([['v1'], ['v2']]);
    await user.click(screen.getByRole('button', { name: /Link Health All links healthy/ }));
    expect(screen.queryByText(/Cleaned|Removed/)).not.toBeInTheDocument();
  });

  it.each(['healthy', 'degraded'] as const)('clears a previous %s report on refresh failure and offers retry without cleanup controls', async (status) => {
    getLinkHealthMock.mockResolvedValueOnce({ ...healthy, status, broken_links: status === 'degraded' ? ['/broken'] : [] });
    getLinkHealthMock.mockRejectedValueOnce({ code: 'unavailable' });
    render(<LinkHealthStatus />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: /Link Health (Issues detected|All links healthy)/ }));
    await user.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Link health unavailable');
    expect(screen.queryByRole('button', { name: 'Clean Broken' })).not.toBeInTheDocument();
    expect(screen.queryByText('Issues detected')).not.toBeInTheDocument();
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
    expect(screen.queryByText('2 links')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Retry link health' })).toBeEnabled();
  });

  it.each(['Clean Broken', 'Remove Orphans'])('refreshes the current report after successful %s', async (action) => {
    getLinkHealthMock.mockResolvedValueOnce({ ...healthy, status: 'degraded', broken_links: ['/broken'], orphaned_links: ['/orphan'] });
    getLinkHealthMock.mockResolvedValueOnce(healthy);
    cleanBrokenLinksMock.mockResolvedValueOnce({ success: true, cleaned: 1 });
    removeOrphansMock.mockResolvedValueOnce({ success: true, removed: 1 });
    render(<LinkHealthStatus activeVersion="v1" />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: /Link Health Issues detected/ }));
    await user.click(screen.getByRole('button', { name: action }));
    expect(await screen.findByText('All links healthy')).toBeInTheDocument();
    expect(screen.getByText(action === 'Clean Broken' ? 'Cleaned 1 broken link' : 'Removed 1 orphaned link')).toBeInTheDocument();
    expect(getLinkHealthMock.mock.calls).toEqual([['v1'], ['v1']]);
    if (action === 'Remove Orphans') expect(removeOrphansMock).toHaveBeenCalledWith('v1');
  });

  it('presents an unavailable bridge and allows manual checks when auto refresh is disabled', async () => {
    isApiAvailableMock.mockReturnValue(false);
    render(<LinkHealthStatus autoRefresh={false} />);
    expect(screen.getByRole('status')).toHaveTextContent('Link health not checked');
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Check link health' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Link health unavailable');
    expect(getLinkHealthMock).not.toHaveBeenCalled();
    isApiAvailableMock.mockReturnValue(true);
    getLinkHealthMock.mockResolvedValueOnce(healthy);
    await user.click(screen.getByRole('button', { name: 'Retry link health' }));
    expect(await screen.findByText('All links healthy')).toBeInTheDocument();
  });

  it('does not apply a superseded same-version read or leave loading after auto refresh ends', async () => {
    let finishOld: ((report: LinkHealthResponse) => void) | undefined;
    getLinkHealthMock.mockReturnValueOnce(new Promise(resolve => { finishOld = resolve; }));
    const { rerender } = render(<LinkHealthStatus activeVersion="v1" />);
    rerender(<LinkHealthStatus activeVersion="v1" autoRefresh={false} />);
    expect(screen.getByRole('status')).toHaveTextContent('Link health not checked');
    getLinkHealthMock.mockRejectedValueOnce({ code: 'unavailable' });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Check link health' }));
    expect(screen.getByRole('alert')).toBeInTheDocument();
    await act(async () => { finishOld?.(healthy); });
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
  });

  it('observes read rejection after unmount without affecting a new owner', async () => {
    let rejectOld: ((reason: unknown) => void) | undefined;
    getLinkHealthMock.mockReturnValueOnce(new Promise((_resolve, reject) => { rejectOld = reject; }));
    const old = render(<LinkHealthStatus activeVersion="v1" />);
    old.unmount();
    getLinkHealthMock.mockResolvedValueOnce(healthy);
    render(<LinkHealthStatus activeVersion="v1" />);
    expect(await screen.findByText('All links healthy')).toBeInTheDocument();
    await act(async () => { rejectOld?.({ code: 'unavailable' }); });
    expect(screen.getByText('All links healthy')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
