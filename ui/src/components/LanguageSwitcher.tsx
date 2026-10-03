import React, { useRef } from 'react';
import { useI18n } from '../i18n';

interface LanguageSwitcherProps {
  className?: string;
}

export const LanguageSwitcher: React.FC<LanguageSwitcherProps> = ({ className = '' }) => {
  const { locale, setLocale, t } = useI18n();
  const viRef = useRef<HTMLButtonElement>(null);
  const enRef = useRef<HTMLButtonElement>(null);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      if (locale !== 'en') {
        setLocale('en');
      }
      enRef.current?.focus();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (locale !== 'vi') {
        setLocale('vi');
      }
      viRef.current?.focus();
    }
  };

  return (
    <div
      className={`lang-switcher ${className}`.trim()}
      role="group"
      aria-label={t('language.selectLanguage')}
    >
      <button
        ref={viRef}
        type="button"
        lang="vi"
        className={`lang-switcher-btn ${locale === 'vi' ? 'active' : ''}`}
        onClick={() => setLocale('vi')}
        onKeyDown={handleKeyDown}
        aria-pressed={locale === 'vi'}
        aria-label={t('language.vi')}
        title={t('language.vi')}
      >
        VI
      </button>
      <button
        ref={enRef}
        type="button"
        lang="en"
        className={`lang-switcher-btn ${locale === 'en' ? 'active' : ''}`}
        onClick={() => setLocale('en')}
        onKeyDown={handleKeyDown}
        aria-pressed={locale === 'en'}
        aria-label={t('language.en')}
        title={t('language.en')}
      >
        EN
      </button>
    </div>
  );
};

export default LanguageSwitcher;
