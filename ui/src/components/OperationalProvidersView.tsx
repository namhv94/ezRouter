import React, { useRef } from 'react';
import { useI18n } from '../i18n';
import {
  IconOpenRouter,
  IconServer,
  IconRefresh,
} from '../icons';
import '../operational-providers.css';

export type OperationalProviderTab = 'openrouter' | 'upstream';

export interface OperationalProvidersViewProps {
  activeTab: OperationalProviderTab;
  onSelectTab: (tab: OperationalProviderTab) => void;
  onFullRefresh?: () => void;
  fullRefreshLoading?: boolean;
  activePoolCount: number;
  totalPoolCount: number;
  openrouterModelCount: number;
  openrouterBalance: string | null;
  openrouterActive: boolean;
  upstreamActiveCount: number;
  upstreamTotalCount: number;
  children?: React.ReactNode;
}

const TAB_ORDER: OperationalProviderTab[] = ['openrouter', 'upstream'];

export const OperationalProvidersView: React.FC<OperationalProvidersViewProps> = ({
  activeTab,
  onSelectTab,
  onFullRefresh,
  fullRefreshLoading = false,
  activePoolCount,
  totalPoolCount,
  openrouterModelCount,
  openrouterBalance,
  openrouterActive,
  upstreamActiveCount,
  upstreamTotalCount,
  children,
}) => {
  const { t } = useI18n();
  const tabRefs = useRef<{ [key in OperationalProviderTab]?: HTMLButtonElement | null }>({});

  const handleTabKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>, currentTab: OperationalProviderTab) => {
    const currentIndex = TAB_ORDER.indexOf(currentTab);
    if (currentIndex === -1) return;

    let targetTab: OperationalProviderTab | null = null;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      const nextIndex = (currentIndex + 1) % TAB_ORDER.length;
      targetTab = TAB_ORDER[nextIndex];
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      const prevIndex = (currentIndex - 1 + TAB_ORDER.length) % TAB_ORDER.length;
      targetTab = TAB_ORDER[prevIndex];
    } else if (e.key === 'Home') {
      e.preventDefault();
      targetTab = TAB_ORDER[0];
    } else if (e.key === 'End') {
      e.preventDefault();
      targetTab = TAB_ORDER[TAB_ORDER.length - 1];
    }

    if (targetTab) {
      onSelectTab(targetTab);
      tabRefs.current[targetTab]?.focus();
    }
  };

  return (
    <div className="operational-providers-container" style={{ width: '100%' }}>
      {/* 0. Operational Header */}
      <div className="operational-providers-header" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
        <div className="operational-header-info">
          <h2 style={{ fontSize: 16, fontWeight: 700, margin: 0, display: 'flex', alignItems: 'center', gap: 8, color: 'var(--text-primary)' }}>
            <span>{t('providers.headerTitle')}</span>
            <span style={{ fontSize: 10, padding: '2px 6px', borderRadius: 4, background: '#ccfbf1', color: '#0f766e', fontWeight: 700, letterSpacing: '0.05em' }}>
              OPERATIONAL
            </span>
          </h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('providers.headerSub')}
          </span>
        </div>
        {onFullRefresh && (
          <div className="operational-header-actions">
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={onFullRefresh}
              disabled={fullRefreshLoading}
              title={t('providers.refreshTitle')}
              style={{ minHeight: 36 }}
            >
              <IconRefresh size={14} className={fullRefreshLoading ? 'spinner' : ''} />
              <span>{t('providers.refreshBtn')}</span>
            </button>
          </div>
        )}
      </div>

      {/* 1. Compact Operational Toolbar */}
      <div className="op-toolbar">
        <div className="op-toolbar-left">
          <div className="op-route-pill">
            <span
              className={`route-pulse-dot ${
                activePoolCount === 0 ? 'error' : ''
              }`}
            />
            <span>
              {activePoolCount > 0
                ? t('providers.routingReady')
                : t('providers.noActiveAccounts')}
            </span>
            <span style={{ color: 'var(--text-muted)', fontWeight: 400, fontSize: 11.5, marginLeft: 2 }}>
              {t('providers.readyCount', {
                active: activePoolCount,
                total: totalPoolCount,
              })}
            </span>
          </div>
        </div>
      </div>

      {/* 2. Theme-Scoped Segmented Navigation */}
      <div className="op-segmented-nav-wrapper">
        <div
          className="op-segmented-tablist"
          role="tablist"
          aria-label={t('providers.title')}
        >
          {/* TAB 1: OpenRouter */}
          <button
            ref={(el) => { tabRefs.current.openrouter = el; }}
            id="op-tab-openrouter"
            type="button"
            role="tab"
            aria-selected={activeTab === 'openrouter'}
            aria-controls="provider-openrouter"
            tabIndex={activeTab === 'openrouter' ? 0 : -1}
            className="op-segmented-tab"
            onClick={() => onSelectTab('openrouter')}
            onKeyDown={(e) => handleTabKeyDown(e, 'openrouter')}
          >
            <IconOpenRouter size={15} />
            <span>OpenRouter</span>
            <span className="op-tab-badge">{openrouterModelCount} models</span>
            {openrouterBalance && (
              <span className="op-tab-quota-pill">{openrouterBalance}</span>
            )}
            <span className={`op-status-dot-mini ${openrouterActive ? 'active' : 'neutral'}`} />
          </button>

          {/* TAB 2: Upstream */}
          <button
            ref={(el) => { tabRefs.current.upstream = el; }}
            id="op-tab-upstream"
            type="button"
            role="tab"
            aria-selected={activeTab === 'upstream'}
            aria-controls="provider-upstream"
            tabIndex={activeTab === 'upstream' ? 0 : -1}
            className="op-segmented-tab"
            onClick={() => onSelectTab('upstream')}
            onKeyDown={(e) => handleTabKeyDown(e, 'upstream')}
          >
            <IconServer size={15} />
            <span>{t('providers.upstreamTitle')}</span>
            <span className="op-tab-badge">
              {upstreamActiveCount}/{upstreamTotalCount}
            </span>
            <span className="op-status-dot-mini active" />
          </button>
        </div>
      </div>

      {/* 3. Operational Single Provider Focus Summary */}
      <div className="op-focus-summary">
        <div className="op-focus-left">
          <span className="op-focus-title">
            {activeTab === 'openrouter' && 'OpenRouter'}
            {activeTab === 'upstream' && t('providers.upstreamTitle')}
          </span>
          <div className="op-focus-badges">
            {activeTab === 'openrouter' && <span className="model-family-chip">openrouter/*</span>}
            {activeTab === 'upstream' && <span className="model-family-chip">upstream/*</span>}
          </div>
        </div>

        <div className="op-focus-stats">
          {activeTab === 'openrouter' && (
            <>
              <span className="op-focus-stat-pill">
                Models: <strong>{openrouterModelCount}</strong>
              </span>
              {openrouterBalance && (
                <span className="op-focus-stat-pill">
                  {t('providers.balanceLabel')}: <strong>{openrouterBalance}</strong>
                </span>
              )}
            </>
          )}
          {activeTab === 'upstream' && (
            <span className="op-focus-stat-pill">
              {t('providers.statActive')}: <strong>{upstreamActiveCount}/{upstreamTotalCount}</strong>
            </span>
          )}
        </div>
      </div>

      {/* 4. Optional Children (Provider Panels, single active visible via CSS) */}
      {children && (
        <div className="provider-segment-container operational-segment-container">
          {children}
        </div>
      )}
    </div>
  );
};
