import { useEffect } from 'react';

/**
 * Sets the document background to transparent so the window content
 * floats over the desktop. Used by popover/overlay windows (tray, launcher).
 */
export function useTransparentBackground() {
  useEffect(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
    return () => {
      document.documentElement.style.background = '';
      document.body.style.background = '';
    };
  }, []);
}
