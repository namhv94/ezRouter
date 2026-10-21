import React, { useRef } from 'react';
import { useI18n } from '../i18n';
import {
  IconGoogle,
  IconOpenAI,
  IconOpenRouter,
  IconServer,
  IconRefresh,
  IconAlertCircle,
} from '../icons';
import { QuotaRefreshStatus } from '../types';
import '../operational-providers.css';

export type OperationalProviderTab = 'google' | 'codex' | 'openrouter' | 'upstream';

export interface OperationalProvidersViewProps {
  activeTab: OperationalProviderTab;
  onSelectTab: (tab: OperationalProviderTab) => void;
  onFullRefresh?: () => void;
  fullRefreshLoading?: boolean;
  quotaRefresh: QuotaRefreshStatus | null;
  quotaRefreshLoading: boolean;
  onChangeRefreshInterval: (seconds: number) => void;
  onToggleAutoRefresh: () => void;
  onManualRunRefresh: () => void;
  formatTimestamp: (ts: number | null) => string;
  formatCountdown: (ts: number | null) => string;
  googleActiveCount: number;
  googleTotalCount: number;
  googleQuotaAvg: number | null;
  googleStatus: string;
  codexActiveCount: number;
  codexTotalCount: number;
  codexPrimaryAvg: number | null;
  codexStatus: string;
  openrouterModelCount: number;
  openrouterBalance: string | null;
  openrouterActive: boolean;
  upstreamActiveCount: number;
  upstreamTotalCount: number;
  children?: React.ReactNode;
}

const TAB_ORDER: OperationalProviderTab[] = ['google', 'codex', 'openrouter', 'upstream'];

export const OperationalProvidersView: React.FC<OperationalProvidersViewProps> = ({
  activeTab,
  onSelectTab,
  onFullRefresh,
  fullRefreshLoading = false,
  quotaRefresh,
  quotaRefreshLoading,
  onChangeRefreshInterval,
  onToggleAutoRefresh,
  onManualRunRefresh,
  formatTimestamp,
  formatCountdown,
  googleActiveCount,
  googleTotalCount,
  googleQuotaAvg,
  googleStatus,
  codexActiveCount,
  codexTotalCount,
  codexPrimaryAvg,
  codexStatus,
  openrouterModelCount,
  openrouterBalance,
  openrouterActive,
  upstreamActiveCount,
  upstreamTotalCount,
  children,
}) => {
  const { t } = useI18n();
  const tabRefs = useRef<{ [key in OperationalProviderTab]?: HTMLButtonElement | null }>({});

  const totalActivePools = googleActiveCount + codexActiveCount + upstreamActiveCount;

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
                totalActivePools === 0 ? 'error' : ''
              }`}
            />
            <span>
              {totalActivePools > 0
                ? t('providers.routingReady')
                : t('providers.noActiveAccounts')}
            </span>
            <span style={{ color: 'var(--text-muted)', fontWeight: 400, fontSize: 11.5, marginLeft: 2 }}>
              {t('providers.readyCount', {
                active: googleActiveCount + codexActiveCount,
                total: googleTotalCount + codexTotalCount,
              })}
            </span>
          </div>

          <div className="op-refresh-meta">
            {quotaRefresh?.is_refreshing ? (
              <span style={{ color: 'var(--accent-primary, #0f766e)', fontWeight: 600 }}>
                {t('providers.autoRefreshRefreshing')}
              </span>
            ) : quotaRefresh?.enabled ? (
              <>
                <span>{t('providers.autoRefreshLast', { time: formatTimestamp(quotaRefresh?.last_refresh ?? null) })}</span>
                <span style={{ margin: '0 2px' }}>•</span>
                <span>{t('providers.autoRefreshNext', { time: formatCountdown(quotaRefresh?.next_refresh ?? null) })}</span>
              </>
            ) : (
              <span>{t('providers.autoRefreshDisabled')}</span>
            )}
          </div>
        </div>

        <div className="op-toolbar-right">
          <div className="op-interval-group" role="radiogroup" aria-label={t('providers.toggleAutoRefreshAria')}>
            {[
              { label: '1m', val: 60, title: t('providers.interval1m') },
              { label: '5m', val: 300, title: t('providers.interval5m') },
              { label: '15m', val: 900, title: t('providers.interval15m') },
              { label: '30m', val: 1800, title: t('providers.interval30m') },
            ].map(({ label, val, title }) => (
              <button
                key={val}
                type="button"
                title={title}
                className={`op-interval-btn ${quotaRefresh?.interval_secs === val ? 'active' : ''}`}
                onClick={() => onChangeRefreshInterval(val)}
                disabled={quotaRefreshLoading}
              >
                {label}
              </button>
            ))}
          </div>

          <button
            type="button"
            className={`op-toggle-btn ${quotaRefresh?.enabled ? 'enabled' : 'disabled'}`}
            onClick={onToggleAutoRefresh}
            disabled={quotaRefreshLoading}
            aria-pressed={quotaRefresh?.enabled}
            title={quotaRefresh?.enabled ? t('providers.turnOffAutoRefresh') : t('providers.turnOnAutoRefresh')}
          >
            <span className="toggle-switch-track">
              <span className="toggle-switch-thumb" />
            </span>
            <span>{quotaRefresh?.enabled ? t('providers.toggleOn') : t('providers.toggleOff')}</span>
          </button>

          <button
            type="button"
            className="op-manual-refresh-btn"
            onClick={onManualRunRefresh}
            disabled={quotaRefreshLoading || quotaRefresh?.is_refreshing}
            title={t('providers.manualRefreshTitle')}
          >
            <IconRefresh size={13} className={quotaRefreshLoading || quotaRefresh?.is_refreshing ? 'spinner' : ''} />
            <span>{t('providers.manualRefreshBtn')}</span>
          </button>
        </div>

        {quotaRefresh?.last_error && (
          <div
            className="auto-quota-error-notice"
            style={{ width: '100%', marginTop: 4, display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: '#b91c1c' }}
          >
            <IconAlertCircle size={14} />
            <span>{t('providers.lastRefreshNotice', { error: quotaRefresh.last_error })}</span>
          </div>
        )}
      </div>

      {/* 2. Theme-Scoped Segmented Navigation */}
      <div className="op-segmented-nav-wrapper">
        <div
          className="op-segmented-tablist"
          role="tablist"
          aria-label={t('providers.title')}
        >
          {/* TAB 1: Google Antigravity */}
          <button
            ref={(el) => { tabRefs.current.google = el; }}
            id="op-tab-google"
            type="button"
            role="tab"
            aria-selected={activeTab === 'google'}
            aria-controls="provider-google"
            tabIndex={activeTab === 'google' ? 0 : -1}
            className="op-segmented-tab"
            onClick={() => onSelectTab('google')}
            onKeyDown={(e) => handleTabKeyDown(e, 'google')}
          >
            <IconGoogle size={15} />
            <span>{t('providers.googleTitle')}</span>
            <span className="op-tab-badge">
              {googleActiveCount}/{googleTotalCount}
            </span>
            {googleQuotaAvg !== null && (
              <span className="op-tab-quota-pill">{googleQuotaAvg}%</span>
            )}
            <span className={`op-status-dot-mini ${googleStatus}`} />
          </button>

          {/* TAB 2: OpenAI Codex */}
          <button
            ref={(el) => { tabRefs.current.codex = el; }}
            id="op-tab-codex"
            type="button"
            role="tab"
            aria-selected={activeTab === 'codex'}
            aria-controls="provider-codex"
            tabIndex={activeTab === 'codex' ? 0 : -1}
            className="op-segmented-tab"
            onClick={() => onSelectTab('codex')}
            onKeyDown={(e) => handleTabKeyDown(e, 'codex')}
          >
            <IconOpenAI size={15} />
            <span>{t('providers.codexTitle')}</span>
            <span className="op-tab-badge">
              {codexActiveCount}/{codexTotalCount}
            </span>
            {codexPrimaryAvg !== null && (
              <span className="op-tab-quota-pill">{codexPrimaryAvg}%</span>
            )}
            <span className={`op-status-dot-mini ${codexStatus}`} />
          </button>

          {/* TAB 3: OpenRouter */}
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

          {/* TAB 4: Upstream */}
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
            {activeTab === 'google' && t('providers.googleTitle')}
            {activeTab === 'codex' && t('providers.codexTitle')}
            {activeTab === 'openrouter' && 'OpenRouter'}
            {activeTab === 'upstream' && t('providers.upstreamTitle')}
          </span>
          <div className="op-focus-badges">
            {activeTab === 'google' && <span className="model-family-chip">ag/*</span>}
            {activeTab === 'codex' && <span className="model-family-chip">cx/*</span>}
            {activeTab === 'openrouter' && <span className="model-family-chip">openrouter/*</span>}
            {activeTab === 'upstream' && <span className="model-family-chip">upstream/*</span>}
          </div>
        </div>

        <div className="op-focus-stats">
          {activeTab === 'google' && (
            <>
              <span className="op-focus-stat-pill">
                {t('providers.statActive')}: <strong>{googleActiveCount}/{googleTotalCount}</strong>
              </span>
              {googleQuotaAvg !== null && (
                <span className="op-focus-stat-pill">
                  Quota TB: <strong>{googleQuotaAvg}%</strong>
                </span>
              )}
            </>
          )}
          {activeTab === 'codex' && (
            <>
              <span className="op-focus-stat-pill">
                {t('providers.statActive')}: <strong>{codexActiveCount}/{codexTotalCount}</strong>
              </span>
              {codexPrimaryAvg !== null && (
                <span className="op-focus-stat-pill">
                  Primary TB: <strong>{codexPrimaryAvg}%</strong>
                </span>
              )}
            </>
          )}
          {activeTab === 'openrouter' && (
            <>
              <span className="op-focus-stat-pill">
                Models: <strong>{openrouterModelCount}</strong>
              </span>
              {openrouterBalance && (
                <span className="op-focus-stat-pill">
                  Số dư: <strong>{openrouterBalance}</strong>
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
