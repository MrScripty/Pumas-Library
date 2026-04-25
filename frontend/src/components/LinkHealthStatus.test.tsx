import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LinkHealthStatus } from './LinkHealthStatus';

const {
  getLinkHealthMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  getLinkHealthMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_link_health: getLinkHealthMock,
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

  it('falls back to an unknown status presentation for unexpected backend values', async () => {
    getLinkHealthMock.mockResolvedValue({
      success: true,
      status: 'errors',
      total_links: 0,
      healthy_links: 0,
      broken_links: [],
      orphaned_links: [],
      warnings: [],
      errors: [],
    });

    render(<LinkHealthStatus />);

    expect(await screen.findByText('Unknown status')).toBeInTheDocument();
  });
});
