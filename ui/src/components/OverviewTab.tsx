import React, { useState, useEffect, useCallback } from 'react';
import {
  AccountResponse,
  AdminStats,
  CodexAccountRecord,
  DailyTokenStat,
  HealthInfo,
  ModelRequestSummary,
  TokenAnalyticsResponse,
} from '../types';
import { TabType } from './Sidebar';
import { IconActivity, IconAlertCircle, IconRefresh } from '../icons';
import { useI18n } from '../i18n';
import { api } from '../api';

interface OverviewTabProps {
  stats: AdminStats | null;
  summaries: ModelRequestSummary[];
  accounts: AccountResponse[];
  codexAccounts?: CodexAccountRecord[];
  health: HealthInfo | null;
  loading: boolean;
  refreshing?: boolean;
  onRefresh?: () => void;
  error: string | null;
  onSelectTab?: (tab: TabType) => void;
}

export const OverviewTab: React.FC<OverviewTabProps> = ({
  stats,
  summaries,
  accounts,
  codexAccounts = [],
  health,
  loading,
  refreshing = false,
  onRefresh,
  error,
}) => {
  const { locale, t } = useI18n();
  const [quotaTab, setQuotaTab] = useState<'all' | 'google' | 'codex'>('all');
  const [analyticsPeriod, setAnalyticsPeriod] = useState<string>('30d');
  const [chartMode, setChartMode] = useState<'stacked' | 'total'>('stacked');
  const [analytics, setAnalytics] = useState<TokenAnalyticsResponse | null>(null);
  const [analyticsLoading, setAnalyticsLoading] = useState<boolean>(false);
  const [hoveredDay, setHoveredDay] = useState<DailyTokenStat | null>(null);

  const loadTokenAnalytics = useCallback(async (p: string) => {
    setAnalyticsLoading(true);
    try {
      const res = await api.getTokenAnalytics(p);
      setAnalytics(res);
    } catch {
      // keep prior state on transient error
    } finally {
      setAnalyticsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTokenAnalytics(analyticsPeriod);
  }, [analyticsPeriod, loadTokenAnalytics, refreshing]);

  if (loading && !stats) {
    return (
      <div className="state-container">
        <div className="spinner" />
        <p>{t('overview.loading')}</p>
      </div>
    );
  }

  if (error && !stats) {
    return (
      <div className="alert alert-error">
        <IconAlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }

  const formatNumber = (n: number | undefined) =>
    (n ?? 0).toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US');

  const formatCompactNumber = (n: number | undefined) => {
    const value = n ?? 0;
    const units: Array<[number, string]> = [
      [1_000_000_000, t('overview.unitBillion')],
      [1_000_000, t('overview.unitMillion')],
      [1_000, t('overview.unitThousand')],
    ];
    const unit = units.find(([threshold]) => Math.abs(value) >= threshold);
    if (!unit) return formatNumber(value);
    const compact = value / unit[0];
    const decimals = compact >= 100 ? 0 : compact >= 10 ? 1 : 2;
    return `${compact.toFixed(decimals).replace(/\.0+$|(?<=\.\d)0+$/g, '')}${unit[1]}`;
  };
  const errorRate = stats?.error_rate ?? 0;
  const activeRate = stats?.active_rate ?? 0;
  const isHealthy = !error && health?.status === 'ok';
  const isPoolHealthy = (stats?.total_accounts ?? 0) > 0 && activeRate >= 80;
  const latencyTone =
    (stats?.avg_duration_ms ?? 0) <= 3000
      ? 'good'
      : (stats?.avg_duration_ms ?? 0) <= 8000
      ? 'warn'
      : 'bad';

  // Format ISO time or timestamp to relative countdown string
  const formatResetTime = (input?: string | number) => {
    if (input == null || input === '') return null;
    try {
      let d: Date;
      if (typeof input === 'number') {
        d = new Date(input < 1e11 ? input * 1000 : input);
      } else if (!isNaN(Number(input))) {
        const num = Number(input);
        d = new Date(num < 1e11 ? num * 1000 : num);
      } else {
        d = new Date(input);
      }
      if (isNaN(d.getTime())) return null;
      const diffSec = Math.max(0, Math.floor((d.getTime() - Date.now()) / 1000));
      if (diffSec <= 0) return t('overview.resetSoon');
      if (diffSec < 60) return `${diffSec}s`;
      if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m`;
      if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ${Math.floor((diffSec % 3600) / 60)}m`;
      const days = Math.floor(diffSec / 86400);
      const hours = Math.floor((diffSec % 86400) / 3600);
      return `${days}d ${hours}h`;
    } catch {
      return null;
    }
  };

  const formatSeconds = (sec?: number) => {
    if (sec == null || sec <= 0) return t('overview.resetSoon');
    if (sec < 60) return `${Math.floor(sec)}s`;
    if (sec < 3600) return `${Math.floor(sec / 60)}m`;
    if (sec < 86400) return `${Math.floor(sec / 3600)}h ${Math.floor((sec % 3600) / 60)}m`;
    const days = Math.floor(sec / 86400);
    const hours = Math.floor((sec % 86400) / 3600);
    return `${days}d ${hours}h`;
  };

  const getRemainingTone = (remainingPercent?: number | null) => {
    if (remainingPercent == null) return 'neutral';
    if (remainingPercent >= 50) return 'good';
    if (remainingPercent >= 20) return 'warn';
    return 'bad';
  };

  // Google Antigravity active accounts with quota
  const googleAccounts = accounts.filter((a) => a.is_active);
  // Codex active accounts
  const activeCodexAccounts = codexAccounts.filter(
    (a) => a.is_active !== false && a.active !== false
  );

  const totalQuotaAccounts = googleAccounts.length + activeCodexAccounts.length;

  // Chart calculation from model summaries
  const maxReq = Math.max(1, ...summaries.map((s) => s.requests));

  return (
    <div className="overview-container">
      {/* System Health Info Banner */}
      {health && (
        <div className={`system-health-banner ${isHealthy ? 'is-healthy' : 'is-warning'}`}>
          <div className="health-status">
            <span className={`status-dot ${isHealthy ? '' : 'error'}`} />
            <span style={{ fontSize: 13, fontWeight: 600, color: '#e5e7eb' }}>
              ezRouter: <strong>{health.service}</strong> ({health.mode})
            </span>
            <span className={`badge ${isHealthy ? 'badge-success' : 'badge-warning'}`}>
              {isHealthy ? t('overview.statusOnline') : t('overview.statusNeedsCheck')}
            </span>
          </div>
          <div className="health-meta">
            <span>{t('overview.healthPort')}: {health.port}</span>
            <span>{t('overview.healthUpstream')}: <strong>Antigravity + Codex</strong></span>
            <span>
              {t('overview.healthPool')}: <strong>{stats?.active_accounts ?? 0}/{stats?.total_accounts ?? 0} {t('overview.healthAvailable')}</strong>
            </span>
          </div>
        </div>
      )}

      {/* Symmetrical 3x2 KPI Grid */}
      <div className="kpi-grid">
        {/* Row 1, Col 1: Tổng Yêu Cầu */}
        <div className="card kpi-card kpi-card-blue">
          <div className="kpi-label">
            <span>{t('overview.totalRequests')}</span>
            <IconActivity size={16} />
          </div>
          <div className="kpi-value" title={formatNumber(stats?.total_requests)}>{formatCompactNumber(stats?.total_requests)}</div>
          <div className="kpi-sub">
            {t('overview.currentRpm', { rpm: stats?.rpm || 0 })}
          </div>
        </div>

        {/* Row 1, Col 2: Tổng Token */}
        <div className="card kpi-card kpi-card-cyan">
          <div className="kpi-label">
            <span>{t('overview.totalTokens')}</span>
            <span className="badge badge-neutral">{t('overview.tokenInOut')}</span>
          </div>
          <div className="kpi-value" title={formatNumber(stats?.total_tokens)}>{formatCompactNumber(stats?.total_tokens)}</div>
          <div className="kpi-sub" title={t('overview.tokenBreakdown', { prompt: formatNumber(stats?.prompt_tokens), completion: formatNumber(stats?.completion_tokens) })}>
            {t('overview.tokenInOutSummary', { prompt: formatCompactNumber(stats?.prompt_tokens), completion: formatCompactNumber(stats?.completion_tokens) })}
          </div>
        </div>

        {/* Row 1, Col 3: Độ Trễ TB */}
        <div className={`card kpi-card ${latencyTone === 'good' ? 'kpi-card-good' : 'kpi-card-warn'}`}>
          <div className="kpi-label">
            <span>{t('overview.avgLatency')}</span>
            <span className="badge badge-neutral">Gồm stream</span>
          </div>
          <div className="kpi-value">
            {stats?.avg_duration_ms
              ? stats.avg_duration_ms < 1000
                ? `${stats.avg_duration_ms.toFixed(0)} ms`
                : `${(stats.avg_duration_ms / 1000).toFixed(1)}s`
              : '0.0 ms'}
          </div>
          <div className="kpi-sub">
            {stats?.avg_duration_ms && stats.avg_duration_ms < 1000
              ? '⚡ Phản hồi ban đầu nhanh (<1s)'
              : '⚡ Thời gian stream hoàn tất trung bình'}
          </div>
        </div>

        {/* Row 2, Col 1: Tỷ Lệ Lỗi */}
        <div className={`card kpi-card ${errorRate <= 1.0 ? 'kpi-card-good' : errorRate <= 3.0 ? 'kpi-card-warn' : 'kpi-card-bad'}`}>
          <div className="kpi-label">
            <span>Tỷ Lệ Lỗi Hệ Thống</span>
            <span className={`badge ${errorRate <= 1.0 ? 'badge-success' : 'badge-error'}`}>
              {errorRate.toFixed(1)}%
            </span>
          </div>
          <div className="kpi-value" title={formatNumber(stats?.error_count)}>{formatCompactNumber(stats?.error_count)}</div>
          <div className="kpi-sub" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span>{stats?.error_count ?? 0} lỗi 5xx/mạng</span>
            {stats?.quota_count !== undefined && (
              <span style={{ color: 'var(--warning)', fontWeight: 500 }}>
                ⚡ Hết Quota: {stats.quota_count}
              </span>
            )}
          </div>
        </div>

        {/* Row 2, Col 2: Pool Khả Dụng */}
        <div className={`card kpi-card ${isPoolHealthy ? 'kpi-card-good' : 'kpi-card-warn'}`}>
          <div className="kpi-label">
            <span>{t('overview.poolActive')}</span>
            <span className={`badge ${isPoolHealthy ? 'badge-success' : 'badge-warning'}`}>{t('overview.poolBadge')}</span>
          </div>
          <div className="kpi-value">
            {stats?.active_accounts || 0} / {stats?.total_accounts || 0}
          </div>
          <div className="kpi-sub">
            {t('overview.cooldownLabel')}: <strong>{stats?.cooldown_accounts || 0}</strong> {t('overview.cooldownUnit')}
          </div>
        </div>

        {/* Row 2, Col 3: Tỷ Lệ Sẵn Sàng */}
        <div className={`card kpi-card ${activeRate >= 80 ? 'kpi-card-good' : 'kpi-card-warn'}`}>
          <div className="kpi-label">
            <span>{t('overview.readyRate')}</span>
            <span>%</span>
          </div>
          <div className="kpi-value">
            {stats?.active_rate != null ? stats.active_rate.toFixed(1) : '100.0'}%
          </div>
          <div className="kpi-sub">{t('overview.readyRateSub')}</div>
        </div>
      </div>

      {/* Token Usage Milestones & Daily Chart Section */}
      <div className="card" style={{ marginBottom: 24 }}>
        <div className="section-header">
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <h2 className="section-title" style={{ margin: 0 }}>{t('overview.tokenAnalyticsTitle')}</h2>
              <span className="badge badge-neutral" style={{ fontSize: 11 }}>Analytics</span>
            </div>
            <p className="overview-section-subtitle">
              {t('overview.tokenAnalyticsSubtitle')}
            </p>
          </div>
          <div className="overview-tab-pills">
            <button
              type="button"
              className={`pill-btn ${analyticsPeriod === '7d' ? 'active' : ''}`}
              onClick={() => setAnalyticsPeriod('7d')}
            >
              7D
            </button>
            <button
              type="button"
              className={`pill-btn ${analyticsPeriod === '14d' ? 'active' : ''}`}
              onClick={() => setAnalyticsPeriod('14d')}
            >
              14D
            </button>
            <button
              type="button"
              className={`pill-btn ${analyticsPeriod === '30d' ? 'active' : ''}`}
              onClick={() => setAnalyticsPeriod('30d')}
            >
              30D
            </button>
            <button
              type="button"
              className={`pill-btn ${analyticsPeriod === 'all' ? 'active' : ''}`}
              onClick={() => setAnalyticsPeriod('all')}
            >
              {t('overview.periodAll')}
            </button>
          </div>
        </div>

        {/* Mini Summary Stats Bar */}
        <div className="token-chart-summary-bar">
          <div className="token-summary-item">
            <span className="token-summary-item-label">{t('overview.chartTotalPeriod')}</span>
            <span className="token-summary-item-val" title={formatNumber(analytics?.summary?.total_tokens)}>
              {formatCompactNumber(analytics?.summary?.total_tokens)}
            </span>
          </div>
          <div className="token-summary-item">
            <span className="token-summary-item-label">{t('overview.chartPeak')}</span>
            <span className="token-summary-item-val" style={{ color: '#f59e0b' }}>
              🔥 {analytics?.summary?.peak_day || '—'} ({formatCompactNumber(analytics?.summary?.peak_tokens)})
            </span>
          </div>
          <div className="token-summary-item">
            <span className="token-summary-item-label">{t('overview.chartAvgDaily')}</span>
            <span className="token-summary-item-val">
              {formatCompactNumber(analytics?.summary?.avg_tokens_per_day)} / ngày
            </span>
          </div>
          <div className="token-summary-item">
            <span className="token-summary-item-label">{t('overview.chartTotalRequests')}</span>
            <span className="token-summary-item-val">
              {formatNumber(analytics?.summary?.total_requests)}
            </span>
          </div>
          <div style={{ marginLeft: 'auto', display: 'flex', gap: 6, alignItems: 'center' }}>
            <button
              type="button"
              className={`pill-btn ${chartMode === 'stacked' ? 'active' : ''}`}
              onClick={() => setChartMode('stacked')}
            >
              {t('overview.chartModeStacked')}
            </button>
            <button
              type="button"
              className={`pill-btn ${chartMode === 'total' ? 'active' : ''}`}
              onClick={() => setChartMode('total')}
            >
              {t('overview.chartModeTotal')}
            </button>
          </div>
        </div>

        {/* Interactive SVG Chart */}
        {analytics?.daily && analytics.daily.length > 0 ? (
          <div className="token-chart-svg-container">
            <svg
              viewBox="0 0 1000 240"
              width="100%"
              height="240"
              preserveAspectRatio="none"
              style={{ overflow: 'visible', display: 'block' }}
            >
              {/* Background horizontal grid lines */}
              {[0, 0.25, 0.5, 0.75, 1.0].map((ratio) => {
                const y = 20 + 170 * (1 - ratio);
                const maxVal = Math.max(1, ...(analytics.daily.map((d) => d.total_tokens)));
                const labelVal = maxVal * ratio;
                return (
                  <g key={ratio}>
                    <line
                      x1="65"
                      y1={y}
                      x2="980"
                      y2={y}
                      stroke="rgba(255, 255, 255, 0.08)"
                      strokeDasharray={ratio > 0 && ratio < 1 ? '4 4' : undefined}
                      strokeWidth="1"
                    />
                    <text
                      x="58"
                      y={y + 4}
                      fill="rgba(255, 255, 255, 0.4)"
                      fontSize="10"
                      textAnchor="end"
                      fontFamily="monospace"
                    >
                      {formatCompactNumber(labelVal)}
                    </text>
                  </g>
                );
              })}

              {/* Render Bars */}
              {(() => {
                const items = analytics.daily;
                const N = items.length;
                const maxTotal = Math.max(1, ...(items.map((d) => d.total_tokens)));
                const availableW = 980 - 75;
                const colW = availableW / N;
                const barW = Math.min(44, Math.max(10, colW * 0.65));

                return items.map((item, idx) => {
                  const cx = 75 + colW * idx + (colW - barW) / 2;
                  const isHovered = hoveredDay?.date === item.date;
                  const isPeak = item.date === analytics.summary?.peak_day;

                  const totalH = Math.max(3, (item.total_tokens / maxTotal) * 170);
                  const promptH = Math.max(2, (item.prompt_tokens / maxTotal) * 170);
                  const compH = Math.max(1, totalH - promptH);

                  const yPrompt = 20 + 170 - promptH;
                  const yComp = yPrompt - compH;
                  const yTotal = 20 + 170 - totalH;

                  const parts = item.date.split('-');
                  const shortDate = parts.length === 3 ? `${parts[2]}/${parts[1]}` : item.date;

                  return (
                    <g
                      key={item.date}
                      style={{ cursor: 'pointer' }}
                      onMouseEnter={() => setHoveredDay(item)}
                      onClick={() => setHoveredDay(item)}
                    >
                      {/* Selection / Hover column highlight */}
                      <rect
                        x={75 + colW * idx}
                        y="15"
                        width={colW}
                        height="180"
                        fill={isHovered ? 'rgba(255, 255, 255, 0.06)' : 'transparent'}
                        rx="4"
                      />

                      {chartMode === 'stacked' ? (
                        <>
                          {/* Prompt bar (lower) */}
                          <rect
                            x={cx}
                            y={yPrompt}
                            width={barW}
                            height={promptH}
                            fill={isHovered ? '#60a5fa' : '#3b82f6'}
                            rx={compH > 2 ? 0 : 3}
                            opacity={isHovered ? 1 : 0.9}
                          />
                          {/* Completion bar (upper) */}
                          <rect
                            x={cx}
                            y={yComp}
                            width={barW}
                            height={compH}
                            fill={isHovered ? '#34d399' : '#10b981'}
                            rx={3}
                            opacity={isHovered ? 1 : 0.95}
                          />
                        </>
                      ) : (
                        <rect
                          x={cx}
                          y={yTotal}
                          width={barW}
                          height={totalH}
                          fill={isPeak ? '#f59e0b' : isHovered ? '#60a5fa' : '#3b82f6'}
                          rx={3}
                          opacity={isHovered ? 1 : 0.9}
                        />
                      )}

                      {/* Flame indicator on peak day */}
                      {isPeak && (
                        <text
                          x={cx + barW / 2}
                          y={yComp - 6}
                          fontSize="12"
                          textAnchor="middle"
                        >
                          🔥
                        </text>
                      )}

                      {/* X-axis date text */}
                      <text
                        x={cx + barW / 2}
                        y="215"
                        fill={isHovered ? '#ffffff' : 'rgba(255, 255, 255, 0.5)'}
                        fontSize={N > 20 ? '9' : '10'}
                        fontWeight={isHovered ? '600' : '400'}
                        textAnchor="middle"
                        fontFamily="monospace"
                      >
                        {shortDate}
                      </text>
                    </g>
                  );
                });
              })()}
            </svg>

            {/* Interactive HUD Card on Hover / Touch */}
            {hoveredDay ? (
              <div className="token-detail-hud">
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span className="badge badge-success" style={{ fontWeight: 600 }}>
                    📅 {hoveredDay.date}
                  </span>
                  <span style={{ fontWeight: 600, color: '#fff', fontSize: 13 }}>
                    {formatNumber(hoveredDay.total_tokens)} tokens ({formatCompactNumber(hoveredDay.total_tokens)})
                  </span>
                </div>
                <div style={{ display: 'flex', gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
                  <span style={{ color: '#93c5fd' }}>
                    Prompt: <strong>{formatCompactNumber(hoveredDay.prompt_tokens)}</strong>
                  </span>
                  <span style={{ color: '#6ee7b7' }}>
                    Completion: <strong>{formatCompactNumber(hoveredDay.completion_tokens)}</strong>
                  </span>
                  <span>
                    ⚡ <strong>{formatNumber(hoveredDay.requests)}</strong> requests
                  </span>
                  <span>
                    Độ trễ TB: <strong>{hoveredDay.avg_duration_ms < 1000 ? `${hoveredDay.avg_duration_ms.toFixed(0)}ms` : `${(hoveredDay.avg_duration_ms / 1000).toFixed(1)}s`}</strong>
                  </span>
                  {hoveredDay.error_count > 0 && (
                    <span style={{ color: 'var(--error)' }}>
                      ⚠️ {hoveredDay.error_count} lỗi
                    </span>
                  )}
                </div>
              </div>
            ) : (
              <div className="token-chart-legend">
                <div className="legend-item">
                  <span className="legend-color-dot" style={{ backgroundColor: '#3b82f6' }} />
                  <span>{t('overview.chartPrompt')}</span>
                </div>
                <div className="legend-item">
                  <span className="legend-color-dot" style={{ backgroundColor: '#10b981' }} />
                  <span>{t('overview.chartCompletion')}</span>
                </div>
                <div className="legend-item">
                  <span>🔥 {t('overview.chartPeak')}</span>
                </div>
                <span style={{ marginLeft: 'auto', fontSize: 11, color: 'var(--text-muted)' }}>
                  💡 Chạm hoặc rê chuột vào cột ngày để xem chi tiết
                </span>
              </div>
            )}
          </div>
        ) : (
          <div style={{ textAlign: 'center', padding: '30px 0', color: 'var(--text-muted)' }}>
            {analyticsLoading ? <div className="spinner" style={{ margin: '0 auto 8px' }} /> : null}
            <p>{t('overview.chartEmpty')}</p>
          </div>
        )}
      </div>

      {/* Quota Overview Section */}
      <div className="card overview-quota-card" style={{ marginBottom: 24 }}>
        <div className="section-header">
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <h2 className="section-title" style={{ margin: 0 }}>{t('overview.quotaTitle')}</h2>
              <span className="badge badge-success" style={{ fontSize: 11 }}>{t('overview.liveBadge')}</span>
            </div>
            <p className="overview-section-subtitle">
              {t('overview.quotaSubtitle')}
            </p>
          </div>
          <div className="overview-quota-actions">
            <div className="overview-tab-pills">
              <button
                type="button"
                className={`pill-btn ${quotaTab === 'all' ? 'active' : ''}`}
                onClick={() => setQuotaTab('all')}
              >
                {t('overview.quotaTabAll')} ({totalQuotaAccounts})
              </button>
              <button
                type="button"
                className={`pill-btn ${quotaTab === 'google' ? 'active' : ''}`}
                onClick={() => setQuotaTab('google')}
              >
                {t('overview.quotaTabGoogle')} ({googleAccounts.length})
              </button>
              <button
                type="button"
                className={`pill-btn ${quotaTab === 'codex' ? 'active' : ''}`}
                onClick={() => setQuotaTab('codex')}
              >
                {t('overview.quotaTabCodex')} ({activeCodexAccounts.length})
              </button>
            </div>
            {onRefresh && (
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={onRefresh}
                disabled={refreshing}
                title={t('overview.refreshQuotaTitle')}
                aria-label={t('overview.refreshQuotaTitle')}
                style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}
              >
                <IconRefresh size={14} className={refreshing ? 'spinning' : ''} />
                <span>{t('actions.refresh')}</span>
              </button>
            )}
          </div>
        </div>

        {totalQuotaAccounts === 0 ? (
          <div className="overview-quota-empty">
            {t('overview.noAccountsInPool')}
          </div>
        ) : (
          <div className="overview-quota-container">
            {/* Google Antigravity Accounts */}
            {(quotaTab === 'all' || quotaTab === 'google') && googleAccounts.length > 0 && (
              <div className="quota-provider-group">
                <div className="quota-group-header">
                  <span className="badge badge-neutral">Google Antigravity ({googleAccounts.length})</span>
                  <small style={{ color: 'var(--text-muted)' }}>{t('overview.googleSubtitle')}</small>
                </div>
                <div className="overview-quota-grid">
                  {googleAccounts.map((acc) => {
                    const q = acc.quota || {};
                    const g5h = q.gemini_5h?.remaining_percent ?? null;
                    const g5hReset = formatResetTime(q.gemini_5h?.reset_time);
                    const gWeekly = q.gemini_weekly?.remaining_percent ?? null;
                    const gWeeklyReset = formatResetTime(q.gemini_weekly?.reset_time);
                    const c5h = q.claude_5h?.remaining_percent ?? null;
                    const c5hReset = formatResetTime(q.claude_5h?.reset_time);

                    return (
                      <div className="overview-quota-card-item" key={acc.id}>
                        <div className="overview-quota-account">
                          <span className={`status-dot ${acc.cooldown_remaining > 0 ? 'error' : ''}`} />
                          <span className="account-email" title={acc.email}>{acc.email}</span>
                          {acc.cooldown_remaining > 0 && (
                            <span className="badge badge-warning" style={{ marginLeft: 'auto', fontSize: 10 }}>
                              CD {Math.round(acc.cooldown_remaining)}s
                            </span>
                          )}
                        </div>

                        <div className="quota-metrics-stack">
                          {/* Gemini 5h */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>Gemini 5h</span>
                              <strong className={`quota-${getRemainingTone(g5h)}`}>
                                {g5h != null ? t('overview.quotaRemaining', { percent: g5h }) : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(g5h)}`}
                                style={{ width: `${Math.max(0, Math.min(100, g5h ?? 0))}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {g5hReset ? t('overview.resetAfter', { time: g5hReset }) : t('overview.noResetInfo')}
                            </div>
                          </div>

                          {/* Gemini Weekly */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>{t('overview.geminiWeekly')}</span>
                              <strong className={`quota-${getRemainingTone(gWeekly)}`}>
                                {gWeekly != null ? t('overview.quotaRemaining', { percent: gWeekly }) : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(gWeekly)}`}
                                style={{ width: `${Math.max(0, Math.min(100, gWeekly ?? 0))}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {gWeeklyReset ? t('overview.resetAfter', { time: gWeeklyReset }) : t('overview.noResetInfo')}
                            </div>
                          </div>

                          {/* Claude 5h */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>Claude 5h</span>
                              <strong className={`quota-${getRemainingTone(c5h)}`}>
                                {c5h != null ? t('overview.quotaRemaining', { percent: c5h }) : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(c5h)}`}
                                style={{ width: `${Math.max(0, Math.min(100, c5h ?? 0))}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {c5hReset ? t('overview.resetAfter', { time: c5hReset }) : t('overview.noResetInfo')}
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* OpenAI Codex Accounts */}
            {(quotaTab === 'all' || quotaTab === 'codex') && activeCodexAccounts.length > 0 && (
              <div className="quota-provider-group" style={{ marginTop: quotaTab === 'all' ? 20 : 0 }}>
                <div className="quota-group-header">
                  <span className="badge badge-neutral">OpenAI Codex ({activeCodexAccounts.length})</span>
                  <small style={{ color: 'var(--text-muted)' }}>GPT-5.6 Sol / Luna / Terra</small>
                </div>
                <div className="overview-quota-grid">
                  {activeCodexAccounts.map((acc) => {
                    const q = acc.quota || {};
                    const pWindow = q.primary_window;
                    const pUsed = pWindow?.used_percent ?? null;
                    const pRem = pUsed != null ? Math.max(0, Math.min(100, Math.round(100 - pUsed))) : null;
                    const pReset = pWindow?.reset_time
                      ? formatResetTime(pWindow.reset_time)
                      : pWindow?.reset_at
                      ? formatResetTime(pWindow.reset_at)
                      : formatSeconds(pWindow?.reset_after_seconds);

                    const wWindow = q.weekly_window ?? q.secondary_window;
                    const wUsed = wWindow?.used_percent ?? null;
                    const wRem = wUsed != null ? Math.max(0, Math.min(100, Math.round(100 - wUsed))) : null;
                    const wReset = wWindow?.reset_time
                      ? formatResetTime(wWindow.reset_time)
                      : wWindow?.reset_at
                      ? formatResetTime(wWindow.reset_at)
                      : formatSeconds(wWindow?.reset_after_seconds);

                    const tickets = q.rate_limit_reset_credits?.available_count ?? 0;

                    return (
                      <div className="overview-quota-card-item" key={acc.id}>
                        <div className="overview-quota-account">
                          <span className={`status-dot ${(acc.cooldown_remaining ?? 0) > 0 ? 'error' : ''}`} />
                          <span className="account-email" title={acc.email || acc.id}>
                            {acc.email || t('overview.codexAccountFallback', { id: acc.id.slice(0, 8) })}
                          </span>
                          {tickets > 0 && (
                            <span className="badge badge-info" style={{ marginLeft: 'auto', fontSize: 10 }}>
                              🎟️ {t('overview.codexTicketsBadge', { count: tickets })}
                            </span>
                          )}
                          {(acc.cooldown_remaining ?? 0) > 0 && (
                            <span className="badge badge-warning" style={{ marginLeft: tickets > 0 ? 4 : 'auto', fontSize: 10 }}>
                              CD {Math.round(acc.cooldown_remaining || 0)}s
                            </span>
                          )}
                        </div>

                        <div className="quota-metrics-stack">
                          {/* Primary 5h */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>{t('overview.quotaLimit5h')}</span>
                              <strong className={`quota-${getRemainingTone(pRem)}`}>
                                {pRem != null ? t('overview.quotaRemainingUsed', { rem: pRem, used: pUsed }) : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(pRem)}`}
                                style={{ width: `${pRem ?? 0}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {pReset ? t('overview.resetAfter', { time: pReset }) : t('overview.noResetInfo')}
                            </div>
                          </div>

                          {/* Weekly */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>{t('overview.quotaLimitWeekly')}</span>
                              <strong className={`quota-${getRemainingTone(wRem)}`}>
                                {wRem != null ? t('overview.quotaRemainingUsed', { rem: wRem, used: wUsed }) : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(wRem)}`}
                                style={{ width: `${wRem ?? 0}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {wReset ? t('overview.resetAfter', { time: wReset }) : t('overview.noResetInfo')}
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Model Request Summary Section */}
      <div className="card" style={{ marginBottom: 24 }}>
        <div className="section-header">
          <div>
            <h2 className="section-title">{t('overview.modelDistributionTitle')}</h2>
            <p className="overview-section-subtitle">{t('overview.modelsProcessed', { count: summaries.length })}</p>
          </div>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('overview.totalCalls', {
              count: formatNumber(summaries.reduce((sum, s) => sum + s.requests, 0)),
            })}
          </span>
        </div>

        {/* Visual Bar Chart */}
        {summaries.length > 0 && (
          <div style={{ marginBottom: 20 }}>
            <div className="chart-bar-container">
              {summaries.slice(0, 12).map((item) => {
                const heightPct = Math.max(8, (item.requests / maxReq) * 100);
                return (
                  <div key={item.model} className="chart-bar-column">
                    <div
                      className="chart-bar"
                      style={{
                        height: `${heightPct}%`,
                        backgroundColor: item.errors > 0 ? '#ef4444' : '#10b981',
                      }}
                      title={t('overview.chartBarTitle', {
                        model: item.model,
                        requests: item.requests,
                        ok: item.ok,
                        errors: item.errors,
                      })}
                    />
                    <div className="chart-bar-label" title={item.model}>
                      {item.model.split('/').pop() || item.model}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>{t('overview.tableModel')}</th>
                <th>{t('overview.tableRequests')}</th>
                <th>{t('overview.tableSuccess')}</th>
                <th>{t('overview.tableErrors')}</th>
                <th>{t('overview.tablePromptTokens')}</th>
                <th>{t('overview.tableCompletionTokens')}</th>
                <th>{t('overview.tableTotalTokens')}</th>
                <th>{t('overview.tableAvgLatency')}</th>
              </tr>
            </thead>
            <tbody>
              {summaries.length === 0 ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: 'center', padding: 32 }}>
                    {t('overview.tableEmpty')}
                  </td>
                </tr>
              ) : (
                summaries.map((s) => (
                  <tr key={s.model}>
                    <td style={{ fontWeight: 600, color: 'var(--text-primary)' }} className="font-mono text-break">
                      {s.model}
                    </td>
                    <td className="font-mono">{formatNumber(s.requests)}</td>
                    <td>
                      <span className={`badge ${s.ok === s.requests && s.requests > 0 ? 'badge-success' : 'badge-warning'} font-mono`}>
                        {formatNumber(s.ok)}
                      </span>
                    </td>
                    <td>
                      <span
                        className={`badge ${s.errors > 0 ? 'badge-error' : 'badge-success'} font-mono`}
                      >
                        {formatNumber(s.errors)}
                      </span>
                    </td>
                    <td className="font-mono">{formatNumber(s.prompt_tokens)}</td>
                    <td className="font-mono">{formatNumber(s.completion_tokens)}</td>
                    <td className="font-mono">{formatNumber(s.total_tokens)}</td>
                    <td className="font-mono">{s.avg_duration_ms.toFixed(1)} ms</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
