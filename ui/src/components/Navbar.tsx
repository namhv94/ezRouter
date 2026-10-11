import React from 'react';
import { IconMenu, IconRefresh, IconLogOut, IconKey, EzRouterMark } from '../icons';
import { useI18n } from '../i18n';
import { LanguageSwitcher } from './LanguageSwitcher';
import { ThemeSwitcher, ThemeMode } from './ThemeSwitcher';

interface NavbarProps {
  title: string;
  onToggleSidebar: () => void;
  onRefresh: () => void;
  refreshing: boolean;
  adminKey: string;
  onLogout: () => void;
  theme?: ThemeMode;
  onThemeChange?: (theme: ThemeMode) => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  title,
  onToggleSidebar,
  onRefresh,
  refreshing,
  adminKey,
  onLogout,
  theme,
  onThemeChange,
}) => {
  const { t } = useI18n();
  const maskedKey =
    adminKey.length > 8
      ? `${adminKey.slice(0, 4)}••••${adminKey.slice(-4)}`
      : '••••••••';

  return (
    <header className="top-navbar">
      <div className="nav-left">
        <button
          className="menu-toggle-btn"
          onClick={onToggleSidebar}
          aria-label={t('actions.openNavAria')}
        >
          <IconMenu size={20} />
        </button>
        <div className="nav-title-group" style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0, flex: 1 }}>
          <span className="mobile-brand-mark" style={{ display: 'none' }}>
            <EzRouterMark size={22} />
          </span>
          <h1 className="page-title">{title}</h1>
        </div>
      </div>

      <div className="nav-right">
        <div
          className="badge badge-neutral nav-key-badge"
          title={t('auth.currentAdminKey', { key: maskedKey })}
        >
          <IconKey size={14} style={{ color: 'var(--accent-primary)' }} />
          <span style={{ fontSize: 12 }}>{maskedKey}</span>
        </div>

        <ThemeSwitcher theme={theme} onThemeChange={onThemeChange} className="nav-theme-switcher" />
        <LanguageSwitcher />

        <button
          className="btn btn-secondary btn-sm nav-action-btn"
          onClick={onRefresh}
          disabled={refreshing}
          title={t('actions.refreshTitle')}
          aria-label={t('actions.refreshTitle')}
        >
          <IconRefresh size={16} className={refreshing ? 'spinner' : ''} />
          <span className="btn-label">{t('actions.refresh')}</span>
        </button>

        <button
          className="btn btn-secondary btn-sm nav-action-btn"
          onClick={onLogout}
          title={t('actions.logoutTitle')}
          aria-label={t('actions.logoutTitle')}
        >
          <IconLogOut size={16} />
          <span className="btn-label">{t('actions.logout')}</span>
        </button>
      </div>
    </header>
  );
};
