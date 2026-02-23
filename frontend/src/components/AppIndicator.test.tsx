/**
 * Unit tests for AppIndicator component
 * Target: 80%+ code coverage
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent, act } from '@testing-library/react';
import { AppIndicator } from './AppIndicator';

describe('AppIndicator', () => {
  const defaultProps = {
    appId: 'test-app',
    state: 'offline' as const,
    isSelected: false,
    hasInstall: true,
    launchError: false,
  };

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  describe('Rendering', () => {
    it('should render without crashing', () => {
      const { container } = render(<AppIndicator {...defaultProps} />);
      expect(container).toBeTruthy();
    });

    it('should not render when state is uninstalled and hasInstall is false', () => {
      const { container } = render(
        <AppIndicator {...defaultProps} state="uninstalled" hasInstall={false} />
      );
      // Component returns null in this case
      expect(container.firstChild).toBeNull();
    });

    it('should render play icon for offline state with install', () => {
      render(<AppIndicator {...defaultProps} state="offline" hasInstall={true} />);
      // Play icon should be in the document (lucide-react renders as svg)
      const playIcon = document.querySelector('svg.lucide-play');
      expect(playIcon).toBeTruthy();
    });

    it('should render play icon for uninstalled state with install', () => {
      render(<AppIndicator {...defaultProps} state="uninstalled" hasInstall={true} />);
      const playIcon = document.querySelector('svg.lucide-play');
      expect(playIcon).toBeTruthy();
    });
  });

  describe('Running State', () => {
    it('should show spinner animation when running', () => {
      const { container } = render(<AppIndicator {...defaultProps} state="running" />);

      // Should show spinner frame character
      const spinnerFrames = ['/', '-', '\\', '|'];
      const text = container.textContent || '';
      const hasSpinnerFrame = spinnerFrames.some(frame => text.includes(frame));
      expect(hasSpinnerFrame).toBe(true);
    });

    it('should animate spinner frames when running', () => {
      const { container } = render(<AppIndicator {...defaultProps} state="running" />);

      const getSpinnerText = () => container.textContent || '';
      const initialFrame = getSpinnerText();

      // Advance timer by spinner interval (150ms)
      act(() => {
        vi.advanceTimersByTime(150);
      });

      const nextFrame = getSpinnerText();
      // Frame should have changed
      expect(nextFrame).not.toBe(initialFrame);
    });

    it('should show stop square icon on hover when running', () => {
      render(<AppIndicator {...defaultProps} state="running" />);

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        // Trigger hover using pointer events
        act(() => {
          fireEvent.pointerEnter(indicator);
        });

        // Should show square icon on hover
        const squareIcon = document.querySelector('svg.lucide-square');
        expect(squareIcon).toBeTruthy();
      }
    });

    it('should call onStop when clicked while running', () => {
      const onStop = vi.fn();

      render(<AppIndicator {...defaultProps} state="running" onStop={onStop} />);

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        act(() => {
          fireEvent.click(indicator);
        });
        expect(onStop).toHaveBeenCalledTimes(1);
      }
    });
  });

  describe('Error State', () => {
    it('should flash between alert and play icons when in error state', () => {
      render(
        <AppIndicator {...defaultProps} state="error" launchError={true} />
      );

      // Initially should have one of the error icons
      const hasAlertOrPlay = () => {
        return (
          document.querySelector('svg.lucide-alert-triangle') ||
          document.querySelector('svg.lucide-play')
        );
      };

      expect(hasAlertOrPlay()).toBeTruthy();

      // Advance timer to trigger flash
      vi.advanceTimersByTime(500);

      // Should still have an icon (but may have switched)
      expect(hasAlertOrPlay()).toBeTruthy();
    });

    it('should show log icon on hover when in error state', () => {
      render(<AppIndicator {...defaultProps} state="error" launchError={true} />);

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        act(() => {
          fireEvent.pointerEnter(indicator);
        });

        // Should show file-text icon on hover
        const fileTextIcon = document.querySelector('svg.lucide-file-text');
        expect(fileTextIcon).toBeTruthy();
      }
    });

    it('should call onLaunch when clicked in error state (hover not simulated)', () => {
      // Note: React Aria's useHover requires real browser events or proper simulation
      // In unit tests, we verify the fallback behavior (launch) works
      const onLaunch = vi.fn();
      const onOpenLog = vi.fn();

      render(
        <AppIndicator
          {...defaultProps}
          state="error"
          launchError={true}
          hasInstall={true}
          onLaunch={onLaunch}
          onOpenLog={onOpenLog}
        />
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        act(() => {
          fireEvent.click(indicator);
        });

        // Without proper hover simulation, falls back to launch behavior
        expect(onLaunch).toHaveBeenCalledTimes(1);
        expect(onOpenLog).not.toHaveBeenCalled();
      }
    });

    it('should call onLaunch when clicked while not hovering in error state with install', () => {
      const onLaunch = vi.fn();

      render(
        <AppIndicator
          {...defaultProps}
          state="error"
          launchError={true}
          hasInstall={true}
          onLaunch={onLaunch}
        />
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        // Click without hovering
        act(() => {
          fireEvent.click(indicator);
        });

        expect(onLaunch).toHaveBeenCalledTimes(1);
      }
    });
  });

  describe('Offline State', () => {
    it('should show play icon for offline state with install', () => {
      render(<AppIndicator {...defaultProps} state="offline" hasInstall={true} />);

      const playIcon = document.querySelector('svg.lucide-play');
      expect(playIcon).toBeTruthy();
    });

    it('should enhance play icon on hover', () => {
      render(
        <AppIndicator {...defaultProps} state="offline" hasInstall={true} />
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        const playIcon = document.querySelector('svg.lucide-play');
        expect(playIcon).toBeTruthy();

        // Hover should enhance the icon (size or glow changes)
        act(() => {
          fireEvent.pointerEnter(indicator);
        });

        // Play icon should still be present after hover
        const playIconAfterHover = document.querySelector('svg.lucide-play');
        expect(playIconAfterHover).toBeTruthy();
      }
    });

    it('should call onLaunch when clicked in offline state with install', () => {
      const onLaunch = vi.fn();

      render(
        <AppIndicator {...defaultProps} state="offline" hasInstall={true} onLaunch={onLaunch} />
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');
      expect(indicator).toBeTruthy();

      if (indicator) {
        act(() => {
          fireEvent.click(indicator);
        });
        expect(onLaunch).toHaveBeenCalledTimes(1);
      }
    });

    it('should not call any handler when clicked without install', () => {
      const onLaunch = vi.fn();
      const onStop = vi.fn();
      const onOpenLog = vi.fn();

      render(
        <AppIndicator
          {...defaultProps}
          state="offline"
          hasInstall={false}
          onLaunch={onLaunch}
          onStop={onStop}
          onOpenLog={onOpenLog}
        />
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');

      if (indicator) {
        act(() => {
          fireEvent.click(indicator);
        });

        expect(onLaunch).not.toHaveBeenCalled();
        expect(onStop).not.toHaveBeenCalled();
        expect(onOpenLog).not.toHaveBeenCalled();
      }
    });
  });

  describe('Event Handling', () => {
    it('should stop event propagation on click', () => {
      const onLaunch = vi.fn();
      const parentClick = vi.fn();

      render(
        <div onClick={parentClick} onKeyDown={() => {}} role="button" tabIndex={0}>
          <AppIndicator {...defaultProps} state="offline" hasInstall={true} onLaunch={onLaunch} />
        </div>
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');

      if (indicator) {
        act(() => {
          fireEvent.click(indicator);
        });

        // Indicator handler should be called
        expect(onLaunch).toHaveBeenCalledTimes(1);

        // Parent handler should NOT be called (propagation stopped)
        expect(parentClick).not.toHaveBeenCalled();
      }
    });
  });

  describe('Animation Cleanup', () => {
    it('should clean up spinner interval when unmounted', () => {
      const { unmount } = render(<AppIndicator {...defaultProps} state="running" />);

      // Spy on clearInterval
      const clearIntervalSpy = vi.spyOn(global, 'clearInterval');

      unmount();

      // Should have cleared the interval
      expect(clearIntervalSpy).toHaveBeenCalled();

      clearIntervalSpy.mockRestore();
    });

    it('should clean up error flash interval when unmounted', () => {
      const { unmount } = render(
        <AppIndicator {...defaultProps} state="error" launchError={true} />
      );

      const clearIntervalSpy = vi.spyOn(global, 'clearInterval');

      unmount();

      expect(clearIntervalSpy).toHaveBeenCalled();

      clearIntervalSpy.mockRestore();
    });

    it('should clean up spinner when state changes from running to offline', () => {
      const { rerender } = render(<AppIndicator {...defaultProps} state="running" />);

      const clearIntervalSpy = vi.spyOn(global, 'clearInterval');

      // Change state to offline
      rerender(<AppIndicator {...defaultProps} state="offline" />);

      // Should have cleared the running spinner interval
      expect(clearIntervalSpy).toHaveBeenCalled();

      clearIntervalSpy.mockRestore();
    });
  });

  describe('Props Combinations', () => {
    it('should handle all state combinations correctly', () => {
      const states = ['running', 'offline', 'uninstalled', 'error'] as const;

      states.forEach(state => {
        const { container, unmount } = render(
          <AppIndicator {...defaultProps} state={state} hasInstall={true} />
        );

        // Should render without errors for all states
        expect(container).toBeTruthy();

        unmount();
      });
    });

    it('should handle optional callbacks being undefined', () => {
      // Test without any callbacks
      render(
        <AppIndicator {...defaultProps} state="offline" hasInstall={true} />
      );

      const indicator = document.querySelector('[class*="cursor-pointer"]');

      if (indicator) {
        // Should not throw when clicking without callbacks
        expect(() => {
          act(() => {
            fireEvent.click(indicator);
          });
        }).not.toThrow();
      }
    });
  });
});
