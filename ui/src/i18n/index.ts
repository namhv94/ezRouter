import React, { createContext, useContext, useState, useEffect, useCallback, useMemo } from 'react';
import {
  Locale,
  TranslationSchema,
  AnyTranslationKey,
  TranslationParams,
  I18nContextValue,
} from './types';
import { vi } from './vi';
import { en } from './en';

export * from './types';
export { vi, en };

export const STORAGE_KEY = 'ezrouter.locale';
export const DEFAULT_LOCALE: Locale = 'vi';
export const SUPPORTED_LOCALES: Locale[] = ['vi', 'en'];

export const catalogues: Record<Locale, TranslationSchema> = { vi, en };

export function isValidLocale(val: unknown): val is Locale {
  return val === 'vi' || val === 'en';
}

export function getInitialLocale(): Locale {
  if (typeof window === 'undefined') return DEFAULT_LOCALE;

  try {
    const stored = window.localStorage?.getItem(STORAGE_KEY);
    if (isValidLocale(stored)) {
      return stored;
    }
  } catch {
    // Ignore localStorage access errors
  }

  try {
    const browserLang = window.navigator?.language?.toLowerCase() || '';
    if (browserLang.startsWith('vi')) {
      return 'vi';
    }
    if (browserLang.startsWith('en')) {
      return 'en';
    }
    // If browser language is present and non-Vietnamese, choose English
    if (browserLang) {
      return 'en';
    }
  } catch {
    // Ignore navigator access errors
  }

  return DEFAULT_LOCALE;
}

export function interpolate(template: string, params?: TranslationParams): string {
  if (!params || Object.keys(params).length === 0) {
    return template;
  }
  return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (match, key) => {
    return Object.prototype.hasOwnProperty.call(params, key) ? String(params[key]) : match;
  });
}

function getNestedString(obj: unknown, path: string): string | undefined {
  if (obj == null || typeof obj !== 'object') return undefined;
  const parts = path.split('.');
  let current: any = obj;
  for (const part of parts) {
    if (current == null || typeof current !== 'object') {
      return undefined;
    }
    current = current[part];
  }
  return typeof current === 'string' ? current : undefined;
}

function isDevelopmentMode(): boolean {
  try {
    if (typeof import.meta !== 'undefined' && (import.meta as any).env?.DEV) {
      return true;
    }
  } catch {
    // Ignore
  }
  try {
    const g = globalThis as any;
    if (typeof g.process !== 'undefined' && g.process.env?.NODE_ENV !== 'production') {
      return true;
    }
  } catch {
    // Ignore
  }
  return false;
}

export function translate(
  locale: Locale,
  key: AnyTranslationKey,
  params?: TranslationParams
): string {
  // 1. Look up in current locale catalogue
  let text = getNestedString(catalogues[locale], key);

  // 2. Fall back to Vietnamese if missing in current locale
  if (text === undefined && locale !== 'vi') {
    text = getNestedString(catalogues.vi, key);
    if (text !== undefined && isDevelopmentMode()) {
      console.warn(`[i18n] Missing translation for "${key}" in locale "${locale}", falling back to Vietnamese.`);
    }
  }

  // 3. Fall back to key itself if not found anywhere
  if (text === undefined) {
    if (isDevelopmentMode()) {
      console.warn(`[i18n] Translation key "${key}" not found.`);
    }
    return interpolate(key, params);
  }

  return interpolate(text, params);
}

const defaultContextValue: I18nContextValue = {
  locale: DEFAULT_LOCALE,
  setLocale: () => {},
  t: (key: AnyTranslationKey, params?: TranslationParams) => translate(DEFAULT_LOCALE, key, params),
};

export const I18nContext = createContext<I18nContextValue>(defaultContextValue);

export interface I18nProviderProps {
  children: React.ReactNode;
  initialLocale?: Locale;
}

export const I18nProvider: React.FC<I18nProviderProps> = ({ children, initialLocale }) => {
  const [locale, setLocaleState] = useState<Locale>(() => initialLocale ?? getInitialLocale());

  const setLocale = useCallback((newLocale: Locale) => {
    if (!isValidLocale(newLocale)) return;
    setLocaleState(newLocale);
    try {
      window.localStorage?.setItem(STORAGE_KEY, newLocale);
    } catch {
      // Ignore storage errors
    }
  }, []);

  useEffect(() => {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = locale;
      const title = translate(locale, 'app.title');
      if (title && title !== 'app.title') {
        document.title = title;
      }
    }
  }, [locale]);

  const t = useCallback(
    (key: AnyTranslationKey, params?: TranslationParams) => {
      return translate(locale, key, params);
    },
    [locale]
  );

  const value = useMemo<I18nContextValue>(
    () => ({
      locale,
      setLocale,
      t,
    }),
    [locale, setLocale, t]
  );

  return React.createElement(I18nContext.Provider, { value }, children);
};

export function useI18n(): I18nContextValue {
  const context = useContext(I18nContext);
  return context || defaultContextValue;
}
