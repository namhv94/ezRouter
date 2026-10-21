import React, { useRef } from 'react';
import { useI18n } from '../i18n';

export type ThemeMode = 'legacy' | 'operational';

export const THEME_STORAGE_KEY = 'ezrouter.ui-theme';
export const DEFAULT_THEME: ThemeMode = 'legacy';

export function getStoredTheme(): ThemeMode {
  if (typeof window === 'undefined') return DEFAULT_THEME;
  try {
    const val = window.localStorage?.getItem(THEME_STORAGE_KEY);
    if (val === 'operational' || val === 'legacy') {
      return val;
    }
  } catch {
    // Ignore localStorage access errors
  }
  return DEFAULT_THEME;
}

export function applyTheme(theme: string | ThemeMode): ThemeMode {
  const safeTheme: ThemeMode = theme === 'operational' ? 'operational' : 'legacy';
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', safeTheme);
  }
  try {
    window.localStorage?.setItem(THEME_STORAGE_KEY, safeTheme);
  } catch {
    // Ignore localStorage access errors
  }
  return safeTheme;
}

interface ThemeSwitcherProps {
  theme?: ThemeMode;
  onThemeChange?: (theme: ThemeMode) => void;
  className?: string;
}

export const ThemeSwitcher: React.FC<ThemeSwitcherProps> = ({
  theme: controlledTheme,
  onThemeChange,
  className = '',
}) => {
  const { t } = useI18n();
  const [internalTheme, setInternalTheme] = React.useState<ThemeMode>(getStoredTheme);
  const theme = controlledTheme ?? internalTheme;

  const legacyRef = useRef<HTMLButtonElement>(null);
  const operationalRef = useRef<HTMLButtonElement>(null);

  const handleSelect = (nextTheme: ThemeMode) => {
    if (nextTheme === theme) return;
    if (onThemeChange) {
      onThemeChange(nextTheme);
    } else {
      setInternalTheme(nextTheme);
      applyTheme(nextTheme);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      handleSelect('operational');
      operationalRef.current?.focus();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      handleSelect('legacy');
      legacyRef.current?.focus();
    }
  };

  const activeLabel = theme === 'operational' ? t('theme.operational') : t('theme.legacy');

  return (
    <div
      className={`theme-switcher ${className}`.trim()}
      role="group"
      aria-label={t('theme.selectTheme')}
      title={t('theme.currentTheme', { theme: activeLabel })}
    >
      <button
        ref={legacyRef}
        type="button"
        className={`theme-switcher-btn ${theme === 'legacy' ? 'active' : ''}`}
        onClick={() => handleSelect('legacy')}
        onKeyDown={handleKeyDown}
        aria-pressed={theme === 'legacy'}
        aria-label={t('theme.legacy')}
        title={t('theme.legacy')}
      >
        <svg
          className="theme-icon"
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
        <span className="theme-switcher-label">{t('theme.legacyShort')}</span>
      </button>
      <button
        ref={operationalRef}
        type="button"
        className={`theme-switcher-btn ${theme === 'operational' ? 'active' : ''}`}
        onClick={() => handleSelect('operational')}
        onKeyDown={handleKeyDown}
        aria-pressed={theme === 'operational'}
        aria-label={t('theme.operational')}
        title={t('theme.operational')}
      >
        <svg
          className="theme-icon"
          width="15"
          height="15"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <rect x="3" y="3" width="7" height="9" />
          <rect x="14" y="3" width="7" height="5" />
          <rect x="14" y="12" width="7" height="9" />
          <rect x="3" y="16" width="7" height="5" />
        </svg>
        <span className="theme-switcher-label">{t('theme.operationalShort')}</span>
      </button>
    </div>
  );
};

export default ThemeSwitcher;
