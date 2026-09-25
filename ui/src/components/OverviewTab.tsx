import React, { useState } from 'react';
import {
  AccountResponse,
  AdminStats,
  CodexAccountRecord,
  HealthInfo,
  ModelRequestSummary,
} from '../types';
import { TabType } from './Sidebar';
import { IconActivity, IconAlertCircle, IconRefresh } from '../icons';

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
  const [quotaTab, setQuotaTab] = useState<'all' | 'google' | 'codex'>('all');

  if (loading && !stats) {
    return (
      <div className="state-container">
        <div className="spinner" />
        <p>Đang tải dữ liệu tổng quan...</p>
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

  const formatNumber = (n: number | undefined) => (n ?? 0).toLocaleString('vi-VN');
  const formatCompactNumber = (n: number | undefined) => {
    const value = n ?? 0;
    const units: Array<[number, string]> = [
      [1_000_000_000, 'Tỷ'],
      [1_000_000, 'Tr'],
      [1_000, 'N'],
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

  // Format ISO time to relative countdown string
  const formatResetTime = (isoString?: string) => {
    if (!isoString) return null;
    try {
      const d = new Date(isoString);
      if (isNaN(d.getTime())) return null;
      const diffSec = Math.round((d.getTime() - Date.now()) / 1000);
      if (diffSec <= 0) return 'sắp reset';
      if (diffSec < 60) return `${diffSec}s`;
      if (diffSec < 3600) return `${Math.round(diffSec / 60)}m`;
      if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ${Math.round((diffSec % 3600) / 60)}m`;
      const days = Math.floor(diffSec / 86400);
      const hours = Math.round((diffSec % 86400) / 3600);
      return `${days}d ${hours}h`;
    } catch {
      return null;
    }
  };

  const formatSeconds = (sec?: number) => {
    if (sec == null || sec <= 0) return 'sắp reset';
    if (sec < 60) return `${Math.round(sec)}s`;
    if (sec < 3600) return `${Math.round(sec / 60)}m`;
    if (sec < 86400) return `${Math.floor(sec / 3600)}h ${Math.round((sec % 3600) / 60)}m`;
    const days = Math.floor(sec / 86400);
    const hours = Math.round((sec % 86400) / 3600);
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
              {isHealthy ? 'Trực tuyến' : 'Cần kiểm tra'}
            </span>
          </div>
          <div className="health-meta">
            <span>Cổng: {health.port}</span>
            <span>Upstream: <strong>Antigravity + Codex</strong></span>
            <span>
              Pool: <strong>{stats?.active_accounts ?? 0}/{stats?.total_accounts ?? 0} khả dụng</strong>
            </span>
          </div>
        </div>
      )}

      {/* Symmetrical 3x2 KPI Grid */}
      <div className="kpi-grid">
        {/* Row 1, Col 1: Tổng Yêu Cầu */}
        <div className="card kpi-card kpi-card-blue">
          <div className="kpi-label">
            <span>Tổng Yêu Cầu</span>
            <IconActivity size={16} />
          </div>
          <div className="kpi-value" title={formatNumber(stats?.total_requests)}>{formatCompactNumber(stats?.total_requests)}</div>
          <div className="kpi-sub">
            RPM hiện tại: <strong>{stats?.rpm || 0} req/min</strong>
          </div>
        </div>

        {/* Row 1, Col 2: Tổng Token */}
        <div className="card kpi-card kpi-card-cyan">
          <div className="kpi-label">
            <span>Tổng Token</span>
            <span className="badge badge-neutral">In / Out</span>
          </div>
          <div className="kpi-value" title={formatNumber(stats?.total_tokens)}>{formatCompactNumber(stats?.total_tokens)}</div>
          <div className="kpi-sub" title={`Prompt: ${formatNumber(stats?.prompt_tokens)} | Completion: ${formatNumber(stats?.completion_tokens)}`}>
            In: {formatCompactNumber(stats?.prompt_tokens)} · Out: {formatCompactNumber(stats?.completion_tokens)}
          </div>
        </div>

        {/* Row 1, Col 3: Độ Trễ TB */}
        <div className={`card kpi-card kpi-card-${latencyTone}`}>
          <div className="kpi-label">
            <span>Độ Trễ TB</span>
            <span className="badge badge-neutral">Upstream</span>
          </div>
          <div className="kpi-value">
            {stats?.avg_duration_ms ? stats.avg_duration_ms.toFixed(1) : '0.0'}
            <span style={{ fontSize: 14, fontWeight: 400, marginLeft: 4 }}>ms</span>
          </div>
          <div className="kpi-sub">
            {stats?.avg_duration_ms && stats.avg_duration_ms < 1000
              ? '⚡ Phản hồi rất nhanh'
              : 'Ổn định theo mô hình'}
          </div>
        </div>

        {/* Row 2, Col 1: Tỷ Lệ Lỗi */}
        <div className={`card kpi-card ${errorRate === 0 ? 'kpi-card-good' : 'kpi-card-bad'}`}>
          <div className="kpi-label">
            <span>Tỷ Lệ Lỗi</span>
            <span className={`badge ${errorRate === 0 ? 'badge-success' : 'badge-error'}`}>
              {(errorRate * 100).toFixed(1)}%
            </span>
          </div>
          <div className="kpi-value" title={formatNumber(stats?.error_count)}>{formatCompactNumber(stats?.error_count)}</div>
          <div className="kpi-sub">Yêu cầu lỗi hoặc gián đoạn</div>
        </div>

        {/* Row 2, Col 2: Pool Khả Dụng */}
        <div className={`card kpi-card ${isPoolHealthy ? 'kpi-card-good' : 'kpi-card-warn'}`}>
          <div className="kpi-label">
            <span>Pool Khả Dụng</span>
            <span className={`badge ${isPoolHealthy ? 'badge-success' : 'badge-warning'}`}>Pool</span>
          </div>
          <div className="kpi-value">
            {stats?.active_accounts || 0} / {stats?.total_accounts || 0}
          </div>
          <div className="kpi-sub">
            Cooldown: <strong>{stats?.cooldown_accounts || 0}</strong> tài khoản
          </div>
        </div>

        {/* Row 2, Col 3: Tỷ Lệ Sẵn Sàng */}
        <div className={`card kpi-card ${activeRate >= 80 ? 'kpi-card-good' : 'kpi-card-warn'}`}>
          <div className="kpi-label">
            <span>Tỷ Lệ Sẵn Sàng</span>
            <span>%</span>
          </div>
          <div className="kpi-value">
            {stats?.active_rate != null ? stats.active_rate.toFixed(1) : '100.0'}%
          </div>
          <div className="kpi-sub">Mức độ sẵn sàng phân phối tải</div>
        </div>
      </div>

      {/* Quota Overview Section */}
      <div className="card overview-quota-card" style={{ marginBottom: 24 }}>
        <div className="section-header">
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <h2 className="section-title" style={{ margin: 0 }}>Quota Tài Khoản</h2>
              <span className="badge badge-success" style={{ fontSize: 11 }}>Trực tiếp</span>
            </div>
            <p className="overview-section-subtitle">
              Hạn mức thời gian thực của các tài khoản Google Antigravity & OpenAI Codex
            </p>
          </div>
          <div className="overview-quota-actions">
            <div className="overview-tab-pills">
              <button
                type="button"
                className={`pill-btn ${quotaTab === 'all' ? 'active' : ''}`}
                onClick={() => setQuotaTab('all')}
              >
                Tất cả ({totalQuotaAccounts})
              </button>
              <button
                type="button"
                className={`pill-btn ${quotaTab === 'google' ? 'active' : ''}`}
                onClick={() => setQuotaTab('google')}
              >
                Google Antigravity ({googleAccounts.length})
              </button>
              <button
                type="button"
                className={`pill-btn ${quotaTab === 'codex' ? 'active' : ''}`}
                onClick={() => setQuotaTab('codex')}
              >
                OpenAI Codex ({activeCodexAccounts.length})
              </button>
            </div>
            {onRefresh && (
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={onRefresh}
                disabled={refreshing}
                title="Làm mới quota và số liệu"
                style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}
              >
                <IconRefresh size={14} className={refreshing ? 'spinning' : ''} />
                <span>Làm mới</span>
              </button>
            )}
          </div>
        </div>

        {totalQuotaAccounts === 0 ? (
          <div className="overview-quota-empty">
            Chưa có tài khoản nào được cấu hình trong pool.
          </div>
        ) : (
          <div className="overview-quota-container">
            {/* Google Antigravity Accounts */}
            {(quotaTab === 'all' || quotaTab === 'google') && googleAccounts.length > 0 && (
              <div className="quota-provider-group">
                <div className="quota-group-header">
                  <span className="badge badge-neutral">Google Antigravity ({googleAccounts.length})</span>
                  <small style={{ color: 'var(--text-muted)' }}>Gemini & Claude Models</small>
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
                                {g5h != null ? `${g5h}% còn lại` : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(g5h)}`}
                                style={{ width: `${Math.max(0, Math.min(100, g5h ?? 0))}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {g5hReset ? `Reset sau ${g5hReset}` : 'Chưa có thông tin reset'}
                            </div>
                          </div>

                          {/* Gemini Weekly */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>Gemini Tuần</span>
                              <strong className={`quota-${getRemainingTone(gWeekly)}`}>
                                {gWeekly != null ? `${gWeekly}% còn lại` : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(gWeekly)}`}
                                style={{ width: `${Math.max(0, Math.min(100, gWeekly ?? 0))}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {gWeeklyReset ? `Reset sau ${gWeeklyReset}` : 'Chưa có thông tin reset'}
                            </div>
                          </div>

                          {/* Claude 5h */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>Claude 5h</span>
                              <strong className={`quota-${getRemainingTone(c5h)}`}>
                                {c5h != null ? `${c5h}% còn lại` : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(c5h)}`}
                                style={{ width: `${Math.max(0, Math.min(100, c5h ?? 0))}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {c5hReset ? `Reset sau ${c5hReset}` : 'Chưa có thông tin reset'}
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
                    const pUsed = q.primary_window?.used_percent ?? null;
                    const pRem = pUsed != null ? Math.max(0, Math.min(100, Math.round(100 - pUsed))) : null;
                    const pReset = formatSeconds(q.primary_window?.reset_after_seconds);

                    const wUsed = q.weekly_window?.used_percent ?? null;
                    const wRem = wUsed != null ? Math.max(0, Math.min(100, Math.round(100 - wUsed))) : null;
                    const wReset = formatSeconds(q.weekly_window?.reset_after_seconds);

                    return (
                      <div className="overview-quota-card-item" key={acc.id}>
                        <div className="overview-quota-account">
                          <span className={`status-dot ${(acc.cooldown_remaining ?? 0) > 0 ? 'error' : ''}`} />
                          <span className="account-email" title={acc.email || acc.id}>
                            {acc.email || `Codex Account (${acc.id.slice(0, 8)})`}
                          </span>
                          {(acc.cooldown_remaining ?? 0) > 0 && (
                            <span className="badge badge-warning" style={{ marginLeft: 'auto', fontSize: 10 }}>
                              CD {Math.round(acc.cooldown_remaining || 0)}s
                            </span>
                          )}
                        </div>

                        <div className="quota-metrics-stack">
                          {/* Primary 5h */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>Hạn mức 5h</span>
                              <strong className={`quota-${getRemainingTone(pRem)}`}>
                                {pRem != null ? `${pRem}% còn lại (${pUsed}% dùng)` : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(pRem)}`}
                                style={{ width: `${pRem ?? 0}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {pReset ? `Reset sau ${pReset}` : 'Chưa có thông tin reset'}
                            </div>
                          </div>

                          {/* Weekly */}
                          <div className="overview-quota-metric">
                            <div className="overview-quota-label">
                              <span>Hạn mức Tuần</span>
                              <strong className={`quota-${getRemainingTone(wRem)}`}>
                                {wRem != null ? `${wRem}% còn lại (${wUsed}% dùng)` : '—'}
                              </strong>
                            </div>
                            <div className="quota-track">
                              <span
                                className={`quota-fill quota-${getRemainingTone(wRem)}`}
                                style={{ width: `${wRem ?? 0}%` }}
                              />
                            </div>
                            <div className="quota-reset-text">
                              {wReset ? `Reset sau ${wReset}` : 'Chưa có thông tin reset'}
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
            <h2 className="section-title">Phân Bổ Yêu Cầu Theo Model</h2>
            <p className="overview-section-subtitle">{summaries.length} mô hình AI đã xử lý request</p>
          </div>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            Tổng {summaries.reduce((sum, s) => sum + s.requests, 0).toLocaleString('vi-VN')} lượt gọi
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
                      title={`${item.model}: ${item.requests} yêu cầu (${item.ok} ok, ${item.errors} lỗi)`}
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
                <th>Model</th>
                <th>Tổng Yêu Cầu</th>
                <th>Thành Công</th>
                <th>Lỗi</th>
                <th>Prompt Tokens</th>
                <th>Completion Tokens</th>
                <th>Tổng Token</th>
                <th>Độ Trễ TB</th>
              </tr>
            </thead>
            <tbody>
              {summaries.length === 0 ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: 'center', padding: 32 }}>
                    Chưa có nhật ký yêu cầu nào được ghi nhận.
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
