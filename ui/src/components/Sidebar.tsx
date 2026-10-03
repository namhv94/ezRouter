import React, { useEffect } from 'react';
import {
  IconActivity,
  IconKey,
  IconServer,
  IconLayers,
  IconZap,
  IconX,
  IconPlayground,
  IconImage,
  IconBookOpen,
  IconShield,
  IconTerminal,
  EzRouterMark,
} from '../icons';
import { HealthInfo } from '../types';
import { useI18n } from '../i18n';

export type TabType =
  | 'overview'
  | 'integration'
  | 'traffic'
  | 'playground'
  | 'images'
  | 'requests'
  | 'providers'
  | 'accounts'
  | 'codex'
  | 'combos'
  | 'api_keys'
  | 'token_saver'
  | 'logs';

interface SidebarProps {
  currentTab: TabType;
  onSelectTab: (tab: TabType) => void;
  isOpen: boolean;
  onClose: () => void;
  health: HealthInfo | null;
  activeAccountsCount?: number;
  totalProvidersCount?: number;
  totalKeysCount?: number;
}

export const Sidebar: React.FC<SidebarProps> = ({
  currentTab,
  onSelectTab,
  isOpen,
  onClose,
  health,
  activeAccountsCount,
  totalProvidersCount,
  totalKeysCount,
}) => {
  const { t } = useI18n();

  const navItems = [
    {
      id: 'overview' as TabType,
      label: t('nav.overview'),
      icon: <IconActivity size={18} />,
    },
    {
      id: 'requests' as TabType,
      label: t('nav.requests'),
      icon: <IconZap size={18} />,
    },
    {
      id: 'playground' as TabType,
      label: t('nav.playground'),
      icon: <IconPlayground size={18} />,
    },
    {
      id: 'images' as TabType,
      label: t('nav.images'),
      icon: <IconImage size={18} />,
    },
    {
      id: 'providers' as TabType,
      label: t('nav.providers'),
      icon: <IconServer size={18} />,
      badge:
        totalProvidersCount !== undefined || activeAccountsCount !== undefined
          ? String((totalProvidersCount || 0) + (activeAccountsCount || 0))
          : undefined,
    },
    {
      id: 'combos' as TabType,
      label: t('nav.combos'),
      icon: <IconLayers size={18} />,
    },
    {
      id: 'token_saver' as TabType,
      label: t('nav.tokenSaver'),
      icon: <IconShield size={18} />,
    },
    {
      id: 'api_keys' as TabType,
      label: t('nav.apiKeys'),
      icon: <IconKey size={18} />,
      badge: totalKeysCount !== undefined ? String(totalKeysCount) : undefined,
    },
    {
      id: 'logs' as TabType,
      label: t('nav.logsShort'),
      icon: <IconTerminal size={18} />,
    },
    {
      id: 'integration' as TabType,
      label: t('nav.integration'),
      icon: <IconBookOpen size={18} />,
    },
  ];

  // Handle ESC key to close drawer
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  // Lock body scroll on mobile when drawer is open
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  const handleItemClick = (id: TabType) => {
    onSelectTab(id);
    onClose();
  };

  return (
    <>
      <div
        className={`mobile-overlay ${isOpen ? 'open' : ''}`}
        onClick={onClose}
        aria-hidden="true"
      />
      <aside className={`sidebar ${isOpen ? 'open' : ''}`}>
        <div className="sidebar-header">
          <div className="sidebar-logo">
            <EzRouterMark size={24} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="sidebar-title brand-title">
              <span className="brand-ez">ez</span>
              <span className="brand-router">Router</span>
            </div>
            <div className="sidebar-subtitle">{t('app.subtitle')}</div>
          </div>
          <button
            className="sidebar-close-btn"
            onClick={onClose}
            aria-label={t('actions.closeNavAria')}
          >
            <IconX size={18} />
          </button>
        </div>

        <nav className="sidebar-nav">
          {navItems.map((item) => (
            <button
              key={item.id}
              className={`nav-item ${currentTab === item.id ? 'active' : ''}`}
              onClick={() => handleItemClick(item.id)}
            >
              {item.icon}
              <span>{item.label}</span>
              {item.badge && <span className="nav-badge">{item.badge}</span>}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div className="service-status-pill">
            <span className={`status-dot ${health?.status === 'ok' ? '' : 'error'}`} />
            <div>
              <div style={{ fontWeight: 600, fontSize: 12, color: '#e5e7eb' }}>
                {health?.status === 'ok' ? t('app.online') : t('app.offline')}
              </div>
              <div style={{ fontSize: 11, color: '#6b7280', fontFamily: 'var(--font-mono)' }}>
                {t('app.portLabel', { port: health?.port || 20229, mode: health?.mode || 'staging' })}
              </div>
            </div>
          </div>
        </div>
      </aside>
    </>
  );
};
