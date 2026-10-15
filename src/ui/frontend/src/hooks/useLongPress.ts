import { useRef, useCallback } from 'react';

interface UseLongPressOptions {
  threshold?: number; // ms to hold before triggering
  onLongPress: (coords: { x: number; y: number }, e: React.TouchEvent | React.MouseEvent) => void;
  onClick?: (e: React.MouseEvent) => void;
  disabled?: boolean;
}

export function useLongPress({
  threshold = 450,
  onLongPress,
  onClick,
  disabled = false,
}: UseLongPressOptions) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const startCoordsRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const isLongPressRef = useRef(false);

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      if (disabled) return;
      if (e.touches.length !== 1) return;

      const touch = e.touches[0];
      startCoordsRef.current = { x: touch.clientX, y: touch.clientY };
      isLongPressRef.current = false;

      clearTimer();
      timerRef.current = setTimeout(() => {
        isLongPressRef.current = true;
        try {
          if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
            navigator.vibrate?.(40);
          }
        } catch {}
        onLongPress({ x: startCoordsRef.current.x, y: startCoordsRef.current.y }, e);
      }, threshold);
    },
    [disabled, threshold, onLongPress, clearTimer]
  );

  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      if (!timerRef.current) return;
      if (e.touches.length !== 1) {
        clearTimer();
        return;
      }
      const touch = e.touches[0];
      const dist = Math.hypot(
        touch.clientX - startCoordsRef.current.x,
        touch.clientY - startCoordsRef.current.y
      );
      // Cancel if user moved more than 8 pixels (scrolling or panning)
      if (dist > 8) {
        clearTimer();
      }
    },
    [clearTimer]
  );

  const handleTouchEnd = useCallback(
    (e: React.TouchEvent) => {
      clearTimer();
      if (isLongPressRef.current) {
        // Prevent click/ghost click following a successful long press
        if (e.cancelable) {
          e.preventDefault();
        }
      }
    },
    [clearTimer]
  );

  const handleTouchCancel = useCallback(() => {
    clearTimer();
  }, [clearTimer]);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if (isLongPressRef.current) {
        e.preventDefault();
        e.stopPropagation();
        isLongPressRef.current = false;
        return;
      }
      onClick?.(e);
    },
    [onClick]
  );

  return {
    onTouchStart: handleTouchStart,
    onTouchMove: handleTouchMove,
    onTouchEnd: handleTouchEnd,
    onTouchCancel: handleTouchCancel,
    onClick: handleClick,
  };
}
