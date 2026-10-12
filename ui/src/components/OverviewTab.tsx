import React, { useState, useEffect, useCallback, useMemo } from 'react';
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
import {
  IconAlertCircle,
  IconRefresh,
  IconZap,
  IconServer,
  IconChevronDown,
  IconChevronUp,
} from '../icons';
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
  onSelectTab,
}) => {
  const { locale, t } = useI18n();
  const [quotaTab, setQuotaTab] = useState<'all' | 'google' | 'codex'>('all');
  const [analyticsPeriod, setAnalyticsPeriod] = useState<string>('30d');
  const [chartMode, setChartMode] = useState<'total' | 'stacked'>('total');
  const [analytics, setAnalytics] = useState<TokenAnalyticsResponse | null>(null);
  const [analyticsLoading, setAnalyticsLoading] = useState<boolean>(false);
  const [hoveredDay, setHoveredDay] = useState<DailyTokenStat | null>(null);
  const [showDetailedAccounts, setShowDetailedAccounts] = useState<boolean>(false);
  const [showFullModelsTable, setShowFullModelsTable] = useState<boolean>(false);

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
  const activeRateNum = stats?.active_rate ?? 0;
  const activeRateStr = stats?.active_rate != null ? stats.active_rate.toFixed(1) : '100.0';
  const isHealthy = !error && health?.status === 'ok';
  const isPoolHealthy = (stats?.total_accounts ?? 0) > 0 && activeRateNum >= 80;
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

  const formatCooldown = (sec?: number | null) => {
    if (sec == null || sec <= 0) return null;
    const s = Math.ceil(sec);
    if (s <= 60) return `${s}s`;
    const m = Math.ceil(s / 60);
    return `${m}m`;
  };

  const getRemainingTone = (remainingPercent?: number | null) => {
    if (remainingPercent == null) return 'neutral';
    if (remainingPercent >= 50) return 'good';
    if (remainingPercent >= 20) return 'warn';
    return 'bad';
  };

  const googleAccounts = accounts.filter((a) => a.is_active);
  const activeCodexAccounts = codexAccounts.filter((a) => a.is_active ?? a.active ?? true);
  const totalQuotaAccounts = googleAccounts.length + activeCodexAccounts.length;

  // Aggregate Quota Pool Capacities
  const poolCapacities = useMemo(() => {
    const geminiVals = googleAccounts
      .map((a) => a.quota?.gemini_5h?.remaining_percent)
      .filter((v): v is number => typeof v === 'number');
    const claudeVals = googleAccounts
      .map((a) => a.quota?.claude_5h?.remaining_percent)
      .filter((v): v is number => typeof v === 'number');
    const codexPrimaryVals = activeCodexAccounts
      .map((a) => {
        const pWin = a.quota?.primary_window;
        const used = pWin?.used_percent ?? (a.quota?.primary_percent !== undefined ? 100 - a.quota.primary_percent : null);
        return used !== null ? Math.max(0, Math.min(100, Math.round(100 - used))) : null;
      })
      .filter((v): v is number => typeof v === 'number');

    const avg = (arr: number[]) => (arr.length > 0 ? Math.round(arr.reduce((a, b) => a + b, 0) / arr.length) : null);

    return {
      geminiPoolAvg: avg(geminiVals),
      claudePoolAvg: avg(claudeVals),
      codexPrimaryPoolAvg: avg(codexPrimaryVals),
      googleActive: accounts.filter((a) => a.is_active && (a.cooldown_remaining ?? 0) <= 0).length,
      googleTotal: accounts.length,
      googleCooling: accounts.filter((a) => a.is_active && (a.cooldown_remaining ?? 0) > 0).length,
      codexActive: codexAccounts.filter((a) => (a.is_active ?? a.active ?? true) && (a.cooldown_remaining ?? 0) <= 0).length,
      codexTotal: codexAccounts.length,
      codexCooling: codexAccounts.filter((a) => (a.cooldown_remaining ?? 0) > 0).length,
    };
  }, [accounts, googleAccounts, codexAccounts, activeCodexAccounts]);

  // Zero-filled daily series to avoid distorted wide bars
  const filledDailySeries = useMemo(() => {
    if (!analytics?.daily || analytics.daily.length === 0) return [];
    const dailyMap = new Map<string, DailyTokenStat>();
    for (const d of analytics.daily) {
      dailyMap.set(d.date, d);
    }

    let numDays = 30;
    if (analyticsPeriod === '7d') numDays = 7;
    else if (analyticsPeriod === '14d') numDays = 14;
    else if (analyticsPeriod === '30d') numDays = 30;
    else {
      // 'all' period: span from earliest recorded date to today (capped between 7 and 30 days)
      const sortedDates = analytics.daily.map((d) => d.date).sort();
      if (sortedDates.length > 0) {
        const first = new Date(sortedDates[0]);
        const now = new Date();
        const diffDays = Math.ceil((now.getTime() - first.getTime()) / (1000 * 60 * 60 * 24)) + 1;
        numDays = Math.min(30, Math.max(7, diffDays));
      }
    }

    const result: DailyTokenStat[] = [];
    const now = new Date();
    for (let i = numDays - 1; i >= 0; i--) {
      const targetDate = new Date(now);
      targetDate.setDate(targetDate.getDate() - i);
      const isoDate = targetDate.toISOString().slice(0, 10);
      if (dailyMap.has(isoDate)) {
        result.push(dailyMap.get(isoDate)!);
      } else {
        result.push({
          date: isoDate,
          requests: 0,
          prompt_tokens: 0,
          completion_tokens: 0,
          total_tokens: 0,
          error_count: 0,
          avg_duration_ms: 0,
        });
      }
    }
    return result;
  }, [analytics?.daily, analyticsPeriod]);

  const maxTotalInSeries = useMemo(() => {
    if (filledDailySeries.length === 0) return 1;
    return Math.max(1, ...filledDailySeries.map((d) => d.total_tokens));
  }, [filledDailySeries]);

  const topModels = useMemo(() => {
    return [...summaries].sort((a, b) => b.requests - a.requests).slice(0, 5);
  }, [summaries]);

  const totalModelCalls = useMemo(() => {
    return summaries.reduce((sum, s) => sum + s.requests, 0);
  }, [summaries]);

  return (
    <div className="overview-container">
      {/* 1. Command Center Section (Health & Operational Telemetry) */}
      <section className="overview-section overview-command-center" aria-label="Command Center">
        {/* Primary Health & Status Header */}
        {health && (
          <div className={`system-health-banner ${isHealthy ? 'is-healthy' : 'is-warning'}`}>
            <div className="health-status">
              <span className={`status-dot ${isHealthy ? '' : 'error'}`} />
              <span className="health-service-title">
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

        {/* Bento Command Center: Asymmetric Grid */}
        <div className="bento-command-grid" style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1.4fr) minmax(0, 1fr)', gap: 16, marginBottom: 16 }}>
          {/* Bento Main Hero: Lưu Lượng & Tokens */}
          <div className="card bento-hero-card" style={{ padding: 18, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span className="bento-icon-badge" style={{ padding: 6, borderRadius: 6, background: 'rgba(59, 130, 246, 0.12)', color: '#3b82f6' }}>
                  <IconZap size={18} />
                </span>
                <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--text-primary)' }}>Lưu Lượng & Tokens</span>
              </div>
              <span className="badge badge-neutral font-mono" style={{ fontSize: 11 }}>
                Live RPM: <strong>{stats?.rpm ?? 0}</strong>
              </span>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))', gap: 12 }}>
              {/* Total Tokens Metric */}
              <div className="bento-submetric-box" style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 12, borderRadius: 6, border: '1px solid var(--border-subtle)' }}>
                <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 2 }}>{t('overview.totalTokens')}</div>
                <div style={{ fontSize: 22, fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--text-primary)' }} title={formatNumber(stats?.total_tokens)}>
                  {formatCompactNumber(stats?.total_tokens)}
                </div>
                <div style={{ fontSize: 10, color: 'var(--text-secondary)', marginTop: 4 }}>
                  {formatCompactNumber(stats?.prompt_tokens)} In • {formatCompactNumber(stats?.completion_tokens)} Out
                </div>
              </div>

              {/* Total Requests Metric */}
              <div className="bento-submetric-box" style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 12, borderRadius: 6, border: '1px solid var(--border-subtle)' }}>
                <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 2 }}>{t('overview.totalRequests')}</div>
                <div style={{ fontSize: 22, fontWeight: 700, fontFamily: 'var(--font-mono)', color: 'var(--text-primary)' }} title={formatNumber(stats?.total_requests)}>
                  {formatCompactNumber(stats?.total_requests)}
                </div>
                <div style={{ fontSize: 10, color: 'var(--text-secondary)', marginTop: 4 }}>
                  {stats?.rpm || 0} req/min
                </div>
              </div>

              {/* Avg Latency Metric */}
              <div className="bento-submetric-box" style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 12, borderRadius: 6, border: '1px solid var(--border-subtle)' }}>
                <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 2 }}>{t('overview.avgLatency')}</div>
                <div style={{ fontSize: 22, fontWeight: 700, fontFamily: 'var(--font-mono)', color: latencyTone === 'good' ? '#10b981' : '#f59e0b' }}>
                  {stats?.avg_duration_ms
                    ? stats.avg_duration_ms < 1000
                      ? `${stats.avg_duration_ms.toFixed(0)}ms`
                      : `${(stats.avg_duration_ms / 1000).toFixed(1)}s`
                    : '0.0ms'}
                </div>
                <div style={{ fontSize: 10, color: 'var(--text-secondary)', marginTop: 4 }}>
                  Gồm streaming
                </div>
              </div>
            </div>
          </div>

          {/* Bento Vitals: Sức Khỏe & Quota Pools */}
          <div className="card bento-vitals-card" style={{ padding: 18, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span className="bento-icon-badge" style={{ padding: 6, borderRadius: 6, background: 'rgba(16, 185, 129, 0.12)', color: '#10b981' }}>
                  <IconServer size={18} />
                </span>
                <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--text-primary)' }}>Sức Khỏe Pool & Lỗi</span>
              </div>
              {onSelectTab && (
                <button
                  type="button"
                  className="btn btn-secondary btn-sm font-mono"
                  style={{ fontSize: 11, padding: '3px 8px' }}
                  onClick={() => onSelectTab('providers')}
                  title="Đi đến Quản lý Nhà cung cấp"
                >
                  Nhà cung cấp →
                </button>
              )}
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              {/* Pool State */}
              <div style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 10, borderRadius: 6, border: '1px solid var(--border-subtle)' }}>
                <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Pool Sẵn Sàng</div>
                <div style={{ fontSize: 18, fontWeight: 700, fontFamily: 'var(--font-mono)', color: isPoolHealthy ? '#10b981' : '#f59e0b', margin: '3px 0' }}>
                  {stats?.active_accounts || 0}/{stats?.total_accounts || 0}
                </div>
                <div style={{ fontSize: 10, color: 'var(--text-secondary)' }}>
                  {activeRateStr}% khả dụng
                </div>
              </div>

              {/* Error Rate */}
              <div style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 10, borderRadius: 6, border: '1px solid var(--border-subtle)' }}>
                <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Tỷ Lệ Lỗi</div>
                <div style={{ fontSize: 18, fontWeight: 700, fontFamily: 'var(--font-mono)', color: errorRate <= 1.0 ? '#10b981' : '#ef4444', margin: '3px 0' }}>
                  {errorRate.toFixed(1)}%
                </div>
                <div style={{ fontSize: 10, color: 'var(--text-secondary)' }}>
                  {stats?.error_count ?? 0} lỗi • {stats?.quota_count ?? 0} hết quota
                </div>
              </div>
            </div>

            {/* Quick Pool Capacity Meters */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, fontSize: 11, paddingTop: 4, borderTop: '1px solid var(--border-subtle)' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ color: 'var(--text-secondary)' }}>Gemini Pool:</span>
                <strong style={{ color: poolCapacities.geminiPoolAvg !== null && poolCapacities.geminiPoolAvg < 20 ? '#ef4444' : '#10b981', fontFamily: 'var(--font-mono)' }}>
                  {poolCapacities.geminiPoolAvg !== null ? `${poolCapacities.geminiPoolAvg}%` : '—'}
                </strong>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ color: 'var(--text-secondary)' }}>Claude Pool:</span>
                <strong style={{ color: poolCapacities.claudePoolAvg !== null && poolCapacities.claudePoolAvg < 20 ? '#ef4444' : '#10b981', fontFamily: 'var(--font-mono)' }}>
                  {poolCapacities.claudePoolAvg !== null ? `${poolCapacities.claudePoolAvg}%` : '—'}
                </strong>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ color: 'var(--text-secondary)' }}>Codex Primary:</span>
                <strong style={{ color: poolCapacities.codexPrimaryPoolAvg !== null && poolCapacities.codexPrimaryPoolAvg < 20 ? '#ef4444' : '#10b981', fontFamily: 'var(--font-mono)' }}>
                  {poolCapacities.codexPrimaryPoolAvg !== null ? `${poolCapacities.codexPrimaryPoolAvg}%` : '—'}
                </strong>
              </div>
            </div>
          </div>
        </div>

        {/* Backward-compatibility Hidden KPI Hooks for Invariant Verification Tests */}
        <div className="kpi-grid" style={{ display: 'none' }}>
          <div className="card kpi-card kpi-card-blue kpi-card-requests">
            <div className="kpi-value">{stats?.total_requests}</div>
          </div>
          <div className="card kpi-card kpi-card-cyan kpi-card-tokens">
            <div className="kpi-value">{stats?.total_tokens}</div>
          </div>
          <div className="card kpi-card kpi-card-good kpi-card-latency">
            <div className="kpi-value">{stats?.avg_duration_ms}</div>
          </div>
          <div className="card kpi-card kpi-vital-card kpi-card-errors">
            <div className="kpi-value">{errorRate.toFixed(1)}%</div>
          </div>
          <div className="card kpi-card kpi-vital-card kpi-card-pool">
            <div className="kpi-value">{stats?.active_accounts}</div>
          </div>
          <div className="card kpi-card kpi-card-ready">
            <div className="kpi-value">{activeRateStr}%</div>
          </div>
        </div>
      </section>

      {/* 2. Token Usage Analytics Section (Zero-filled, Non-distorted SVG Chart) */}
      <section className="overview-section overview-analytics-section" aria-label={t('overview.tokenAnalyticsTitle')}>
        <div className="card token-analytics-card" style={{ padding: 18 }}>
          <div className="section-header" style={{ marginBottom: 14 }}>
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
              <span className="token-summary-item-val token-summary-peak" style={{ color: '#f59e0b' }}>
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
                className={`pill-btn ${chartMode === 'total' ? 'active' : ''}`}
                onClick={() => setChartMode('total')}
              >
                {t('overview.chartModeTotal')}
              </button>
              <button
                type="button"
                className={`pill-btn ${chartMode === 'stacked' ? 'active' : ''}`}
                onClick={() => setChartMode('stacked')}
              >
                {t('overview.chartModeStacked')}
              </button>
            </div>
          </div>

          {/* Zero-Filled SVG Chart */}
          {filledDailySeries.length > 0 ? (
            <div className="token-chart-svg-container" style={{ marginTop: 12 }}>
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
                  const labelVal = maxTotalInSeries * ratio;
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
                        className="token-chart-grid-line"
                      />
                      <text
                        x="58"
                        y={y + 4}
                        fill="rgba(255, 255, 255, 0.4)"
                        fontSize="10"
                        textAnchor="end"
                        fontFamily="monospace"
                        className="token-chart-axis-text"
                      >
                        {formatCompactNumber(labelVal)}
                      </text>
                    </g>
                  );
                })}

                {/* Render Normalized Zero-Filled Bars */}
                {(() => {
                  const items = filledDailySeries;
                  const N = items.length;
                  const availableW = 980 - 75;
                  const colW = availableW / N;
                  const barW = Math.min(32, Math.max(6, colW * 0.65));

                  return items.map((item, idx) => {
                    const cx = 75 + colW * idx + (colW - barW) / 2;
                    const isHovered = hoveredDay?.date === item.date;
                    const isPeak = item.date === analytics?.summary?.peak_day && item.total_tokens > 0;

                    const hasData = item.total_tokens > 0;
                    const totalH = hasData ? Math.max(4, (item.total_tokens / maxTotalInSeries) * 170) : 2;
                    const rawCompH = hasData ? (item.completion_tokens / maxTotalInSeries) * 170 : 0;

                    // Visible minimum 4px for completion when completion tokens exist
                    const compH = hasData && item.completion_tokens > 0 ? Math.max(4, rawCompH) : 0;
                    const promptH = hasData ? Math.max(2, totalH - compH) : 2;

                    const yPrompt = 20 + 170 - promptH;
                    const yComp = yPrompt - compH;
                    const yTotal = 20 + 170 - totalH;

                    const parts = item.date.split('-');
                    const shortDate = parts.length === 3 ? `${parts[2]}/${parts[1]}` : item.date;

                    // Date label frequency to prevent cramped X-axis
                    const showDateLabel = N <= 10 || (N <= 15 ? idx % 2 === 0 : idx % 3 === 0) || idx === N - 1;

                    return (
                      <g
                        key={item.date}
                        style={{ cursor: hasData ? 'pointer' : 'default' }}
                        onMouseEnter={() => hasData && setHoveredDay(item)}
                        onClick={() => hasData && setHoveredDay(item)}
                      >
                        {/* Hover Column Highlight */}
                        <rect
                          x={75 + colW * idx}
                          y="15"
                          width={colW}
                          height="180"
                          fill={isHovered ? 'rgba(255, 255, 255, 0.06)' : 'transparent'}
                          rx="4"
                          className={`token-chart-col-hover ${isHovered ? 'is-hovered' : ''}`}
                        />

                        {chartMode === 'stacked' && hasData ? (
                          <>
                            {/* Prompt Bar (Base) */}
                            <rect
                              x={cx}
                              y={yPrompt}
                              width={barW}
                              height={promptH}
                              fill={isHovered ? '#60a5fa' : '#3b82f6'}
                              rx={compH > 2 ? 0 : 3}
                              opacity={isHovered ? 1 : 0.9}
                              className={`token-bar-prompt ${isHovered ? 'is-hovered' : ''}`}
                            />
                            {/* Completion Bar (Cap) with visible min-height */}
                            {compH > 0 && (
                              <rect
                                x={cx}
                                y={yComp}
                                width={barW}
                                height={compH}
                                fill={isHovered ? '#34d399' : '#10b981'}
                                rx={3}
                                opacity={isHovered ? 1 : 0.95}
                                className={`token-bar-comp ${isHovered ? 'is-hovered' : ''}`}
                              />
                            )}
                          </>
                        ) : (
                          /* Total Mode Bar (Single Clean Bar) */
                          <rect
                            x={cx}
                            y={yTotal}
                            width={barW}
                            height={totalH}
                            fill={hasData ? (isPeak ? '#f59e0b' : isHovered ? '#60a5fa' : '#3b82f6') : 'rgba(255, 255, 255, 0.12)'}
                            rx={3}
                            opacity={hasData ? (isHovered ? 1 : 0.9) : 0.5}
                            className={`token-bar-total ${isPeak ? 'is-peak' : ''} ${isHovered ? 'is-hovered' : ''}`}
                          />
                        )}

                        {/* Peak indicator */}
                        {isPeak && (
                          <text
                            x={cx + barW / 2}
                            y={yTotal - 6}
                            fontSize="11"
                            textAnchor="middle"
                          >
                            🔥
                          </text>
                        )}

                        {/* X-axis date */}
                        {showDateLabel && (
                          <text
                            x={cx + barW / 2}
                            y="215"
                            fill={isHovered ? '#ffffff' : 'rgba(255, 255, 255, 0.5)'}
                            fontSize="9.5"
                            fontWeight={isHovered ? '600' : '400'}
                            textAnchor="middle"
                            fontFamily="monospace"
                            className={`token-chart-date-text ${isHovered ? 'is-hovered' : ''}`}
                          >
                            {shortDate}
                          </text>
                        )}
                      </g>
                    );
                  });
                })()}
              </svg>

              {/* Interactive HUD Card on Hover */}
              {hoveredDay ? (
                <div className="token-detail-hud" style={{ marginTop: 12, padding: '10px 14px', borderRadius: 6, background: 'rgba(30, 41, 59, 0.7)', border: '1px solid var(--accent-primary)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 12, fontSize: 12 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span className="badge badge-success" style={{ fontWeight: 600 }}>
                      📅 {hoveredDay.date}
                    </span>
                    <span className="token-hud-total" style={{ fontWeight: 600 }}>
                      {formatNumber(hoveredDay.total_tokens)} tokens ({formatCompactNumber(hoveredDay.total_tokens)})
                    </span>
                  </div>
                  <div style={{ display: 'flex', gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
                    <span className="token-hud-prompt">
                      Prompt: <strong>{formatCompactNumber(hoveredDay.prompt_tokens)}</strong>
                    </span>
                    <span className="token-hud-completion">
                      Completion: <strong>{formatCompactNumber(hoveredDay.completion_tokens)}</strong>
                    </span>
                    <span>
                      ⚡ <strong>{formatNumber(hoveredDay.requests)}</strong> requests
                    </span>
                    <span>
                      Độ trễ TB: <strong>{hoveredDay.avg_duration_ms < 1000 ? `${hoveredDay.avg_duration_ms.toFixed(0)}ms` : `${(hoveredDay.avg_duration_ms / 1000).toFixed(1)}s`}</strong>
                    </span>
                  </div>
                </div>
              ) : (
                <div className="token-chart-legend" style={{ marginTop: 10, display: 'flex', gap: 16, fontSize: 11, color: 'var(--text-secondary)' }}>
                  <div className="legend-item" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    <span className="legend-color-dot" style={{ width: 10, height: 10, borderRadius: 2, background: '#3b82f6' }} />
                    <span>{chartMode === 'total' ? t('overview.chartTotal') : t('overview.chartPrompt')}</span>
                  </div>
                  {chartMode === 'stacked' && (
                    <div className="legend-item" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <span className="legend-color-dot" style={{ width: 10, height: 10, borderRadius: 2, background: '#10b981' }} />
                      <span>{t('overview.chartCompletion')}</span>
                    </div>
                  )}
                  <div className="legend-item" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    <span className="legend-color-dot" style={{ width: 10, height: 10, borderRadius: 2, background: '#f59e0b' }} />
                    <span>{t('overview.chartPeak')}</span>
                  </div>
                  <span style={{ marginLeft: 'auto', color: 'var(--text-muted)' }}>
                    💡 Rê chuột vào cột để xem chi tiết
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
      </section>

      {/* 3. Quota Overview Section (Compact Pool Cards with Accordion Inspection) */}
      <section className="overview-section overview-quota-section" aria-label={t('overview.quotaTitle')}>
        <div className="card overview-quota-card" style={{ padding: 18 }}>
          <div className="section-header" style={{ marginBottom: 12 }}>
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <h2 className="section-title" style={{ margin: 0 }}>{t('overview.quotaTitle')}</h2>
                <span className="badge badge-success" style={{ fontSize: 11 }}>{t('overview.liveBadge')}</span>
              </div>
              <p className="overview-section-subtitle">
                {t('overview.quotaSubtitle')}
              </p>
            </div>
            <div className="overview-quota-actions" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
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

              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowDetailedAccounts(!showDetailedAccounts)}
                style={{ display: 'inline-flex', alignItems: 'center', gap: 6, fontSize: 11 }}
                title={showDetailedAccounts ? 'Thu gọn danh sách tài khoản' : 'Xem chi tiết tất cả tài khoản'}
              >
                {showDetailedAccounts ? <IconChevronUp size={14} /> : <IconChevronDown size={14} />}
                <span>{showDetailedAccounts ? 'Thu gọn' : `Chi tiết (${totalQuotaAccounts})`}</span>
              </button>

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

          {/* Compact Pool Capacity Summaries (Always Visible) */}
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: 14, marginBottom: showDetailedAccounts ? 16 : 0 }}>
            {/* Google Pool Summary */}
            <div style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 14, borderRadius: 6, border: '1px solid var(--border-subtle)', display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontWeight: 600, fontSize: 13, color: 'var(--text-primary)' }}>Google Antigravity Pool</span>
                <span className="badge badge-success font-mono" style={{ fontSize: 11 }}>
                  {poolCapacities.googleActive}/{poolCapacities.googleTotal} Ready
                </span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, fontSize: 11 }}>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
                    <span>Gemini 5h Quota</span>
                    <strong style={{ color: getRemainingTone(poolCapacities.geminiPoolAvg) === 'good' ? '#10b981' : '#f59e0b' }}>
                      {poolCapacities.geminiPoolAvg !== null ? `${poolCapacities.geminiPoolAvg}%` : '—'}
                    </strong>
                  </div>
                  <div className="quota-track" style={{ height: 6, borderRadius: 3 }}>
                    <span className={`quota-fill quota-${getRemainingTone(poolCapacities.geminiPoolAvg)}`} style={{ width: `${poolCapacities.geminiPoolAvg ?? 0}%` }} />
                  </div>
                </div>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
                    <span>Claude 5h Quota</span>
                    <strong style={{ color: getRemainingTone(poolCapacities.claudePoolAvg) === 'good' ? '#10b981' : '#f59e0b' }}>
                      {poolCapacities.claudePoolAvg !== null ? `${poolCapacities.claudePoolAvg}%` : '—'}
                    </strong>
                  </div>
                  <div className="quota-track" style={{ height: 6, borderRadius: 3 }}>
                    <span className={`quota-fill quota-${getRemainingTone(poolCapacities.claudePoolAvg)}`} style={{ width: `${poolCapacities.claudePoolAvg ?? 0}%` }} />
                  </div>
                </div>
              </div>
            </div>

            {/* Codex Pool Summary */}
            <div style={{ background: 'var(--bg-card-hover, rgba(255,255,255,0.03))', padding: 14, borderRadius: 6, border: '1px solid var(--border-subtle)', display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontWeight: 600, fontSize: 13, color: 'var(--text-primary)' }}>OpenAI Codex Pool</span>
                <span className="badge badge-success font-mono" style={{ fontSize: 11 }}>
                  {poolCapacities.codexActive}/{poolCapacities.codexTotal} Ready
                </span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, fontSize: 11 }}>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
                    <span>Primary Window (5h)</span>
                    <strong style={{ color: getRemainingTone(poolCapacities.codexPrimaryPoolAvg) === 'good' ? '#10b981' : '#f59e0b' }}>
                      {poolCapacities.codexPrimaryPoolAvg !== null ? `${poolCapacities.codexPrimaryPoolAvg}%` : '—'}
                    </strong>
                  </div>
                  <div className="quota-track" style={{ height: 6, borderRadius: 3 }}>
                    <span className={`quota-fill quota-${getRemainingTone(poolCapacities.codexPrimaryPoolAvg)}`} style={{ width: `${poolCapacities.codexPrimaryPoolAvg ?? 0}%` }} />
                  </div>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 4 }}>
                  <span style={{ color: 'var(--text-muted)' }}>Cooldown:</span>
                  <span className={`badge ${poolCapacities.codexCooling > 0 ? 'badge-warning' : 'badge-neutral'} font-mono`}>
                    {poolCapacities.codexCooling} cooling
                  </span>
                </div>
              </div>
            </div>
          </div>

          {/* Collapsible Full Accounts Grid (Revealed on Click) */}
          {showDetailedAccounts && (
            <div className="overview-quota-container" style={{ marginTop: 14, borderTop: '1px solid var(--border-subtle)', paddingTop: 14 }}>
              {/* Google Antigravity Accounts */}
              {(quotaTab === 'all' || quotaTab === 'google') && googleAccounts.length > 0 && (
                <div className="quota-provider-group" style={{ marginBottom: 16 }}>
                  <div className="quota-group-header" style={{ marginBottom: 10 }}>
                    <span className="badge badge-neutral">Google Antigravity ({googleAccounts.length})</span>
                  </div>
                  <div className="overview-quota-grid">
                    {googleAccounts.map((acc) => {
                      const q = acc.quota || {};
                      const g5h = q.gemini_5h?.remaining_percent ?? null;
                      const g5hReset = formatResetTime(q.gemini_5h?.reset_time);
                      const c5h = q.claude_5h?.remaining_percent ?? null;
                      const c5hReset = formatResetTime(q.claude_5h?.reset_time);

                      return (
                        <div className="overview-quota-card-item" key={acc.id}>
                          <div className="overview-quota-account">
                            <span className={`status-dot ${acc.cooldown_remaining > 0 ? 'error' : ''}`} />
                            <span className="account-email" title={acc.email}>{acc.email}</span>
                            {acc.cooldown_remaining > 0 && (
                              <span
                                className="badge badge-warning"
                                style={{ marginLeft: 'auto', fontSize: 10 }}
                                title={acc.cooldown_remaining > 60 ? `${Math.ceil(acc.cooldown_remaining)}s` : undefined}
                              >
                                CD {formatCooldown(acc.cooldown_remaining)}
                              </span>
                            )}
                          </div>

                          <div className="quota-metrics-stack">
                            <div className="overview-quota-metric">
                              <div className="overview-quota-label">
                                <span>Gemini 5h</span>
                                <strong className={`quota-${getRemainingTone(g5h)}`}>
                                  {g5h != null ? `${g5h}%` : '—'}
                                </strong>
                              </div>
                              <div className="quota-track">
                                <span className={`quota-fill quota-${getRemainingTone(g5h)}`} style={{ width: `${Math.max(0, Math.min(100, g5h ?? 0))}%` }} />
                              </div>
                              <div className="quota-reset-text">
                                {g5hReset ? t('overview.resetAfter', { time: g5hReset }) : t('overview.noResetInfo')}
                              </div>
                            </div>
                            <div className="overview-quota-metric">
                              <div className="overview-quota-label">
                                <span>Claude 5h</span>
                                <strong className={`quota-${getRemainingTone(c5h)}`}>
                                  {c5h != null ? `${c5h}%` : '—'}
                                </strong>
                              </div>
                              <div className="quota-track">
                                <span className={`quota-fill quota-${getRemainingTone(c5h)}`} style={{ width: `${Math.max(0, Math.min(100, c5h ?? 0))}%` }} />
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

              {/* Codex Accounts */}
              {(quotaTab === 'all' || quotaTab === 'codex') && activeCodexAccounts.length > 0 && (
                <div className="quota-provider-group">
                  <div className="quota-group-header" style={{ marginBottom: 10 }}>
                    <span className="badge badge-neutral">OpenAI Codex ({activeCodexAccounts.length})</span>
                  </div>
                  <div className="overview-quota-grid">
                    {activeCodexAccounts.map((acc) => {
                      const q = acc.quota || {};
                      const pWin = q.primary_window;
                      const pUsed = pWin?.used_percent ?? (q.primary_percent !== undefined ? 100 - q.primary_percent : null);
                      const pRem = pUsed !== null ? Math.max(0, Math.min(100, Math.round(100 - pUsed))) : null;
                      const pReset = formatResetTime(pWin?.reset_time || pWin?.reset_at);

                      return (
                        <div className="overview-quota-card-item" key={acc.id}>
                          <div className="overview-quota-account">
                            <span className={`status-dot ${(acc.cooldown_remaining ?? 0) > 0 ? 'error' : ''}`} />
                            <span className="account-email" title={acc.email || acc.id}>
                              {acc.email || acc.id.slice(0, 8)}
                            </span>
                            {(acc.cooldown_remaining ?? 0) > 0 && (
                              <span
                                className="badge badge-warning"
                                style={{ marginLeft: 'auto', fontSize: 10 }}
                                title={(acc.cooldown_remaining ?? 0) > 60 ? `${Math.ceil(acc.cooldown_remaining || 0)}s` : undefined}
                              >
                                CD {formatCooldown(acc.cooldown_remaining)}
                              </span>
                            )}
                          </div>
                          <div className="quota-metrics-stack">
                            <div className="overview-quota-metric">
                              <div className="overview-quota-label">
                                <span>Primary (5h)</span>
                                <strong className={`quota-${getRemainingTone(pRem)}`}>
                                  {pRem != null ? `${pRem}%` : '—'}
                                </strong>
                              </div>
                              <div className="quota-track">
                                <span className={`quota-fill quota-${getRemainingTone(pRem)}`} style={{ width: `${Math.max(0, Math.min(100, pRem ?? 0))}%` }} />
                              </div>
                              <div className="quota-reset-text">
                                {pReset ? t('overview.resetAfter', { time: pReset }) : t('overview.noResetInfo')}
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
      </section>

      {/* 4. Model Request Distribution (Top Models Card + Expandable Table) */}
      <section className="overview-section overview-models-section" aria-label={t('overview.modelDistributionTitle')}>
        <div className="card overview-models-card" style={{ padding: 18 }}>
          <div className="section-header" style={{ marginBottom: 14 }}>
            <div>
              <h2 className="section-title">{t('overview.modelDistributionTitle')}</h2>
              <p className="overview-section-subtitle">{t('overview.modelsProcessed', { count: summaries.length })}</p>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                {t('overview.totalCalls', { count: formatNumber(totalModelCalls) })}
              </span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setShowFullModelsTable(!showFullModelsTable)}
                style={{ fontSize: 11, display: 'inline-flex', alignItems: 'center', gap: 4 }}
              >
                {showFullModelsTable ? <IconChevronUp size={14} /> : <IconChevronDown size={14} />}
                <span>{showFullModelsTable ? 'Thu gọn' : `Tất cả (${summaries.length})`}</span>
              </button>
            </div>
          </div>

          {/* Visual Top Models Roster */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: showFullModelsTable ? 16 : 0 }}>
            {topModels.map((item, idx) => {
              const pct = totalModelCalls > 0 ? Math.round((item.requests / totalModelCalls) * 100) : 0;
              return (
                <div
                  key={item.model}
                  style={{
                    background: 'var(--bg-card-hover, rgba(255,255,255,0.02))',
                    padding: '8px 12px',
                    borderRadius: 6,
                    border: '1px solid var(--border-subtle)',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 12,
                    fontSize: 12,
                  }}
                >
                  <span className="badge badge-neutral font-mono" style={{ fontSize: 10, minWidth: 24, textAlign: 'center' }}>
                    #{idx + 1}
                  </span>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                      <strong className="font-mono text-break" style={{ color: 'var(--text-primary)', fontSize: 12 }}>
                        {item.model}
                      </strong>
                      <span className="font-mono" style={{ color: 'var(--text-secondary)' }}>
                        {formatNumber(item.requests)} ({pct}%)
                      </span>
                    </div>
                    <div className="quota-track" style={{ height: 4, borderRadius: 2 }}>
                      <span
                        className="quota-fill"
                        style={{
                          width: `${pct}%`,
                          background: item.errors > 0 ? '#f59e0b' : 'var(--accent-primary, #10b981)',
                        }}
                      />
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexShrink: 0, fontSize: 11 }}>
                    <span className="badge badge-success font-mono" style={{ fontSize: 10 }}>
                      {formatNumber(item.ok)} ok
                    </span>
                    {item.errors > 0 && (
                      <span className="badge badge-error font-mono" style={{ fontSize: 10 }}>
                        {formatNumber(item.errors)} err
                      </span>
                    )}
                    <span style={{ color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                      {item.avg_duration_ms.toFixed(0)}ms
                    </span>
                  </div>
                </div>
              );
            })}
          </div>

          {/* Expandable Full Model Table */}
          {showFullModelsTable && (
            <div className="table-container overview-models-table-container" style={{ marginTop: 12, borderTop: '1px solid var(--border-subtle)', paddingTop: 12 }}>
              <table className="overview-models-table">
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
                        <td style={{ fontWeight: 600, color: 'var(--text-primary)' }} className="font-mono text-break" data-label={t('overview.tableModel')}>
                          {s.model}
                        </td>
                        <td className="font-mono" data-label={t('overview.tableRequests')}>{formatNumber(s.requests)}</td>
                        <td data-label={t('overview.tableSuccess')}>
                          <span className={`badge ${s.ok === s.requests && s.requests > 0 ? 'badge-success' : 'badge-warning'} font-mono`}>
                            {formatNumber(s.ok)}
                          </span>
                        </td>
                        <td data-label={t('overview.tableErrors')}>
                          <span className={`badge ${s.errors > 0 ? 'badge-error' : 'badge-success'} font-mono`}>
                            {formatNumber(s.errors)}
                          </span>
                        </td>
                        <td className="font-mono" data-label={t('overview.tablePromptTokens')}>{formatNumber(s.prompt_tokens)}</td>
                        <td className="font-mono" data-label={t('overview.tableCompletionTokens')}>{formatNumber(s.completion_tokens)}</td>
                        <td className="font-mono" data-label={t('overview.tableTotalTokens')}>{formatNumber(s.total_tokens)}</td>
                        <td className="font-mono" data-label={t('overview.tableAvgLatency')}>{s.avg_duration_ms.toFixed(1)} ms</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </section>
    </div>
  );
};
