import { useState, useEffect, useRef } from 'react';

/**
 * Hook to smoothly interpolate resource values between updates
 *
 * @param targetValue - The target value to interpolate towards
 * @param updateInterval - How often the target value changes (ms)
 * @returns The smoothly interpolated current value
 */
export function useSmoothResource(
  targetValue: number | undefined,
  updateInterval: number = 500
): number | undefined {
  const [currentValue, setCurrentValue] = useState(targetValue);
  const previousTargetRef = useRef(targetValue);
  const startValueRef = useRef(targetValue);
  const startTimeRef = useRef(Date.now());

  useEffect(() => {
    // If target value changed, start new interpolation
    if (targetValue !== previousTargetRef.current) {
      startValueRef.current = currentValue;
      previousTargetRef.current = targetValue;
      startTimeRef.current = Date.now();
    }

    if (targetValue === undefined) {
      setCurrentValue(undefined);
      return;
    }

    // Animate from start value to target value
    const animate = () => {
      const elapsed = Date.now() - startTimeRef.current;
      const progress = Math.min(elapsed / updateInterval, 1);

      // Ease-in-out interpolation
      const easeProgress = progress < 0.5
        ? 2 * progress * progress
        : 1 - Math.pow(-2 * progress + 2, 2) / 2;

      const start = startValueRef.current ?? targetValue;
      const interpolated = start + (targetValue - start) * easeProgress;

      setCurrentValue(interpolated);

      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };

    const animationFrame = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationFrame);
  }, [targetValue, updateInterval, currentValue]);

  return currentValue;
}
