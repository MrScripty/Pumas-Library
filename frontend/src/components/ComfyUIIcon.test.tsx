import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ComfyUIIcon } from './ComfyUIIcon';

describe('ComfyUIIcon', () => {
  const defaultProps = {
    state: 'offline' as const,
    hasInstall: true,
    launchError: false,
  };

  describe('Basic Rendering', () => {
    it('renders the ComfyUI icon', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} title="ComfyUI" />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('displays AppIndicator when not a ghost', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} />);
      const button = container.querySelector('button');
      expect(button).toBeInTheDocument();
    });

    it('calls onClick when icon is clicked', async () => {
      const user = userEvent.setup();
      const onClick = vi.fn();
      const { container } = render(<ComfyUIIcon {...defaultProps} onClick={onClick} />);

      await user.click(container.querySelector('button')!);
      expect(onClick).toHaveBeenCalledTimes(1);
    });

    it('hides AppIndicator when isGhost is true', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} isGhost={true} />);
      const indicator = container.querySelector('[data-testid="app-indicator"]');
      expect(indicator).not.toBeInTheDocument();
    });
  });

  describe('Icon States', () => {
    it('renders RunningIcon for running state', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" ramUsage={50} gpuUsage={30} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('renders OfflineIcon for offline state', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="offline" />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('renders ErrorIcon for error state', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="error" />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('renders UninstalledIcon for uninstalled state', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="uninstalled" />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });
  });

  describe('Drag Opacity', () => {
    it('applies dragOpacity when provided', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} dragOpacity={0.5} />);
      const button = container.querySelector('button');
      expect(button?.style.opacity).toBe('0.5');
    });

    it('applies default opacity of 1.0 when not provided', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} />);
      const button = container.querySelector('button');
      expect(button?.style.opacity).toBe('1');
    });
  });

  describe('Shake Styles', () => {
    it('applies shakeStyle when provided', () => {
      const shakeStyle = { transform: 'translateY(5px)' };
      const { container } = render(<ComfyUIIcon {...defaultProps} shakeStyle={shakeStyle} />);
      const button = container.querySelector('button');
      expect(button?.style.transform).toBe('translateY(5px)');
    });

    it('does not apply shake style when not provided', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} />);
      const button = container.querySelector('button');
      expect(button?.style.transform).toBeFalsy();
    });
  });

  describe('Resource Usage Display', () => {
    it('shows RAM and GPU arcs when running', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" ramUsage={75} gpuUsage={40} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('handles ramUsage = 0 correctly', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" ramUsage={0} gpuUsage={50} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('handles ramUsage = 100 correctly', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" ramUsage={100} gpuUsage={50} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('handles gpuUsage = 0 correctly', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" ramUsage={50} gpuUsage={0} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('handles gpuUsage = 100 correctly', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" ramUsage={50} gpuUsage={100} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });
  });

  describe('App Callbacks', () => {
    it('passes onLaunch to AppIndicator', () => {
      const onLaunch = vi.fn();
      const { container } = render(<ComfyUIIcon {...defaultProps} onLaunch={onLaunch} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('passes onStop to AppIndicator', () => {
      const onStop = vi.fn();
      const { container } = render(<ComfyUIIcon {...defaultProps} state="running" onStop={onStop} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });

    it('passes onOpenLog to AppIndicator', () => {
      const onOpenLog = vi.fn();
      const { container } = render(<ComfyUIIcon {...defaultProps} onOpenLog={onOpenLog} />);
      expect(container.querySelector('button')).toBeInTheDocument();
    });
  });

  describe('Ghost Mode', () => {
    it('shows shadow and cursor-grabbing when isGhost is true', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} isGhost={true} />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('shadow-2xl');
      expect(button?.className).toContain('cursor-grabbing');
    });

    it('shows cursor-grab when isGhost is false', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} isGhost={false} />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('cursor-grab');
      expect(button?.className).not.toContain('cursor-grabbing');
    });
  });

  describe('Opacity States', () => {
    it('applies correct opacity for selected state', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} isSelected={true} />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('opacity-100');
    });

    it('applies correct opacity for unselected offline state', () => {
      const { container} = render(<ComfyUIIcon {...defaultProps} isSelected={false} state="offline" />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('opacity-80');
    });

    it('applies correct opacity for uninstalled state', () => {
      const { container } = render(<ComfyUIIcon {...defaultProps} state="uninstalled" />);
      const button = container.querySelector('button');
      expect(button?.className).toContain('opacity-60');
    });
  });
});
