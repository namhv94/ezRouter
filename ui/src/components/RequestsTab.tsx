import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '../i18n';
import { RequestLogItem, RequestsResponse, AdminStats, ActiveRequestItem } from '../types';
import { api } from '../api';
import {
  IconAlertCircle,
  IconCheck,
  IconX,
  IconZap,
  IconRefresh,
  IconPause,
  IconPlay,
  IconCopy,
  IconActivity,
} from '../icons';

const OPENROUTER_PRICING: Record<string, { prompt: number; completion: number }> = {
  'deepseek/deepseek-chat': { prompt: 0.14, completion: 0.28 },
  'deepseek/deepseek-r1': { prompt: 0.55, completion: 2.19 },
  'meta-llama/llama-3.3-70b-instruct': { prompt: 0.13, completion: 0.40 },
  'openai/gpt-4o': { prompt: 2.50, completion: 10.00 },
  'openai/gpt-4o-mini': { prompt: 0.15, completion: 0.60 },
  'google/gemini-2.5-pro': { prompt: 1.25, completion: 5.00 },
  'google/gemini-2.5-flash': { prompt: 0.075, completion: 0.30 },
  'qwen/qwen-2.5-coder-32b-instruct': { prompt: 0.07, completion: 0.16 },
  'anthropic/claude-3.5-sonnet': { prompt: 3.00, completion: 15.00 },
  'anthropic/claude-3.7-sonnet': { prompt: 3.00, completion: 15.00 },
};

const estimateRequestCost = (model: string, promptTokens: number, completionTokens: number): number | null => {
  if (!model || !model.startsWith('openrouter/')) return null;
  const subModel = model.replace('openrouter/', '');
  const pricing = OPENROUTER_PRICING[subModel];
  if (pricing) {
    return ((promptTokens || 0) * pricing.prompt + (completionTokens || 0) * pricing.completion) / 1_000_000;
  }
  return ((promptTokens || 0) * 0.50 + (completionTokens || 0) * 1.50) / 1_000_000;
};

export const RequestsTab: React.FC = () => {
  const { locale, t } = useI18n();
  const [data, setData] = useState<RequestsResponse | null>(null);
  const [stats, setStats] = useState<AdminStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Active Requests (Live pipeline flow)
  const [activeRequests, setActiveRequests] = useState<ActiveRequestItem[]>([]);
  const [selectedActiveReq, setSelectedActiveReq] = useState<ActiveRequestItem | null>(null);
  const [copiedActiveId, setCopiedActiveId] = useState(false);

  // Filters
  const [modelFilter, setModelFilter] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [latencyPreset, setLatencyPreset] = useState<'all' | 'fast' | 'med' | 'slow'>('all');
  const [limit, setLimit] = useState(50);
  const [offset, setOffset] = useState(0);

  // Auto-refresh state
  const [autoRefreshSec, setAutoRefreshSec] = useState<number>(5);
  const [lastRefreshedAt, setLastRefreshedAt] = useState<Date>(new Date());

  // Modal inspection state
  const [inspectItem, setInspectItem] = useState<RequestLogItem | null>(null);
  const [inspectTab, setInspectTab] = useState<'overview' | 'error' | 'raw'>('overview');
  const [copiedRaw, setCopiedRaw] = useState(false);

  const loadRequests = useCallback(
    async (showLoadingSpinner = false) => {
      if (showLoadingSpinner) {
        setLoading(true);
      } else {
        setRefreshing(true);
      }
      setError(null);
      try {
        const [reqResp, statsResp] = await Promise.allSettled([
          api.getRequests({
            limit,
            offset,
            model: modelFilter.trim() || undefined,
            status: statusFilter.trim() || undefined,
          }),
          api.getStats(),
        ]);

        if (reqResp.status === 'fulfilled') {
          setData(reqResp.value);
        } else {
          throw reqResp.reason;
        }

        if (statsResp.status === 'fulfilled') {
          setStats(statsResp.value);
        }
        setLastRefreshedAt(new Date());
      } catch (err: any) {
        setError(err?.message || t('requests.fetchError'));
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [limit, offset, modelFilter, statusFilter, t]
  );

  // Initial and on filter change
  useEffect(() => {
    loadRequests(true);
  }, [offset, limit, modelFilter, statusFilter, loadRequests]);

  // Polling hook: pauses when modal is open or when autoRefreshSec is 0
  useEffect(() => {
    if (autoRefreshSec <= 0 || inspectItem !== null || selectedActiveReq !== null) return;

    const timer = setInterval(() => {
      loadRequests(false);
    }, autoRefreshSec * 1000);

    return () => clearInterval(timer);
  }, [autoRefreshSec, inspectItem, selectedActiveReq, loadRequests]);

  // Real-time Active Requests SSE stream with fallback polling
  useEffect(() => {
    if (autoRefreshSec <= 0 || inspectItem !== null || selectedActiveReq !== null) return;

    let cleanupStream: (() => void) | null = null;
    let fallbackTimer: any = null;

    try {
      cleanupStream = api.getActiveRequestsStream(
        (streamData) => {
          setActiveRequests(streamData.items || []);
        },
        () => {
          // If SSE fails or disconnects, fallback to fast polling
          if (!fallbackTimer) {
            fallbackTimer = setInterval(async () => {
              try {
                const res = await api.getActiveRequests();
                setActiveRequests(res.items || []);
              } catch {
                // ignore
              }
            }, 1500);
          }
        }
      );
    } catch {
      fallbackTimer = setInterval(async () => {
        try {
          const res = await api.getActiveRequests();
          setActiveRequests(res.items || []);
        } catch {
          // ignore
        }
      }, 1500);
    }

    // Initial fetch
    api.getActiveRequests().then((res) => setActiveRequests(res.items || [])).catch(() => {});

    return () => {
      if (cleanupStream) cleanupStream();
      if (fallbackTimer) clearInterval(fallbackTimer);
    };
  }, [autoRefreshSec, inspectItem, selectedActiveReq]);

  // Handle ESC key to close modal
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (inspectItem) setInspectItem(null);
        if (selectedActiveReq) setSelectedActiveReq(null);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [inspectItem, selectedActiveReq]);

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setOffset(0);
    loadRequests(true);
  };

  const setModelPreset = (prefix: string) => {
    setModelFilter(prefix);
    setOffset(0);
  };

  const formatRelativeTime = (ts: number) => {
    if (!ts) return '-';
    const ms = ts > 10000000000 ? ts : ts * 1000;
    const diffSec = Math.floor((Date.now() - ms) / 1000);
    if (diffSec < 5) return t('requests.timeJustNow');
    if (diffSec < 60) return t('requests.timeSecAgo', { count: diffSec });
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return t('requests.timeMinAgo', { count: diffMin });
    const diffHour = Math.floor(diffMin / 60);
    if (diffHour < 24) return t('requests.timeHourAgo', { count: diffHour });
    return new Date(ms).toLocaleDateString(locale === 'vi' ? 'vi-VN' : 'en-US', {
      day: '2-digit',
      month: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const formatExactTime = (ts: number) => {
    if (!ts) return '-';
    const ms = ts > 10000000000 ? ts : ts * 1000;
    const d = new Date(ms);
    const dateLoc = locale === 'vi' ? 'vi-VN' : 'en-US';
    return `${d.toLocaleTimeString(dateLoc)} ${d.toLocaleDateString(dateLoc)}`;
  };

  const getLatencyBadgeClass = (ms: number) => {
    if (ms < 1000) return 'badge-latency latency-fast';
    if (ms < 3000) return 'badge-latency latency-medium';
    return 'badge-latency latency-slow';
  };

  const getLatencyLabel = (ms: number) => {
    if (ms < 1000) return t('requests.latencyFast');
    if (ms < 3000) return t('requests.latencyMed');
    return t('requests.latencySlow');
  };

  const getModelFamily = (model: string) => {
    if (model.startsWith('ag/')) return { label: 'AG', className: 'family-tag family-tag-ag' };
    if (model.startsWith('cx/')) return { label: 'CX', className: 'family-tag family-tag-cx' };
    if (model.startsWith('openrouter/')) return { label: 'OR', className: 'family-tag family-tag-or' };
    return { label: 'UP', className: 'family-tag family-tag-custom' };
  };

  const handleCopyRaw = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedRaw(true);
    setTimeout(() => setCopiedRaw(false), 2000);
  };

  // Client-side latency filter if user selects fast/med/slow
  const filteredItems = (data?.items || []).filter((item) => {
    if (latencyPreset === 'fast') return item.duration_ms < 1000;
    if (latencyPreset === 'med') return item.duration_ms >= 1000 && item.duration_ms < 3000;
    if (latencyPreset === 'slow') return item.duration_ms >= 3000;
    return true;
  });

  const totalPages = data ? Math.ceil(data.total / limit) : 1;
  const currentPage = Math.floor(offset / limit) + 1;

  const formatDuration = (ms: number) => {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  };

  const agActive = activeRequests.find(
    (r) => r.provider === 'Google Antigravity' || r.model.startsWith('ag/')
  );
  const cxActive = activeRequests.find(
    (r) => r.provider === 'Codex' || r.model.startsWith('cx/')
  );
  const extActive = activeRequests.find(
    (r) => r !== agActive && r !== cxActive && r.provider !== 'Router'
  );

  return (
    <div>
      {/* Top Live Control Bar */}
      <div className="traffic-topbar">
        <div className="traffic-live-indicator">
          <span className={`live-pulse-dot ${autoRefreshSec === 0 ? 'paused' : ''}`} />
          <span style={{ color: autoRefreshSec > 0 ? '#10b981' : 'var(--text-muted)' }}>
            {autoRefreshSec > 0 ? t('requests.liveActive', { sec: autoRefreshSec }) : t('requests.livePaused')}
          </span>
          <span style={{ fontSize: 11, color: 'var(--text-muted)', marginLeft: 8 }}>
            {t('requests.lastUpdated', { time: lastRefreshedAt.toLocaleTimeString(locale === 'vi' ? 'vi-VN' : 'en-US') })}
          </span>
        </div>

        <div className="traffic-controls-group">
          <label style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{t('requests.frequencyLabel')}</label>
          <select
            value={autoRefreshSec}
            onChange={(e) => setAutoRefreshSec(Number(e.target.value))}
            style={{ width: 'auto', padding: '6px 10px', fontSize: 13, minHeight: 44 }}
          >
            <option value={0}>{t('requests.autoOff')}</option>
            <option value={3}>{t('requests.secCount', { count: 3 })}</option>
            <option value={5}>{t('requests.sec5Default')}</option>
            <option value={10}>{t('requests.secCount', { count: 10 })}</option>
            <option value={15}>{t('requests.secCount', { count: 15 })}</option>
          </select>

          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => setAutoRefreshSec(autoRefreshSec > 0 ? 0 : 5)}
            title={autoRefreshSec > 0 ? t('requests.pausePolling') : t('requests.resumePolling')}
          >
            {autoRefreshSec > 0 ? <IconPause size={15} /> : <IconPlay size={15} />}
            <span>{autoRefreshSec > 0 ? t('requests.pause') : t('requests.resume')}</span>
          </button>

          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={() => loadRequests(false)}
            disabled={refreshing}
            style={{ display: 'flex', alignItems: 'center', gap: 6 }}
          >
            <IconRefresh size={15} className={refreshing ? 'spinning' : ''} />
            <span>{t('actions.refresh')}</span>
          </button>
        </div>
      </div>

      {/* 9router-style Always-Visible Request Pipeline Flow */}
      <div className="pf" id="pipelineFlow">
        <div className="pf-title">
          <div className="pf-heading">
            <div className="pf-kicker">{t('requests.kicker')}</div>
            <b>{t('requests.pipelineTitle')}</b>
          </div>
          <div className="pf-title-meta">
            <span className={`pf-live-badge ${autoRefreshSec === 0 ? 'paused' : ''}`}>
              <span className={`pf-live-dot ${autoRefreshSec === 0 ? 'paused' : ''}`} />
              {autoRefreshSec > 0 ? t('requests.liveBadge') : t('requests.pausedBadge')}
            </span>
            <span className="pf-count">
              <span>{activeRequests.length}</span>
              <small>{t('requests.activeCount')}</small>
            </span>
            <span className="pf-legend">
              {activeRequests.length > 0
                ? activeRequests.length === 1
                  ? t('requests.routingActiveSingular')
                  : t('requests.routingActivePlural', { count: activeRequests.length })
                : t('requests.monitoringIdle')}
            </span>
          </div>
        </div>

        <div className="pf-diagram">
          {/* Client Node */}
          <div className="pf-node pf-source">
            <div className="pf-icon">⬡</div>
            <div className="pf-label">{t('requests.clientNode')}</div>
            <div className="pf-detail">{t('requests.clientDetail')}</div>
          </div>

          {/* Client -> Router Curved Connector */}
          <div className={`pf-conn pf-conn-client-router ${activeRequests.length > 0 ? 'active' : ''}`}>
            {/* Desktop Horizontal S-Curve */}
            <svg
              className="pf-svg-curve pf-svg-desktop"
              viewBox="0 0 100 40"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <defs>
                <linearGradient id="pfGradClientRouter" x1="0%" y1="0%" x2="100%" y2="0%">
                  <stop offset="0%" stopColor="#6366f1" />
                  <stop offset="50%" stopColor="#818cf8" />
                  <stop offset="100%" stopColor="#c7d2fe" />
                </linearGradient>
                <filter id="pfGlowClient" x="-20%" y="-20%" width="140%" height="140%">
                  <feGaussianBlur stdDeviation="2.5" result="blur" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
              </defs>
              {/* Idle dim continuous path */}
              <path
                d="M 0,20 C 35,10 65,30 100,20"
                className="pf-path-base"
                vectorEffect="non-scaling-stroke"
              />
              {/* Active illuminated path */}
              {activeRequests.length > 0 && (
                <>
                  <path
                    d="M 0,20 C 35,10 65,30 100,20"
                    className="pf-path-active"
                    stroke="url(#pfGradClientRouter)"
                    filter="url(#pfGlowClient)"
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle className="pf-flowing-dot pf-dot-client" r="3.5" fill="#e0e7ff">
                    <animateMotion
                      dur="1.3s"
                      repeatCount="indefinite"
                      path="M 0,20 C 35,10 65,30 100,20"
                    />
                  </circle>
                </>
              )}
            </svg>

            {/* Mobile Vertical S-Curve */}
            <svg
              className="pf-svg-curve pf-svg-mobile"
              viewBox="0 0 100 32"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <path
                d="M 50,0 C 40,11 60,21 50,32"
                className="pf-path-base"
                vectorEffect="non-scaling-stroke"
              />
              {activeRequests.length > 0 && (
                <>
                  <path
                    d="M 50,0 C 40,11 60,21 50,32"
                    className="pf-path-active"
                    stroke="url(#pfGradClientRouter)"
                    filter="url(#pfGlowClient)"
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle className="pf-flowing-dot pf-dot-client" r="3.5" fill="#e0e7ff">
                    <animateMotion
                      dur="1.3s"
                      repeatCount="indefinite"
                      path="M 50,0 C 40,11 60,21 50,32"
                    />
                  </circle>
                </>
              )}
            </svg>
          </div>

          {/* Router Gateway Node */}
          <div
            className={`pf-node pf-source pf-router ${activeRequests.length > 0 ? 'active' : 'idle'}`}
          >
            <div className="pf-icon">ez</div>
            <div className="pf-label">{t('requests.routerNode')}</div>
            <div className="pf-detail">{t('requests.routerDetail')}</div>
            <span className="pf-router-badge">{t('requests.gatewayBadge')}</span>
          </div>

          {/* Router -> Providers Curved Fan-out Connector */}
          <div className="pf-conn pf-conn-router-branch">
            {/* Desktop Natural S-Curve Fan-out to 3 Provider Nodes */}
            <svg
              className="pf-svg-curve pf-svg-branch-desktop pf-svg-desktop"
              viewBox="0 0 100 100"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <defs>
                <linearGradient id="pfGradAg" x1="0%" y1="0%" x2="100%" y2="0%">
                  <stop offset="0%" stopColor="#818cf8" />
                  <stop offset="60%" stopColor="#a855f7" />
                  <stop offset="100%" stopColor="#d8b4fe" />
                </linearGradient>
                <linearGradient id="pfGradCx" x1="0%" y1="0%" x2="100%" y2="0%">
                  <stop offset="0%" stopColor="#818cf8" />
                  <stop offset="60%" stopColor="#0ea5e9" />
                  <stop offset="100%" stopColor="#7dd3fc" />
                </linearGradient>
                <linearGradient id="pfGradExt" x1="0%" y1="0%" x2="100%" y2="0%">
                  <stop offset="0%" stopColor="#818cf8" />
                  <stop offset="60%" stopColor="#10b981" />
                  <stop offset="100%" stopColor="#6ee7b7" />
                </linearGradient>
                <filter id="pfGlowAg" x="-20%" y="-20%" width="140%" height="140%">
                  <feGaussianBlur stdDeviation="2.5" result="blur" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
                <filter id="pfGlowCx" x="-20%" y="-20%" width="140%" height="140%">
                  <feGaussianBlur stdDeviation="2.5" result="blur" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
                <filter id="pfGlowExt" x="-20%" y="-20%" width="140%" height="140%">
                  <feGaussianBlur stdDeviation="2.5" result="blur" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
              </defs>

              {/* Idle dim continuous fan-out paths */}
              <path
                d="M 0,50 C 45,50 55,16.7 100,16.7"
                className="pf-path-base"
                vectorEffect="non-scaling-stroke"
              />
              <path
                d="M 0,50 C 35,50 65,50 100,50"
                className="pf-path-base"
                vectorEffect="non-scaling-stroke"
              />
              <path
                d="M 0,50 C 45,50 55,83.3 100,83.3"
                className="pf-path-base"
                vectorEffect="non-scaling-stroke"
              />

              {/* Router Junction Source Node */}
              <circle
                cx="0"
                cy="50"
                r="3.5"
                className={`pf-junction-node ${activeRequests.length > 0 ? 'active' : ''}`}
              />

              {/* Active Path & Flowing Dot: Google Antigravity */}
              {agActive && (
                <>
                  <path
                    d="M 0,50 C 45,50 55,16.7 100,16.7"
                    className="pf-path-active pf-path-ag"
                    stroke="url(#pfGradAg)"
                    filter="url(#pfGlowAg)"
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle className="pf-flowing-dot pf-dot-ag" r="3.5" fill="#f3e8ff">
                    <animateMotion
                      dur="1.4s"
                      repeatCount="indefinite"
                      path="M 0,50 C 45,50 55,16.7 100,16.7"
                    />
                  </circle>
                  <circle cx="100" cy="16.7" r="3.5" className="pf-terminal-node active-ag" />
                </>
              )}

              {/* Active Path & Flowing Dot: OpenAI Codex */}
              {cxActive && (
                <>
                  <path
                    d="M 0,50 C 35,50 65,50 100,50"
                    className="pf-path-active pf-path-cx"
                    stroke="url(#pfGradCx)"
                    filter="url(#pfGlowCx)"
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle className="pf-flowing-dot pf-dot-cx" r="3.5" fill="#e0f2fe">
                    <animateMotion
                      dur="1.4s"
                      repeatCount="indefinite"
                      path="M 0,50 C 35,50 65,50 100,50"
                    />
                  </circle>
                  <circle cx="100" cy="50" r="3.5" className="pf-terminal-node active-cx" />
                </>
              )}

              {/* Active Path & Flowing Dot: External */}
              {extActive && (
                <>
                  <path
                    d="M 0,50 C 45,50 55,83.3 100,83.3"
                    className="pf-path-active pf-path-ext"
                    stroke="url(#pfGradExt)"
                    filter="url(#pfGlowExt)"
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle className="pf-flowing-dot pf-dot-ext" r="3.5" fill="#d1fae5">
                    <animateMotion
                      dur="1.4s"
                      repeatCount="indefinite"
                      path="M 0,50 C 45,50 55,83.3 100,83.3"
                    />
                  </circle>
                  <circle cx="100" cy="83.3" r="3.5" className="pf-terminal-node active-ext" />
                </>
              )}
            </svg>

            {/* Mobile Vertical Trunk Connector: curves from Router bottom center to left branch rail */}
            <svg
              className="pf-svg-curve pf-svg-mobile"
              viewBox="0 0 360 36"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <path
                d="M 180,0 C 180,18 14,18 14,36"
                className="pf-path-base"
                vectorEffect="non-scaling-stroke"
              />
              {activeRequests.length > 0 && (
                <>
                  <path
                    d="M 180,0 C 180,18 14,18 14,36"
                    className="pf-path-active"
                    stroke="url(#pfGradClientRouter)"
                    filter="url(#pfGlowClient)"
                    vectorEffect="non-scaling-stroke"
                  />
                  <circle className="pf-flowing-dot pf-dot-client" r="3.5" fill="#e0e7ff">
                    <animateMotion
                      dur="1.3s"
                      repeatCount="indefinite"
                      path="M 180,0 C 180,18 14,18 14,36"
                    />
                  </circle>
                </>
              )}
            </svg>
          </div>

          {/* Providers Branch */}
          <div className="pf-branch">
            {/* Google Antigravity Node */}
            <div className="pf-branch-row">
              <div className={`pf-conn pf-conn-sub ${agActive ? 'active' : ''}`}>
                <svg
                  className="pf-svg-curve pf-svg-mobile-sub"
                  viewBox="0 0 28 50"
                  preserveAspectRatio="none"
                  aria-hidden="true"
                >
                  {/* Left continuous trunk spanning through gap */}
                  <path
                    d="M 14,0 L 14,60"
                    className="pf-path-base pf-path-trunk"
                    vectorEffect="non-scaling-stroke"
                  />
                  {/* Organic curve into card */}
                  <path
                    d="M 14,0 L 14,12 C 14,22 18,25 28,25"
                    className="pf-path-base"
                    vectorEffect="non-scaling-stroke"
                  />
                  {agActive && (
                    <>
                      <path
                        d="M 14,0 L 14,12 C 14,22 18,25 28,25"
                        className="pf-path-active"
                        stroke="#a855f7"
                        vectorEffect="non-scaling-stroke"
                      />
                      <circle className="pf-flowing-dot" r="3" fill="#f3e8ff">
                        <animateMotion
                          dur="1.2s"
                          repeatCount="indefinite"
                          path="M 14,0 L 14,12 C 14,22 18,25 28,25"
                        />
                      </circle>
                    </>
                  )}
                </svg>
              </div>
              <div
                className={`pf-node pf-provider ${
                  agActive
                    ? `active ${
                        agActive.status === 'success'
                          ? 'state-success'
                          : agActive.status === 'error'
                          ? 'state-error'
                          : agActive.status === 'cancelled'
                          ? 'state-cancelled'
                          : ''
                      }`
                    : 'idle'
                }`}
                onClick={() => agActive && setSelectedActiveReq(agActive)}
                title={agActive ? t('requests.inspectRequestTitle') : 'Google Antigravity'}
              >
                <div className="pf-icon pf-icon-ag">G</div>
                <div className="pf-provider-info">
                  <div className="pf-label">Google Antigravity</div>
                  <div className="pf-detail">
                    {agActive
                      ? `${agActive.model} · ${agActive.account} · ${
                          agActive.status === 'generating'
                            ? `Stream ${formatDuration(agActive.elapsed_ms)}`
                            : formatDuration(agActive.elapsed_ms)
                        }`
                      : t('requests.readyWaiting')}
                  </div>
                  </div>
                  <span className="pf-provider-state">
                  {agActive
                    ? agActive.status === 'success'
                      ? t('requests.statusSuccess')
                      : agActive.status === 'error'
                      ? t('requests.statusError')
                      : agActive.status === 'cancelled'
                      ? t('requests.statusCancelled')
                      : agActive.status === 'generating'
                      ? '⚡ Đang sinh...'
                      : t('requests.statusActive')
                    : t('requests.statusReady')}
                  </span>
              </div>
            </div>

            {/* OpenAI Codex Node */}
            <div className="pf-branch-row">
              <div className={`pf-conn pf-conn-sub ${cxActive ? 'active' : ''}`}>
                <svg
                  className="pf-svg-curve pf-svg-mobile-sub"
                  viewBox="0 0 28 50"
                  preserveAspectRatio="none"
                  aria-hidden="true"
                >
                  {/* Left continuous trunk spanning through gap */}
                  <path
                    d="M 14,0 L 14,60"
                    className="pf-path-base pf-path-trunk"
                    vectorEffect="non-scaling-stroke"
                  />
                  {/* Organic curve into card */}
                  <path
                    d="M 14,0 L 14,12 C 14,22 18,25 28,25"
                    className="pf-path-base"
                    vectorEffect="non-scaling-stroke"
                  />
                  {cxActive && (
                    <>
                      <path
                        d="M 14,0 L 14,12 C 14,22 18,25 28,25"
                        className="pf-path-active"
                        stroke="#0ea5e9"
                        vectorEffect="non-scaling-stroke"
                      />
                      <circle className="pf-flowing-dot" r="3" fill="#e0f2fe">
                        <animateMotion
                          dur="1.2s"
                          repeatCount="indefinite"
                          path="M 14,0 L 14,12 C 14,22 18,25 28,25"
                        />
                      </circle>
                    </>
                  )}
                </svg>
              </div>
              <div
                className={`pf-node pf-provider ${
                  cxActive
                    ? `active ${
                        cxActive.status === 'success'
                          ? 'state-success'
                          : cxActive.status === 'error'
                          ? 'state-error'
                          : cxActive.status === 'cancelled'
                          ? 'state-cancelled'
                          : ''
                      }`
                    : 'idle'
                }`}
                onClick={() => cxActive && setSelectedActiveReq(cxActive)}
                title={cxActive ? t('requests.inspectRequestTitle') : 'OpenAI Codex'}
              >
                <div className="pf-icon pf-icon-cx">CX</div>
                <div className="pf-provider-info">
                  <div className="pf-label">OpenAI Codex</div>
                  <div className="pf-detail">
                    {cxActive
                      ? `${cxActive.model} · ${cxActive.account} · ${
                          cxActive.status === 'generating'
                            ? `Stream ${formatDuration(cxActive.elapsed_ms)}`
                            : formatDuration(cxActive.elapsed_ms)
                        }`
                      : t('requests.readyWaiting')}
                  </div>
                  </div>
                  <span className="pf-provider-state">
                  {cxActive
                    ? cxActive.status === 'success'
                      ? t('requests.statusSuccess')
                      : cxActive.status === 'error'
                      ? t('requests.statusError')
                      : cxActive.status === 'cancelled'
                      ? t('requests.statusCancelled')
                      : cxActive.status === 'generating'
                      ? '⚡ Đang sinh...'
                      : t('requests.statusActive')
                    : t('requests.statusReady')}
                  </span>
              </div>
            </div>

            {/* External Upstream Node */}
            <div className="pf-branch-row">
              <div className={`pf-conn pf-conn-sub ${extActive ? 'active' : ''}`}>
                <svg
                  className="pf-svg-curve pf-svg-mobile-sub"
                  viewBox="0 0 28 50"
                  preserveAspectRatio="none"
                  aria-hidden="true"
                >
                  {/* Organic curve into card (terminal) */}
                  <path
                    d="M 14,0 L 14,12 C 14,22 18,25 28,25"
                    className="pf-path-base"
                    vectorEffect="non-scaling-stroke"
                  />
                  {extActive && (
                    <>
                      <path
                        d="M 14,0 L 14,12 C 14,22 18,25 28,25"
                        className="pf-path-active"
                        stroke="#10b981"
                        vectorEffect="non-scaling-stroke"
                      />
                      <circle className="pf-flowing-dot" r="3" fill="#d1fae5">
                        <animateMotion
                          dur="1.2s"
                          repeatCount="indefinite"
                          path="M 14,0 L 14,12 C 14,22 18,25 28,25"
                        />
                      </circle>
                    </>
                  )}
                </svg>
              </div>
              <div
                className={`pf-node pf-provider ${
                  extActive
                    ? `active ${
                        extActive.status === 'success'
                          ? 'state-success'
                          : extActive.status === 'error'
                          ? 'state-error'
                          : extActive.status === 'cancelled'
                          ? 'state-cancelled'
                          : ''
                      }`
                    : 'idle'
                }`}
                onClick={() => extActive && setSelectedActiveReq(extActive)}
                title={extActive ? t('requests.inspectRequestTitle') : 'External Provider'}
              >
                <div className="pf-icon pf-icon-ext">↗</div>
                <div className="pf-provider-info">
                  <div className="pf-label">{extActive?.provider || 'External'}</div>
                  <div className="pf-detail">
                    {extActive
                      ? `${extActive.model} · ${formatDuration(extActive.elapsed_ms)}`
                      : t('requests.readyWaiting')}
                  </div>
                </div>
                <span className="pf-provider-state">
                  {extActive
                    ? extActive.status === 'success'
                      ? t('requests.statusSuccess')
                      : extActive.status === 'error'
                      ? t('requests.statusError')
                      : extActive.status === 'cancelled'
                      ? t('requests.statusCancelled')
                      : t('requests.statusActive')
                    : t('requests.statusReady')}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Active Requests Chips Bar */}
        <div className="pf-active-bar" aria-live="polite">
          {activeRequests.length === 0 ? (
            <div className="pf-active-empty">
              {t('requests.activeBarEmpty')}
            </div>
          ) : (
            activeRequests.map((req) => (
              <div
                key={req.id}
                className="pf-active-chip"
                onClick={() => setSelectedActiveReq(req)}
                title={t('requests.activeChipTitle')}
              >
                <span
                  className={`pf-chip-pulse ${
                    req.status === 'success'
                      ? 'success'
                      : req.status === 'error'
                      ? 'error'
                      : req.status === 'cancelled'
                      ? 'cancelled'
                      : ''
                  }`}
                />
                <span className="pf-chip-model">{req.model}</span>
                <span className="pf-chip-arrow">→</span>
                <span className="pf-chip-prov">{req.provider}</span>
                {req.account && <span className="pf-chip-acc">· {req.account}</span>}
                <span className="pf-chip-elapsed">
                  {req.status === 'generating'
                    ? `Stream ${formatDuration(req.elapsed_ms)}`
                    : req.status === 'routing'
                    ? `Kết nối ${formatDuration(req.elapsed_ms)}`
                    : formatDuration(req.elapsed_ms)}
                </span>
                <span className={`pf-chip-badge ${req.status}`}>
                  {req.status === 'generating'
                    ? '⚡ ĐANG SINH'
                    : req.status === 'routing'
                    ? 'KẾT NỐI'
                    : req.status.toUpperCase()}
                </span>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Real-time Traffic KPI Cards */}
      <div className="traffic-kpi-grid">
        <div className="card">
          <div className="kpi-label">
            <span>{t('requests.kpiTotalRequests')}</span>
            <IconZap size={16} />
          </div>
          <div className="kpi-value">
            {stats ? stats.total_requests.toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US') : data?.total ?? 0}
          </div>
          <div className="kpi-sub">
            {t('requests.kpiFilteredCount', { count: data?.total ?? 0 })}
          </div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>{t('requests.kpiMonitoring')}</span>
            <IconActivity size={16} />
          </div>
          <div className="kpi-value" style={{ color: '#818cf8' }}>
            {stats?.rpm ?? 0}
            <span style={{ fontSize: 13, fontWeight: 400, marginLeft: 4 }}>req/min</span>
          </div>
          <div className="kpi-sub">{t('requests.kpiRpmSub')}</div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>Thời Gian Stream TB</span>
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
              : '⚡ Thời gian truyền toàn bộ stream'}
          </div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>Tỷ Lệ Lỗi Hệ Thống</span>
            <span
              className={`badge ${
                stats && stats.error_rate > 1.0 ? 'badge-error' : 'badge-success'
              }`}
            >
              {stats?.error_rate ? `${stats.error_rate}%` : '0%'}
            </span>
          </div>
          <div
            className="kpi-value"
            style={{ color: stats && stats.error_rate > 1.0 ? '#ef4444' : 'inherit' }}
          >
            {stats?.error_count ?? 0}
          </div>
          <div className="kpi-sub">
            {stats?.quota_count !== undefined
              ? `⚡ Chạm Quota/Limit: ${stats.quota_count}`
              : 'Lỗi 5xx hoặc mạng (không tính hết quota)'}
          </div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>{t('requests.kpiTotalTokens')}</span>
            <span className="badge badge-neutral">In / Out</span>
          </div>
          <div className="kpi-value">
            {stats ? stats.total_tokens.toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US') : 0}
          </div>
          <div className="kpi-sub">
            P: {(stats?.prompt_tokens ?? 0).toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US')} | C:{' '}
            {(stats?.completion_tokens ?? 0).toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US')}
          </div>
        </div>
      </div>

      {/* Advanced Filter Form & Fast Chips */}
      <div className="card" style={{ marginBottom: 20 }}>
        <form onSubmit={handleSearchSubmit} className="filter-form">
          <div className="form-group filter-item-model">
            <label>{t('requests.filterModelLabel')}</label>
            <input
              type="text"
              placeholder={t('requests.filterModelPlaceholder')}
              value={modelFilter}
              onChange={(e) => setModelFilter(e.target.value)}
            />
          </div>

          <div className="form-group filter-item-status">
            <label>{t('requests.filterStatusLabel')}</label>
            <select value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}>
              <option value="">{t('requests.filterStatusAll')}</option>
              <option value="success">{t('requests.filterStatusSuccess')}</option>
              <option value="error">{t('requests.filterStatusError')}</option>
              <option value="cancelled">{t('requests.filterStatusCancelled')}</option>
            </select>
          </div>

          <div className="form-group filter-item-status">
            <label>{t('requests.filterLatencyLabel')}</label>
            <select
              value={latencyPreset}
              onChange={(e) => setLatencyPreset(e.target.value as any)}
            >
              <option value="all">{t('requests.filterLatencyAll')}</option>
              <option value="fast">{t('requests.filterLatencyFast')}</option>
              <option value="med">{t('requests.filterLatencyMed')}</option>
              <option value="slow">{t('requests.filterLatencySlow')}</option>
            </select>
          </div>

          <div className="form-group filter-item-limit">
            <label>{t('requests.filterLimitLabel')}</label>
            <select value={limit} onChange={(e) => setLimit(Number(e.target.value))}>
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
            </select>
          </div>

          <button type="submit" className="btn btn-primary filter-submit-btn">
            {t('requests.filterApply')}
          </button>
        </form>

        {/* Quick Filter Presets */}
        <div className="traffic-filter-presets">
          <span style={{ fontSize: 12, color: 'var(--text-muted)', alignSelf: 'center', marginRight: 4 }}>
            {t('requests.quickFilterLabel')}
          </span>
          <button
            type="button"
            className={`preset-chip ${modelFilter === '' ? 'active' : ''}`}
            onClick={() => setModelPreset('')}
          >
            {t('requests.quickFilterAll')}
          </button>
          <button
            type="button"
            className={`preset-chip ${modelFilter === 'ag/' ? 'active' : ''}`}
            onClick={() => setModelPreset('ag/')}
          >
            Google Antigravity (ag/*)
          </button>
          <button
            type="button"
            className={`preset-chip ${modelFilter === 'cx/' ? 'active' : ''}`}
            onClick={() => setModelPreset('cx/')}
          >
            OpenAI Codex (cx/*)
          </button>
          <button
            type="button"
            className={`preset-chip ${statusFilter === 'error' ? 'active' : ''}`}
            onClick={() => {
              setStatusFilter(statusFilter === 'error' ? '' : 'error');
              setOffset(0);
            }}
          >
            Lỗi Hệ Thống
          </button>
          <button
            type="button"
            className={`preset-chip ${statusFilter === 'quota_exhausted' ? 'active' : ''}`}
            onClick={() => {
              setStatusFilter(statusFilter === 'quota_exhausted' ? '' : 'quota_exhausted');
              setOffset(0);
            }}
          >
            ⚡ Hết Quota
          </button>
        </div>
      </div>

      {error && (
        <div className="alert alert-error">
          <IconAlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      {/* Main Traffic Table Card */}
      <div className="card">
        <div className="section-header">
          <div>
            <h2 className="section-title">{t('requests.tableTitle')}</h2>
            <p style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 2 }}>
              {t('requests.tableSubtitle')}
            </p>
          </div>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('requests.showingRequests', { shown: filteredItems.length, total: data?.total ?? 0 })}
          </span>
        </div>

        {loading ? (
          <div className="state-container">
            <div className="spinner" />
            <p>{t('common.loading')}</p>
          </div>
        ) : (
          <>
            <div className="table-container">
              <table>
                <thead>
                  <tr>
                    <th>{t('requests.thId')}</th>
                    <th>{t('requests.thTime')}</th>
                    <th>{t('requests.thModel')}</th>
                    <th>{t('requests.thAccountKey')}</th>
                    <th>{t('requests.thStatus')}</th>
                    <th>{t('requests.thTokens')}</th>
                    <th>{t('requests.thLatency')}</th>
                    <th style={{ textAlign: 'right' }}>{t('requests.thActions')}</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredItems.length === 0 ? (
                    <tr>
                      <td colSpan={8} style={{ textAlign: 'center', padding: 36, color: 'var(--text-muted)' }}>
                        {t('requests.tableEmpty')}
                      </td>
                    </tr>
                  ) : (
                    filteredItems.map((item: RequestLogItem) => {
                      const family = getModelFamily(item.model);
                      const totalTokens = item.prompt_tokens + item.completion_tokens || item.total_tokens || 1;
                      const promptPct = Math.round(((item.prompt_tokens || 0) / totalTokens) * 100);
                      const compPct = 100 - promptPct;

                      return (
                        <tr
                          key={item.id}
                          className="clickable-row"
                          onClick={() => {
                            setInspectItem(item);
                            setInspectTab(item.error ? 'error' : 'overview');
                          }}
                        >
                          <td className="font-mono" style={{ fontSize: 12 }}>
                            #{item.id}
                          </td>
                          <td
                            className="font-mono"
                            style={{ fontSize: 12, whiteSpace: 'nowrap' }}
                            title={formatExactTime(item.timestamp)}
                          >
                            {formatRelativeTime(item.timestamp)}
                          </td>
                          <td>
                            <div style={{ display: 'flex', alignItems: 'center' }}>
                              <span className={family.className}>{family.label}</span>
                              <span
                                className="font-mono text-break"
                                style={{ fontWeight: 600, color: 'var(--text-primary)' }}
                              >
                                {item.model}
                              </span>
                            </div>
                          </td>
                          <td className="font-mono" style={{ fontSize: 12 }}>
                            {item.account_id ? (
                              <span title={item.account_id} className="text-break">
                                {item.account_id.length > 14
                                  ? `${item.account_id.slice(0, 12)}...`
                                  : item.account_id}
                              </span>
                            ) : (
                              <span style={{ color: 'var(--text-muted)' }}>—</span>
                            )}
                          </td>
                          <td>
                            {(() => {
                              const isQuota =
                                item.status === 'quota_exhausted' ||
                                (item.error &&
                                  (item.error.toLowerCase().includes('quota') ||
                                    item.error.toLowerCase().includes('cooldown') ||
                                    item.error.toLowerCase().includes('usage_limit') ||
                                    item.error.toLowerCase().includes('payment required') ||
                                    item.error.toLowerCase().includes('credits')));
                              const isOk =
                                item.status === '200' ||
                                item.status === 'ok' ||
                                item.status === 'success' ||
                                item.status === 'completed';
                              const isCancelled = item.status === 'cancelled';

                              if (isQuota) {
                                return (
                                  <span
                                    className="badge badge-warning"
                                    style={{
                                      background: 'rgba(245, 158, 11, 0.15)',
                                      color: '#f59e0b',
                                      borderColor: 'rgba(245, 158, 11, 0.3)',
                                    }}
                                  >
                                    <IconAlertCircle size={12} />
                                    Hết Quota
                                  </span>
                                );
                              }
                              if (isOk) {
                                return (
                                  <span className="badge badge-success">
                                    <IconCheck size={12} />
                                    Thành công
                                  </span>
                                );
                              }
                              if (isCancelled) {
                                return (
                                  <span className="badge badge-neutral">
                                    Đã hủy
                                  </span>
                                );
                              }
                              return (
                                <span className="badge badge-error">
                                  <IconX size={12} />
                                  Lỗi Hệ Thống
                                </span>
                              );
                            })()}
                          </td>
                          <td>
                            <div className="token-bar-wrapper">
                              <span className="font-mono" style={{ fontSize: 12 }}>
                                {item.prompt_tokens} / {item.completion_tokens} /{' '}
                                <strong>{item.total_tokens}</strong>
                              </span>
                              <div className="token-bar-track" title={`Prompt: ${promptPct}%, Comp: ${compPct}%`}>
                                <div className="token-bar-prompt" style={{ width: `${promptPct}%` }} />
                                <div className="token-bar-comp" style={{ width: `${compPct}%` }} />
                              </div>
                              {(() => {
                                const cost = estimateRequestCost(item.model, item.prompt_tokens, item.completion_tokens);
                                if (cost === null) return null;
                                return (
                                  <div style={{ marginTop: 3 }}>
                                    <span
                                      className="font-mono"
                                      style={{
                                        fontSize: 10.5,
                                        padding: '1px 5px',
                                        borderRadius: 4,
                                        background: 'rgba(16, 185, 129, 0.15)',
                                        color: '#10b981',
                                        border: '1px solid rgba(16, 185, 129, 0.25)',
                                        display: 'inline-block',
                                        fontWeight: 600,
                                      }}
                                      title={`Ước tính chi phí OpenRouter: $${cost.toFixed(6)} USD`}
                                    >
                                      💰 ${cost < 0.0001 ? '<0.0001' : cost < 0.01 ? cost.toFixed(4) : cost.toFixed(3)}
                                    </span>
                                  </div>
                                );
                              })()}
                            </div>
                          </td>
                          <td>
                            {(() => {
                              const tps =
                                item.duration_ms > 0 && item.completion_tokens > 20
                                  ? (item.completion_tokens / (item.duration_ms / 1000)).toFixed(1)
                                  : null;
                              return (
                                <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                                  <span
                                    className={
                                      item.completion_tokens > 20
                                        ? 'badge-latency latency-fast'
                                        : getLatencyBadgeClass(item.duration_ms)
                                    }
                                  >
                                    {item.duration_ms < 1000
                                      ? `${item.duration_ms.toFixed(0)} ms`
                                      : `${(item.duration_ms / 1000).toFixed(1)}s`}
                                  </span>
                                  {tps && (
                                    <span style={{ fontSize: 10.5, color: 'var(--text-muted)' }}>
                                      ⚡ {tps} t/s
                                    </span>
                                  )}
                                </div>
                              );
                            })()}
                          </td>
                          <td style={{ textAlign: 'right' }}>
                            <button
                              type="button"
                              className="btn btn-secondary btn-sm"
                              onClick={(e) => {
                                e.stopPropagation();
                                setInspectItem(item);
                                setInspectTab(item.error ? 'error' : 'overview');
                              }}
                              style={{
                                color: item.error ? '#f87171' : 'inherit',
                                borderColor: item.error ? 'rgba(239, 68, 68, 0.4)' : undefined,
                              }}
                            >
                              {item.error ? t('requests.btnViewError') : t('requests.btnDetail')}
                            </button>
                          </td>
                        </tr>
                      );
                    })
                  )}
                </tbody>
              </table>
            </div>

            {/* Pagination Controls */}
            <div className="pagination-bar">
              <span className="pagination-info">
                {t('requests.paginationInfo', {
                  page: currentPage,
                  totalPages: totalPages || 1,
                  from: offset + 1,
                  to: Math.min(offset + limit, data?.total || 0),
                  total: data?.total || 0,
                })}
              </span>

              <div className="pagination-controls">
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => setOffset(Math.max(0, offset - limit))}
                  disabled={offset === 0}
                >
                  {t('requests.paginationPrev')}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => setOffset(offset + limit)}
                  disabled={offset + limit >= (data?.total || 0)}
                >
                  {t('requests.paginationNext')}
                </button>
              </div>
            </div>
          </>
        )}
      </div>

      {/* Comprehensive Request Inspector Modal */}
      {inspectItem && (
        <div className="modal-backdrop" onClick={() => setInspectItem(null)}>
          <div
            className="modal-card"
            style={{ maxWidth: 650 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <h3 className="modal-title font-mono">
                  {t('requests.modalRequestTitle', { id: inspectItem.id })}
                </h3>
                <span
                  className={`badge ${
                    inspectItem.status === 'quota_exhausted' ||
                    (inspectItem.error &&
                      (inspectItem.error.toLowerCase().includes('quota') ||
                        inspectItem.error.toLowerCase().includes('cooldown') ||
                        inspectItem.error.toLowerCase().includes('usage_limit') ||
                        inspectItem.error.toLowerCase().includes('payment required') ||
                        inspectItem.error.toLowerCase().includes('credits')))
                      ? 'badge-warning'
                      : inspectItem.status === '200' ||
                        inspectItem.status === 'ok' ||
                        inspectItem.status === 'success' ||
                        inspectItem.status === 'completed'
                      ? 'badge-success'
                      : inspectItem.status === 'cancelled'
                      ? 'badge-neutral'
                      : 'badge-error'
                  }`}
                  style={
                    inspectItem.status === 'quota_exhausted' ||
                    (inspectItem.error &&
                      (inspectItem.error.toLowerCase().includes('quota') ||
                        inspectItem.error.toLowerCase().includes('cooldown') ||
                        inspectItem.error.toLowerCase().includes('usage_limit') ||
                        inspectItem.error.toLowerCase().includes('payment required') ||
                        inspectItem.error.toLowerCase().includes('credits')))
                      ? {
                          background: 'rgba(245, 158, 11, 0.15)',
                          color: '#f59e0b',
                          borderColor: 'rgba(245, 158, 11, 0.3)',
                        }
                      : undefined
                  }
                >
                  {inspectItem.status === 'quota_exhausted' ||
                  (inspectItem.error &&
                    (inspectItem.error.toLowerCase().includes('quota') ||
                      inspectItem.error.toLowerCase().includes('cooldown') ||
                      inspectItem.error.toLowerCase().includes('usage_limit') ||
                      inspectItem.error.toLowerCase().includes('payment required') ||
                      inspectItem.error.toLowerCase().includes('credits')))
                    ? '⚡ Hết Quota'
                    : inspectItem.status === '200' ||
                      inspectItem.status === 'ok' ||
                      inspectItem.status === 'success' ||
                      inspectItem.status === 'completed'
                    ? 'Thành công'
                    : inspectItem.status === 'cancelled'
                    ? 'Đã hủy'
                    : 'Lỗi Hệ Thống'}
                </span>
              </div>
              <button
                type="button"
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setInspectItem(null)}
              >
                <IconX size={16} />
              </button>
            </div>

            {/* Modal Tabs */}
            <div className="inspector-tabs">
              <button
                type="button"
                className={`inspector-tab-btn ${inspectTab === 'overview' ? 'active' : ''}`}
                onClick={() => setInspectTab('overview')}
              >
                {t('requests.modalTabOverview')}
              </button>
              {inspectItem.error && (
                <button
                  type="button"
                  className={`inspector-tab-btn ${inspectTab === 'error' ? 'active' : ''}`}
                  onClick={() => setInspectTab('error')}
                  style={{ color: '#ef4444' }}
                >
                  {t('requests.modalTabError')}
                </button>
              )}
              <button
                type="button"
                className={`inspector-tab-btn ${inspectTab === 'raw' ? 'active' : ''}`}
                onClick={() => setInspectTab('raw')}
              >
                {t('requests.modalTabRaw')}
              </button>
            </div>

            {/* Tab 1: Overview */}
            {inspectTab === 'overview' && (
              <div>
                <div className="inspector-grid">
                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">{t('requests.modalFieldModel')}</div>
                    <div className="inspector-stat-value font-mono">
                      {inspectItem.model}
                    </div>
                  </div>

                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">{t('requests.modalFieldAccountKey')}</div>
                    <div className="inspector-stat-value font-mono">
                      {inspectItem.account_id || '—'}
                    </div>
                  </div>

                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">{t('requests.modalFieldRecordedAt')}</div>
                    <div className="inspector-stat-value font-mono" style={{ fontSize: 13 }}>
                      {formatExactTime(inspectItem.timestamp)}
                    </div>
                  </div>

                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">Thời gian thực thi / Stream</div>
                    <div className="inspector-stat-value">
                      <span className={inspectItem.completion_tokens > 20 ? 'badge-latency latency-fast' : getLatencyBadgeClass(inspectItem.duration_ms)}>
                        {inspectItem.duration_ms < 1000
                          ? `${inspectItem.duration_ms.toFixed(1)} ms (${getLatencyLabel(inspectItem.duration_ms)})`
                          : `${(inspectItem.duration_ms / 1000).toFixed(2)}s (Tổng stream)`}
                      </span>
                      {inspectItem.duration_ms > 0 && inspectItem.completion_tokens > 0 && (
                        <span style={{ fontSize: 12, color: 'var(--text-muted)', marginLeft: 8 }}>
                          ⚡ {((inspectItem.completion_tokens) / (inspectItem.duration_ms / 1000)).toFixed(1)} t/s
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                <div className="inspector-stat-box" style={{ marginTop: 12 }}>
                  <div className="inspector-stat-label">{t('requests.modalFieldTokenStats')}</div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 6, fontSize: 13 }}>
                    <span>{t('requests.modalPrompt')}: <strong>{inspectItem.prompt_tokens}</strong></span>
                    <span>{t('requests.modalCompletion')}: <strong>{inspectItem.completion_tokens}</strong></span>
                    <span>{t('requests.modalTotal')}: <strong>{inspectItem.total_tokens}</strong></span>
                  </div>
                  <div
                    className="token-bar-track"
                    style={{ maxWidth: '100%', height: 6, marginTop: 8 }}
                  >
                    <div
                      className="token-bar-prompt"
                      style={{
                        width: `${Math.round(
                          (inspectItem.prompt_tokens / (inspectItem.total_tokens || 1)) * 100
                        )}%`,
                      }}
                    />
                    <div
                      className="token-bar-comp"
                      style={{
                        width: `${Math.round(
                          (inspectItem.completion_tokens / (inspectItem.total_tokens || 1)) * 100
                        )}%`,
                      }}
                    />
                  </div>
                </div>

                {(() => {
                  const cost = estimateRequestCost(inspectItem.model, inspectItem.prompt_tokens, inspectItem.completion_tokens);
                  if (cost === null) return null;
                  return (
                    <div className="inspector-stat-box" style={{ marginTop: 12 }}>
                      <div className="inspector-stat-label">Chi phí ước tính (OpenRouter Cost)</div>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 6, fontSize: 13 }}>
                        <span style={{ color: '#10b981', fontWeight: 700, fontSize: 16 }}>
                          ${cost < 0.0001 ? '<$0.0001' : cost.toFixed(5)} USD
                        </span>
                        <span className="font-mono" style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                          {inspectItem.model.replace('openrouter/', '')}
                        </span>
                      </div>
                    </div>
                  );
                })()}
              </div>
            )}

            {/* Tab 2: Error Analysis */}
            {inspectTab === 'error' && inspectItem.error && (
              <div>
                <div style={{ marginBottom: 12, fontSize: 13, color: '#fca5a5' }}>
                  {t('requests.modalErrorNotice')}
                </div>
                <div
                  className="code-viewer-box"
                  style={{
                    color: '#fca5a5',
                    background: '#1a0d0d',
                    borderColor: 'rgba(239, 68, 68, 0.3)',
                    maxHeight: 300,
                  }}
                >
                  {inspectItem.error}
                </div>
              </div>
            )}

            {/* Tab 3: Raw JSON */}
            {inspectTab === 'raw' && (
              <div>
                <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleCopyRaw(JSON.stringify(inspectItem, null, 2))}
                    style={{ display: 'flex', alignItems: 'center', gap: 6 }}
                  >
                    <IconCopy size={14} />
                    <span>{copiedRaw ? t('requests.copiedJson') : t('requests.copyJson')}</span>
                  </button>
                </div>
                <div className="code-viewer-box" style={{ maxHeight: 300, fontSize: 12 }}>
                  {JSON.stringify(inspectItem, null, 2)}
                </div>
              </div>
            )}

            <div className="modal-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setInspectItem(null)}
              >
                {t('actions.close')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Active Request Detail Modal */}
      {selectedActiveReq && (
        <div className="modal-backdrop" onClick={() => setSelectedActiveReq(null)}>
          <div
            className="modal-card"
            style={{ maxWidth: 540 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h2 className="modal-title" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span className="live-pulse-dot" />
                {t('requests.modalLiveTitle')}
              </h2>
              <button
                type="button"
                className="btn-icon-only modal-close-btn"
                onClick={() => setSelectedActiveReq(null)}
                title={t('actions.close')}
                style={{ minWidth: 44, minHeight: 44 }}
              >
                <IconX size={18} />
              </button>
            </div>

            <div className="pf-modal-notice">
              {t('requests.modalLiveNotice')}
            </div>

            <div className="pf-modal-grid">
              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldStatus')}</div>
                <div className="pf-modal-field-value">
                  <span className={`pf-chip-badge ${selectedActiveReq.status}`}>
                    {selectedActiveReq.status.toUpperCase()}
                  </span>
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldElapsed')}</div>
                <div className="pf-modal-field-value" style={{ color: '#34d399', fontFamily: 'var(--font-mono)' }}>
                  {formatDuration(selectedActiveReq.elapsed_ms)}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldClient')}</div>
                <div className="pf-modal-field-value">{selectedActiveReq.client}</div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldProvider')}</div>
                <div className="pf-modal-field-value">{selectedActiveReq.provider}</div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldModel')}</div>
                <div className="pf-modal-field-value" style={{ fontFamily: 'var(--font-mono)' }}>
                  {selectedActiveReq.model}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldAccountMasked')}</div>
                <div className="pf-modal-field-value" style={{ fontFamily: 'var(--font-mono)' }}>
                  {selectedActiveReq.account}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldMode')}</div>
                <div className="pf-modal-field-value">
                  {selectedActiveReq.stream ? t('requests.modeStreaming') : t('requests.modeNonStreaming')}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">{t('requests.modalFieldStartedAt')}</div>
                <div className="pf-modal-field-value">
                  {new Date(selectedActiveReq.started_at * 1000).toLocaleTimeString(locale === 'vi' ? 'vi-VN' : 'en-US')}
                </div>
              </div>
            </div>

            <div className="pf-modal-field" style={{ marginBottom: 16 }}>
              <div className="pf-modal-field-label">{t('requests.modalFieldRequestId')}</div>
              <div
                className="pf-modal-field-value"
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: 12,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: 8,
                }}
              >
                <span style={{ wordBreak: 'break-all' }}>{selectedActiveReq.id}</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => {
                    navigator.clipboard.writeText(selectedActiveReq.id);
                    setCopiedActiveId(true);
                    setTimeout(() => setCopiedActiveId(false), 2000);
                  }}
                  style={{ minHeight: 44, padding: '0 12px', flexShrink: 0 }}
                >
                  {copiedActiveId ? <IconCheck size={14} /> : <IconCopy size={14} />}
                  <span>{copiedActiveId ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
            </div>

            <div className="modal-actions" style={{ justifyContent: 'flex-end' }}>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => setSelectedActiveReq(null)}
                style={{ minHeight: 44, minWidth: 90 }}
              >
                {t('actions.close')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
