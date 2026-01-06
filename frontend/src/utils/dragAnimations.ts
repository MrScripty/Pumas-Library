import React from 'react';

/**
 * Physics-based drag animation utilities
 *
 * Provides three types of shake animations:
 * 1. Settling shake - Like a bouncing coin settling into place
 * 2. Resistance shake - Vibration when pulling against magnetic force
 * 3. Delete zone shake - Warning shake when approaching delete
 */

/**
 * Generate settling shake animation (like a bouncing coin)
 * Bounce height decreases quadratically, frequency increases
 *
 * @param intensity - 0 to 1, where 1 is peak bounce
 * @param timestamp - Current time in milliseconds (from performance.now())
 */
export function getSettlingShakeStyle(
  intensity: number,
  timestamp: number
): React.CSSProperties {
  if (intensity === 0) return {};

  // Frequency increases as it settles (faster bounces)
  const baseFrequency = 8; // Hz
  const maxFrequency = 24; // Hz
  const frequency = baseFrequency + (intensity * (maxFrequency - baseFrequency));

  // Amplitude decreases with a quadratic curve for natural settling
  const maxBounce = 3; // pixels
  const bounceHeight = maxBounce * intensity * (1 - intensity * intensity);

  // Calculate current bounce position
  const phase = (timestamp / 1000) * frequency * Math.PI * 2;
  const yOffset = Math.sin(phase) * bounceHeight;

  // Rotation wobble - decreases with intensity
  const maxRotation = 5; // degrees
  const rotation = Math.sin(phase * 1.3) * maxRotation * intensity * (1 - intensity * 0.5);

  return {
    transform: `translateY(${yOffset}px) rotate(${rotation}deg)`,
    transformOrigin: 'bottom center'
  };
}

/**
 * Generate resistance shake (pulling against magnetic force)
 * Small, rapid vibration
 *
 * @param intensity - 0 to 1, where 1 is maximum resistance
 * @param timestamp - Current time in milliseconds
 */
export function getResistanceShakeStyle(
  intensity: number,
  timestamp: number
): React.CSSProperties {
  if (intensity === 0) return {};

  // High frequency, small amplitude vibration
  const frequency = 30; // Hz (rapid shake)
  const maxOffset = 2; // pixels

  const phase = (timestamp / 1000) * frequency * Math.PI * 2;
  const xOffset = Math.sin(phase) * maxOffset * intensity;
  const yOffset = Math.cos(phase * 1.7) * maxOffset * intensity;

  return {
    transform: `translate(${xOffset}px, ${yOffset}px)`,
    transformOrigin: 'center'
  };
}

/**
 * Generate delete zone shake (edit-mode wiggle)
 * Bouncy, iOS-style wiggle that scales smoothly with intensity
 *
 * @param intensity - 0 to 1, where 1 is frantic shake
 * @param timestamp - Current time in milliseconds
 */
export function getDeleteZoneShakeStyle(
  intensity: number,
  timestamp: number
): React.CSSProperties {
  if (intensity === 0) return {};

  // Slower, bouncy wiggle like iOS edit mode
  const minFrequency = 5; // Hz
  const maxFrequency = 8; // Hz
  const frequency = minFrequency + (intensity * (maxFrequency - minFrequency));

  const translateAmplitude = 1.2 + (intensity * 2.4);
  const rotationAmplitude = 2 + (intensity * 4.5);

  const phase = (timestamp / 1000) * frequency * Math.PI * 2;
  const xOffset = Math.sin(phase) * translateAmplitude;
  const yOffset = Math.cos(phase * 1.3 + 0.5) * translateAmplitude * 0.55;
  const bouncePhase = (timestamp / 1000) * (frequency * 0.6) * Math.PI * 2;
  const bounceOffset = Math.sin(bouncePhase) * translateAmplitude * 0.35;
  const rotation = Math.sin(phase * 1.1 + 0.8) * rotationAmplitude;

  return {
    transform: `translate(${xOffset}px, ${yOffset + bounceOffset}px) rotate(${rotation}deg)`,
    transformOrigin: 'center'
  };
}

/**
 * Hook to get current animation frame timestamp
 * Updates at 60fps using requestAnimationFrame
 */
export function useAnimationTimestamp(): number {
  const [timestamp, setTimestamp] = React.useState(performance.now());

  React.useEffect(() => {
    let rafId: number;

    const update = () => {
      setTimestamp(performance.now());
      rafId = requestAnimationFrame(update);
    };

    rafId = requestAnimationFrame(update);
    return () => cancelAnimationFrame(rafId);
  }, []);

  return timestamp;
}
