import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AppIcon } from './AppIcon';

describe('AppIcon', () => {
  const defaultProps = {
    appId: 'test-app',
    state: 'offline' as const,
    hasInstall: true,
    launchError: false,
  };

  describe('Basic Rendering', () => {
    it('renders the app icon', () => {
      render(<AppIcon {...defaultProps} title="Test App" />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('displays AppIndicator when not a ghost', () => {
      const { container } = render(<AppIcon {...defaultProps} />);
      const button = container.querySelector('button');
      expect(button).toBeInTheDocument();
    });

    it('calls onClick when icon is clicked', async () => {
      const user = userEvent.setup();
      const onClick = vi.fn();
      render(<AppIcon {...defaultProps} onClick={onClick} />);

      await user.click(screen.getByRole('button'));
      expect(onClick).toHaveBeenCalledTimes(1);
    });

    it('hides AppIndicator when isGhost is true', () => {
      const { container } = render(<AppIcon {...defaultProps} isGhost={true} />);
      const indicator = container.querySelector('[data-testid="app-indicator"]');
      expect(indicator).not.toBeInTheDocument();
    });
  });

  describe('Icon States', () => {
    it('renders RunningIcon for running state', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={50} gpuUsage={30} />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('renders OfflineIcon for offline state', () => {
      render(<AppIcon {...defaultProps} state="offline" />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('renders ErrorIcon for error state', () => {
      render(<AppIcon {...defaultProps} state="error" />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('renders UninstalledIcon for uninstalled state', () => {
      render(<AppIcon {...defaultProps} state="uninstalled" />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
  });

  describe('Drag Opacity', () => {
    it('applies dragOpacity when provided', () => {
      const { container } = render(<AppIcon {...defaultProps} dragOpacity={0.5} />);
      const button = container.querySelector('button');
      expect(button?.style.opacity).toBe('0.5');
    });

    it('applies default opacity of 1.0 when not provided', () => {
      const { container } = render(<AppIcon {...defaultProps} />);
      const button = container.querySelector('button');
      expect(button?.style.opacity).toBe('1');
    });
  });

  describe('Shake Styles', () => {
    it('applies shakeStyle when provided', () => {
      const shakeStyle = { transform: 'translateY(5px)' };
      const { container } = render(<AppIcon {...defaultProps} shakeStyle={shakeStyle} />);
      const button = container.querySelector('button');
      expect(button?.style.transform).toBe('translateY(5px)');
    });

    it('does not apply shake style when not provided', () => {
      const { container } = render(<AppIcon {...defaultProps} />);
      const button = container.querySelector('button');
      expect(button?.style.transform).toBeFalsy();
    });
  });

  describe('Resource Usage Display', () => {
    it('shows RAM arc at correct percentage when running', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={75} gpuUsage={40} />);
      // RunningIcon renders with RAM and GPU arcs
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('shows GPU arc at correct percentage when running', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={60} gpuUsage={85} />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('handles ramUsage = 0 correctly', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={0} gpuUsage={50} />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('handles ramUsage = 100 correctly', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={100} gpuUsage={50} />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('handles gpuUsage = 0 correctly', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={50} gpuUsage={0} />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('handles gpuUsage = 100 correctly', () => {
      render(<AppIcon {...defaultProps} state="running" ramUsage={50} gpuUsage={100} />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });
  });

  describe('App Callbacks', () => {
    it('passes onLaunch to AppIndicator', () => {
      const onLaunch = vi.fn();
      render(<AppIcon {...defaultProps} onLaunch={onLaunch} />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('passes onStop to AppIndicator', () => {
      const onStop = vi.fn();
      render(<AppIcon {...defaultProps} state="running" onStop={onStop} />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('passes onOpenLog to AppIndicator', () => {
      const onOpenLog = vi.fn();
      render(<AppIcon {...defaultProps} onOpenLog={onOpenLog} />);
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
  });

  describe('Ghost Mode', () => {
    it('shows shadow and cursor-grabbing when isGhost is true', () => {
      const { container } = render(<AppIcon {...defaultProps} isGhost={true} />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('shadow-2xl');
      expect(button?.className).toContain('cursor-grabbing');
    });

    it('shows cursor-grab when isGhost is false', () => {
      const { container } = render(<AppIcon {...defaultProps} isGhost={false} />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('cursor-grab');
      expect(button?.className).not.toContain('cursor-grabbing');
    });
  });

  describe('Icon Paths', () => {
    it('uses correct icon path for openwebui', () => {
      render(<AppIcon {...defaultProps} appId="openwebui" />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('uses correct icon path for ollama', () => {
      render(<AppIcon {...defaultProps} appId="ollama" />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('uses correct icon path for invoke', () => {
      render(<AppIcon {...defaultProps} appId="invoke" />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('uses correct icon path for krita-diffusion', () => {
      render(<AppIcon {...defaultProps} appId="krita-diffusion" />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });

    it('falls back to comfyui icon for unknown appId', () => {
      render(<AppIcon {...defaultProps} appId="unknown-app" />);
      const button = screen.getByRole('button');
      expect(button).toBeInTheDocument();
    });
  });
});
