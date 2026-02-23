import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AppSidebar } from './AppSidebar';
import type { AppConfig } from '../types/apps';
import { LIST_TOP_PADDING, TOTAL_HEIGHT } from '../hooks/usePhysicsDrag';
import { Box } from 'lucide-react';

const mockApps: AppConfig[] = [
  {
    id: 'comfyui',
    name: 'comfyui',
    displayName: 'ComfyUI',
    icon: Box,
    status: 'running',
    iconState: 'running',
    ramUsage: 60,
    gpuUsage: 40,
  },
  {
    id: 'openwebui',
    name: 'openwebui',
    displayName: 'OpenWebUI',
    icon: Box,
    status: 'idle',
    iconState: 'offline',
    ramUsage: 0,
    gpuUsage: 0,
  },
  {
    id: 'invoke',
    name: 'invoke',
    displayName: 'Invoke',
    icon: Box,
    status: 'idle',
    iconState: 'uninstalled',
    ramUsage: 0,
    gpuUsage: 0,
  },
];

const defaultProps = {
  apps: mockApps,
  selectedAppId: null,
  onSelectApp: vi.fn(),
};

describe('AppSidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders all app icons', () => {
    render(<AppSidebar {...defaultProps} />);
    const appButtons = mockApps.map(app => screen.getByTitle(app.displayName));
    expect(appButtons.length).toBe(mockApps.length);
  });

  it('calls onSelectApp when icon is clicked', async () => {
    const user = userEvent.setup();
    const onSelectApp = vi.fn();
    render(<AppSidebar {...defaultProps} onSelectApp={onSelectApp} />);

    const firstApp = screen.getByTitle(mockApps[0]!.displayName);
    await user.click(firstApp);

    expect(onSelectApp).toHaveBeenCalled();
  });

  it('deselects when clicking the sidebar background', async () => {
    const user = userEvent.setup();
    const onSelectApp = vi.fn();
    const { container } = render(<AppSidebar {...defaultProps} onSelectApp={onSelectApp} />);

    const sidebar = container.firstChild;
    if (sidebar) {
      await user.click(sidebar as HTMLElement);
    }

    expect(onSelectApp).toHaveBeenCalledWith(null);
  });

  it('calls onAddApp with nearest index when Plus is clicked', async () => {
    const onAddApp = vi.fn();
    const { container } = render(<AppSidebar {...defaultProps} onAddApp={onAddApp} />);

    act(() => {
      window.dispatchEvent(new MouseEvent('mousemove', {
        clientX: 10,
        clientY: LIST_TOP_PADDING + TOTAL_HEIGHT,
      }));
    });

    await waitFor(() => {
      const plusIcon = container.querySelector('svg.lucide-plus');
      expect(plusIcon).toBeInTheDocument();
    });

    // The plus icon is inside a clickable div with role="button"
    const plusButton = container.querySelector('svg.lucide-plus')?.closest('[role="button"]') as HTMLElement;
    fireEvent.click(plusButton);

    expect(onAddApp).toHaveBeenCalledWith(1);
  });
});
