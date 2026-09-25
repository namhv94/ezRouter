import React, { useState, useEffect, useCallback } from 'react';
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

export const RequestsTab: React.FC = () => {
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
        setError(err?.message || 'Không thể tải dữ liệu monitoring');
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [limit, offset, modelFilter, statusFilter]
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
    if (diffSec < 5) return 'vừa xong';
    if (diffSec < 60) return `${diffSec} giây trước`;
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `${diffMin} phút trước`;
    const diffHour = Math.floor(diffMin / 60);
    if (diffHour < 24) return `${diffHour} giờ trước`;
    return new Date(ms).toLocaleDateString('vi-VN', {
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
    return `${d.toLocaleTimeString('vi-VN')} ${d.toLocaleDateString('vi-VN')}`;
  };

  const getLatencyBadgeClass = (ms: number) => {
    if (ms < 1000) return 'badge-latency latency-fast';
    if (ms < 3000) return 'badge-latency latency-medium';
    return 'badge-latency latency-slow';
  };

  const getLatencyLabel = (ms: number) => {
    if (ms < 1000) return 'Nhanh';
    if (ms < 3000) return 'Trung bình';
    return 'Chậm';
  };

  const getModelFamily = (model: string) => {
    if (model.startsWith('ag/')) return { label: 'AG', className: 'family-tag family-tag-ag' };
    if (model.startsWith('cx/')) return { label: 'CX', className: 'family-tag family-tag-cx' };
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
            {autoRefreshSec > 0 ? `Trực tiếp (${autoRefreshSec}s)` : 'Tạm dừng tự động'}
          </span>
          <span style={{ fontSize: 11, color: 'var(--text-muted)', marginLeft: 8 }}>
            Cập nhật lúc: {lastRefreshedAt.toLocaleTimeString('vi-VN')}
          </span>
        </div>

        <div className="traffic-controls-group">
          <label style={{ fontSize: 12, color: 'var(--text-secondary)' }}>Tần suất:</label>
          <select
            value={autoRefreshSec}
            onChange={(e) => setAutoRefreshSec(Number(e.target.value))}
            style={{ width: 'auto', padding: '6px 10px', fontSize: 13, minHeight: 44 }}
          >
            <option value={0}>Tắt tự động</option>
            <option value={3}>3 giây</option>
            <option value={5}>5 giây (Mặc định)</option>
            <option value={10}>10 giây</option>
            <option value={15}>15 giây</option>
          </select>

          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => setAutoRefreshSec(autoRefreshSec > 0 ? 0 : 5)}
            title={autoRefreshSec > 0 ? 'Tạm dừng polling' : 'Bật polling'}
          >
            {autoRefreshSec > 0 ? <IconPause size={15} /> : <IconPlay size={15} />}
            <span>{autoRefreshSec > 0 ? 'Dừng' : 'Bật'}</span>
          </button>

          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={() => loadRequests(false)}
            disabled={refreshing}
            style={{ display: 'flex', alignItems: 'center', gap: 6 }}
          >
            <IconRefresh size={15} className={refreshing ? 'spinning' : ''} />
            <span>Làm Mới</span>
          </button>
        </div>
      </div>

      {/* 9router-style Always-Visible Request Pipeline Flow */}
      <div className="pf" id="pipelineFlow">
        <div className="pf-title">
          <div className="pf-heading">
            <div className="pf-kicker">Traffic</div>
            <b>Pipeline Flow</b>
          </div>
          <div className="pf-title-meta">
            <span className={`pf-live-badge ${autoRefreshSec === 0 ? 'paused' : ''}`}>
              <span className={`pf-live-dot ${autoRefreshSec === 0 ? 'paused' : ''}`} />
              {autoRefreshSec > 0 ? 'LIVE' : 'PAUSED'}
            </span>
            <span className="pf-count">
              <span>{activeRequests.length}</span>
              <small>active</small>
            </span>
            <span className="pf-legend">
              {activeRequests.length > 0
                ? `Routing ${activeRequests.length} request${activeRequests.length === 1 ? '' : 's'} · signal active`
                : 'Monitoring · all routes idle'}
            </span>
          </div>
        </div>

        <div className="pf-diagram">
          {/* Client Node */}
          <div className="pf-node pf-source">
            <div className="pf-icon">⬡</div>
            <div className="pf-label">Client</div>
            <div className="pf-detail">OpenAI / Hermes</div>
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
            <div className="pf-label">ezRouter</div>
            <div className="pf-detail">Route &amp; balance</div>
            <span className="pf-router-badge">Gateway</span>
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
                title={agActive ? 'Nhấp để xem chi tiết request' : 'Google Antigravity'}
              >
                <div className="pf-icon pf-icon-ag">G</div>
                <div className="pf-provider-info">
                  <div className="pf-label">Google Antigravity</div>
                  <div className="pf-detail">
                    {agActive
                      ? `${agActive.model} · ${agActive.account} · ${formatDuration(agActive.elapsed_ms)}`
                      : 'Sẵn sàng · Chờ request'}
                  </div>
                </div>
                <span className="pf-provider-state">
                  {agActive
                    ? agActive.status === 'success'
                      ? 'Thành công'
                      : agActive.status === 'error'
                      ? 'Lỗi'
                      : agActive.status === 'cancelled'
                      ? 'Đã hủy'
                      : 'Active'
                    : 'Ready'}
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
                title={cxActive ? 'Nhấp để xem chi tiết request' : 'OpenAI Codex'}
              >
                <div className="pf-icon pf-icon-cx">CX</div>
                <div className="pf-provider-info">
                  <div className="pf-label">OpenAI Codex</div>
                  <div className="pf-detail">
                    {cxActive
                      ? `${cxActive.model} · ${cxActive.account} · ${formatDuration(cxActive.elapsed_ms)}`
                      : 'Sẵn sàng · Chờ request'}
                  </div>
                </div>
                <span className="pf-provider-state">
                  {cxActive
                    ? cxActive.status === 'success'
                      ? 'Thành công'
                      : cxActive.status === 'error'
                      ? 'Lỗi'
                      : cxActive.status === 'cancelled'
                      ? 'Đã hủy'
                      : 'Active'
                    : 'Ready'}
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
                title={extActive ? 'Nhấp để xem chi tiết request' : 'External Provider'}
              >
                <div className="pf-icon pf-icon-ext">↗</div>
                <div className="pf-provider-info">
                  <div className="pf-label">{extActive?.provider || 'External'}</div>
                  <div className="pf-detail">
                    {extActive
                      ? `${extActive.model} · ${formatDuration(extActive.elapsed_ms)}`
                      : 'Sẵn sàng · Chờ request'}
                  </div>
                </div>
                <span className="pf-provider-state">
                  {extActive
                    ? extActive.status === 'success'
                      ? 'Thành công'
                      : extActive.status === 'error'
                      ? 'Lỗi'
                      : extActive.status === 'cancelled'
                      ? 'Đã hủy'
                      : 'Active'
                    : 'Ready'}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Active Requests Chips Bar */}
        <div className="pf-active-bar" aria-live="polite">
          {activeRequests.length === 0 ? (
            <div className="pf-active-empty">
              Không có request đang xử lý · Hệ thống ở trạng thái chờ (Idle)
            </div>
          ) : (
            activeRequests.map((req) => (
              <div
                key={req.id}
                className="pf-active-chip"
                onClick={() => setSelectedActiveReq(req)}
                title="Nhấp để kiểm tra chi tiết request"
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
                <span className="pf-chip-elapsed">{formatDuration(req.elapsed_ms)}</span>
                <span className={`pf-chip-badge ${req.status}`}>{req.status}</span>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Real-time Traffic KPI Cards */}
      <div className="traffic-kpi-grid">
        <div className="card">
          <div className="kpi-label">
            <span>Tổng Yêu Cầu</span>
            <IconZap size={16} />
          </div>
          <div className="kpi-value">
            {stats ? stats.total_requests.toLocaleString('vi-VN') : data?.total ?? 0}
          </div>
          <div className="kpi-sub">
            Hiển thị: <strong>{data?.total ?? 0}</strong> bản ghi lọc
          </div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>Monitoring</span>
            <IconActivity size={16} />
          </div>
          <div className="kpi-value" style={{ color: '#818cf8' }}>
            {stats?.rpm ?? 0}
            <span style={{ fontSize: 13, fontWeight: 400, marginLeft: 4 }}>req/min</span>
          </div>
          <div className="kpi-sub">Thông lượng Antigravity + Codex</div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>Độ Trễ TB</span>
            <span className="badge badge-neutral">Upstream</span>
          </div>
          <div className="kpi-value">
            {stats?.avg_duration_ms ? stats.avg_duration_ms.toFixed(1) : '0.0'}
            <span style={{ fontSize: 13, fontWeight: 400, marginLeft: 4 }}>ms</span>
          </div>
          <div className="kpi-sub">
            {stats?.avg_duration_ms && stats.avg_duration_ms < 1000
              ? '⚡ Phản hồi rất nhanh'
              : 'Ổn định theo mô hình'}
          </div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>Tỷ Lệ Lỗi</span>
            <span
              className={`badge ${
                stats && stats.error_count > 0 ? 'badge-error' : 'badge-success'
              }`}
            >
              {stats?.error_rate ? `${stats.error_rate}%` : '0%'}
            </span>
          </div>
          <div
            className="kpi-value"
            style={{ color: stats && stats.error_count > 0 ? '#ef4444' : 'inherit' }}
          >
            {stats?.error_count ?? 0}
          </div>
          <div className="kpi-sub">Lỗi upstream hoặc ngắt kết nối</div>
        </div>

        <div className="card">
          <div className="kpi-label">
            <span>Tổng Tokens</span>
            <span className="badge badge-neutral">In / Out</span>
          </div>
          <div className="kpi-value">
            {stats ? stats.total_tokens.toLocaleString('vi-VN') : 0}
          </div>
          <div className="kpi-sub">
            P: {(stats?.prompt_tokens ?? 0).toLocaleString('vi-VN')} | C:{' '}
            {(stats?.completion_tokens ?? 0).toLocaleString('vi-VN')}
          </div>
        </div>
      </div>

      {/* Advanced Filter Form & Fast Chips */}
      <div className="card" style={{ marginBottom: 20 }}>
        <form onSubmit={handleSearchSubmit} className="filter-form">
          <div className="form-group filter-item-model">
            <label>Lọc Theo Tên Model</label>
            <input
              type="text"
              placeholder="VD: gemini, claude, sol..."
              value={modelFilter}
              onChange={(e) => setModelFilter(e.target.value)}
            />
          </div>

          <div className="form-group filter-item-status">
            <label>Trạng Thái</label>
            <select value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}>
              <option value="">Tất cả trạng thái</option>
              <option value="success">Thành công (success / 200)</option>
              <option value="error">Lỗi (error / 4xx / 5xx)</option>
              <option value="cancelled">Đã hủy (cancelled)</option>
            </select>
          </div>

          <div className="form-group filter-item-status">
            <label>Phân Loại Độ Trễ</label>
            <select
              value={latencyPreset}
              onChange={(e) => setLatencyPreset(e.target.value as any)}
            >
              <option value="all">Tất cả tốc độ</option>
              <option value="fast">Nhanh (&lt; 1 giây)</option>
              <option value="med">Vừa (1 - 3 giây)</option>
              <option value="slow">Chậm (&gt; 3 giây)</option>
            </select>
          </div>

          <div className="form-group filter-item-limit">
            <label>Số Lượng</label>
            <select value={limit} onChange={(e) => setLimit(Number(e.target.value))}>
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
            </select>
          </div>

          <button type="submit" className="btn btn-primary filter-submit-btn">
            Áp Dụng Lọc
          </button>
        </form>

        {/* Quick Filter Presets */}
        <div className="traffic-filter-presets">
          <span style={{ fontSize: 12, color: 'var(--text-muted)', alignSelf: 'center', marginRight: 4 }}>
            Bộ lọc nhanh:
          </span>
          <button
            type="button"
            className={`preset-chip ${modelFilter === '' ? 'active' : ''}`}
            onClick={() => setModelPreset('')}
          >
            Tất Cả Models
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
            Chỉ Xem Lỗi
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
            <h2 className="section-title">Nhật Ký Yêu Cầu</h2>
            <p style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 2 }}>
              Nhấp vào dòng để xem chi tiết request, token và log upstream.
            </p>
          </div>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            Đang hiển thị <strong>{filteredItems.length}</strong> / <strong>{data?.total ?? 0}</strong> yêu cầu
          </span>
        </div>

        {loading ? (
          <div className="state-container">
            <div className="spinner" />
            <p>Đang tải nhật ký...</p>
          </div>
        ) : (
          <>
            <div className="table-container">
              <table>
                <thead>
                  <tr>
                    <th>ID</th>
                    <th>Thời Gian</th>
                    <th>Model</th>
                    <th>Tài Khoản / Key</th>
                    <th>Trạng Thái</th>
                    <th>Tokens (In / Out)</th>
                    <th>Độ Trễ</th>
                    <th style={{ textAlign: 'right' }}>Thao Tác</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredItems.length === 0 ? (
                    <tr>
                      <td colSpan={8} style={{ textAlign: 'center', padding: 36, color: 'var(--text-muted)' }}>
                        Không có yêu cầu nào phù hợp với bộ lọc hiện tại.
                      </td>
                    </tr>
                  ) : (
                    filteredItems.map((item: RequestLogItem) => {
                      const isOk =
                        item.status === '200' ||
                        item.status === 'ok' ||
                        item.status === 'success';
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
                            <span
                              className={`badge ${
                                isOk
                                  ? 'badge-success'
                                  : item.error
                                  ? 'badge-error'
                                  : 'badge-warning'
                              }`}
                            >
                              {isOk ? <IconCheck size={12} /> : <IconX size={12} />}
                              {item.status}
                            </span>
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
                            </div>
                          </td>
                          <td>
                            <span className={getLatencyBadgeClass(item.duration_ms)}>
                              {item.duration_ms.toFixed(0)} ms
                            </span>
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
                              {item.error ? 'Xem Lỗi' : 'Chi Tiết'}
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
                Trang {currentPage} / {totalPages || 1} (Bản ghi {offset + 1} -{' '}
                {Math.min(offset + limit, data?.total || 0)} trong tổng số {data?.total || 0})
              </span>

              <div className="pagination-controls">
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => setOffset(Math.max(0, offset - limit))}
                  disabled={offset === 0}
                >
                  Trước
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => setOffset(offset + limit)}
                  disabled={offset + limit >= (data?.total || 0)}
                >
                  Tiếp Theo
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
                  Yêu Cầu #{inspectItem.id}
                </h3>
                <span
                  className={`badge ${
                    inspectItem.status === '200' ||
                    inspectItem.status === 'ok' ||
                    inspectItem.status === 'success'
                      ? 'badge-success'
                      : 'badge-error'
                  }`}
                >
                  {inspectItem.status}
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
                Tổng Quan
              </button>
              {inspectItem.error && (
                <button
                  type="button"
                  className={`inspector-tab-btn ${inspectTab === 'error' ? 'active' : ''}`}
                  onClick={() => setInspectTab('error')}
                  style={{ color: '#ef4444' }}
                >
                  Lỗi Upstream
                </button>
              )}
              <button
                type="button"
                className={`inspector-tab-btn ${inspectTab === 'raw' ? 'active' : ''}`}
                onClick={() => setInspectTab('raw')}
              >
                Dữ Liệu Thô (JSON)
              </button>
            </div>

            {/* Tab 1: Overview */}
            {inspectTab === 'overview' && (
              <div>
                <div className="inspector-grid">
                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">Mô Hình (Model)</div>
                    <div className="inspector-stat-value font-mono">
                      {inspectItem.model}
                    </div>
                  </div>

                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">Tài Khoản / Key</div>
                    <div className="inspector-stat-value font-mono">
                      {inspectItem.account_id || '—'}
                    </div>
                  </div>

                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">Thời Gian Ghi Nhận</div>
                    <div className="inspector-stat-value font-mono" style={{ fontSize: 13 }}>
                      {formatExactTime(inspectItem.timestamp)}
                    </div>
                  </div>

                  <div className="inspector-stat-box">
                    <div className="inspector-stat-label">Thời Gian Phản Hồi</div>
                    <div className="inspector-stat-value">
                      <span className={getLatencyBadgeClass(inspectItem.duration_ms)}>
                        {inspectItem.duration_ms.toFixed(1)} ms ({getLatencyLabel(inspectItem.duration_ms)})
                      </span>
                    </div>
                  </div>
                </div>

                <div className="inspector-stat-box" style={{ marginTop: 12 }}>
                  <div className="inspector-stat-label">Thống Kê Token Tiêu Thụ</div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 6, fontSize: 13 }}>
                    <span>Prompt: <strong>{inspectItem.prompt_tokens}</strong></span>
                    <span>Completion: <strong>{inspectItem.completion_tokens}</strong></span>
                    <span>Tổng: <strong>{inspectItem.total_tokens}</strong></span>
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
              </div>
            )}

            {/* Tab 2: Error Analysis */}
            {inspectTab === 'error' && inspectItem.error && (
              <div>
                <div style={{ marginBottom: 12, fontSize: 13, color: '#fca5a5' }}>
                  Thông báo lỗi được ghi nhận từ phía upstream hoặc hệ thống proxy:
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
                    <span>{copiedRaw ? 'Đã Sao Chép!' : 'Sao Chép JSON'}</span>
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
                Đóng
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
                Chi Tiết Live Request
              </h2>
              <button
                type="button"
                className="btn-icon-only modal-close-btn"
                onClick={() => setSelectedActiveReq(null)}
                title="Đóng"
                style={{ minWidth: 44, minHeight: 44 }}
              >
                <IconX size={18} />
              </button>
            </div>

            <div className="pf-modal-notice">
              🔒 Dữ liệu trực tiếp được chuẩn hóa an toàn: prompts, completions, tokens và API keys không bao giờ được ghi nhớ trong registry bộ nhớ đệm.
            </div>

            <div className="pf-modal-grid">
              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Trạng Thái</div>
                <div className="pf-modal-field-value">
                  <span className={`pf-chip-badge ${selectedActiveReq.status}`}>
                    {selectedActiveReq.status.toUpperCase()}
                  </span>
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Độ Trễ / Thời Gian</div>
                <div className="pf-modal-field-value" style={{ color: '#34d399', fontFamily: 'var(--font-mono)' }}>
                  {formatDuration(selectedActiveReq.elapsed_ms)}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Khách Hàng (Client)</div>
                <div className="pf-modal-field-value">{selectedActiveReq.client}</div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Provider Đã Chọn</div>
                <div className="pf-modal-field-value">{selectedActiveReq.provider}</div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Mô Hình (Model)</div>
                <div className="pf-modal-field-value" style={{ fontFamily: 'var(--font-mono)' }}>
                  {selectedActiveReq.model}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Tài Khoản (Masked)</div>
                <div className="pf-modal-field-value" style={{ fontFamily: 'var(--font-mono)' }}>
                  {selectedActiveReq.account}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Chế Độ Truyền</div>
                <div className="pf-modal-field-value">
                  {selectedActiveReq.stream ? 'SSE Streaming' : 'Non-streaming (JSON)'}
                </div>
              </div>

              <div className="pf-modal-field">
                <div className="pf-modal-field-label">Bắt Đầu Lúc</div>
                <div className="pf-modal-field-value">
                  {new Date(selectedActiveReq.started_at * 1000).toLocaleTimeString('vi-VN')}
                </div>
              </div>
            </div>

            <div className="pf-modal-field" style={{ marginBottom: 16 }}>
              <div className="pf-modal-field-label">Request ID</div>
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
                  <span>{copiedActiveId ? 'Đã sao chép' : 'Copy'}</span>
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
                Đóng
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
