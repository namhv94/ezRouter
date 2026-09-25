import React, { useState, useEffect, useCallback, useRef } from 'react';
import { SystemLogEntry } from '../types';
import { api } from '../api';
import {
  IconTerminal,
  IconRefresh,
  IconCopy,
  IconCheck,
  IconTrash,
  IconAlertCircle,
  IconSearch,
} from '../icons';

export const ConsoleLogTab: React.FC = () => {
  const [logs, setLogs] = useState<SystemLogEntry[]>([]);
  const [total, setTotal] = useState<number>(0);
  const [loading, setLoading] = useState<boolean>(true);
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Filters & Controls
  const [levelFilter, setLevelFilter] = useState<string>('all');
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [limit, setLimit] = useState<number>(200);
  const [autoRefreshSec, setAutoRefreshSec] = useState<number>(3);
  const [autoScroll, setAutoScroll] = useState<boolean>(true);

  // Actions feedback
  const [copied, setCopied] = useState<boolean>(false);
  const [clearConfirm, setClearConfirm] = useState<boolean>(false);

  const consoleEndRef = useRef<HTMLDivElement | null>(null);
  const consoleContainerRef = useRef<HTMLDivElement | null>(null);

  const fetchLogs = useCallback(
    async (isInitial = false) => {
      if (isInitial) {
        setLoading(true);
      } else {
        setRefreshing(true);
      }
      setError(null);
      try {
        const resp = await api.getSystemLogs({
          limit,
          level: levelFilter !== 'all' ? levelFilter : undefined,
          search: searchQuery.trim() || undefined,
        });
        setLogs(resp.logs || []);
        setTotal(resp.total || 0);
      } catch (err: any) {
        setError(err?.message || 'Không thể tải log hệ thống.');
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [limit, levelFilter, searchQuery]
  );

  useEffect(() => {
    fetchLogs(true);
  }, [fetchLogs]);

  // Auto-refresh interval
  useEffect(() => {
    if (autoRefreshSec <= 0) return;
    const interval = setInterval(() => {
      fetchLogs(false);
    }, autoRefreshSec * 1000);
    return () => clearInterval(interval);
  }, [autoRefreshSec, fetchLogs]);

  // Auto-scroll to bottom if enabled
  useEffect(() => {
    if (autoScroll && consoleEndRef.current) {
      consoleEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const handleCopyLogs = () => {
    if (logs.length === 0) return;
    const rawText = logs
      .map(
        (l) =>
          `[${l.timestamp}] [${l.level}] [${l.target}] ${l.message}`
      )
      .join('\n');
    navigator.clipboard.writeText(rawText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const handleClearLogs = async () => {
    try {
      await api.clearSystemLogs();
      setLogs([]);
      setTotal(0);
      setClearConfirm(false);
    } catch (err: any) {
      setError(err?.message || 'Không thể xóa log.');
    }
  };

  // Counts by level for badge display
  const countInfo = logs.filter((l) => l.level.toUpperCase() === 'INFO').length;
  const countWarn = logs.filter((l) => l.level.toUpperCase() === 'WARN').length;
  const countError = logs.filter((l) => l.level.toUpperCase() === 'ERROR').length;
  const countDebug = logs.filter((l) => l.level.toUpperCase() === 'DEBUG').length;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {/* Header */}
      <div className="section-header" style={{ marginBottom: 0 }}>
        <div>
          <h2 className="section-title" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <IconTerminal size={22} style={{ color: 'var(--color-primary, #6366f1)' }} />
            <span>Nhật Ký Hệ Thống (Console Log)</span>
          </h2>
          <span style={{ fontSize: 13, color: 'var(--text-muted)' }}>
            Theo dõi real-time log hệ thống, quota scheduler, và routing pipeline từ ezRouter
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
          {/* Auto Refresh selector */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Tự động:</span>
            <select
              value={autoRefreshSec}
              onChange={(e) => setAutoRefreshSec(Number(e.target.value))}
              style={{
                fontSize: 12,
                padding: '4px 8px',
                borderRadius: 'var(--radius-sm, 6px)',
                background: 'var(--bg-card)',
                color: 'var(--text-primary)',
                border: '1px solid var(--border-subtle)',
              }}
            >
              <option value={0}>Tắt</option>
              <option value={2}>2 giây</option>
              <option value={3}>3 giây</option>
              <option value={5}>5 giây</option>
              <option value={10}>10 giây</option>
            </select>
          </div>

          {/* Manual Refresh */}
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => fetchLogs(false)}
            disabled={refreshing || loading}
            title="Làm mới log"
          >
            <IconRefresh size={14} className={refreshing ? 'spin' : ''} />
            <span>{refreshing ? 'Đang tải...' : 'Làm Mới'}</span>
          </button>

          {/* Copy logs */}
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={handleCopyLogs}
            disabled={logs.length === 0}
            title="Sao chép toàn bộ log đang hiển thị"
          >
            {copied ? <IconCheck size={14} style={{ color: '#10b981' }} /> : <IconCopy size={14} />}
            <span>{copied ? 'Đã sao chép' : 'Sao Chép'}</span>
          </button>

          {/* Clear logs */}
          {clearConfirm ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <button
                type="button"
                className="btn btn-danger btn-sm"
                onClick={handleClearLogs}
              >
                Xác nhận xóa
              </button>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setClearConfirm(false)}
              >
                Hủy
              </button>
            </div>
          ) : (
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => setClearConfirm(true)}
              title="Xóa bộ đệm log trong RAM"
            >
              <IconTrash size={14} />
              <span>Xóa Màn Hình</span>
            </button>
          )}
        </div>
      </div>

      {error && (
        <div className="alert alert-error">
          <IconAlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      {/* Control bar */}
      <div
        className="card"
        style={{
          padding: '12px 16px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: 12,
        }}
      >
        {/* Level Filters */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
          <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-secondary)' }}>Mức độ:</span>
          {(['all', 'INFO', 'WARN', 'ERROR', 'DEBUG'] as const).map((lvl) => {
            const isActive = levelFilter === lvl;
            let badgeCount = total;
            if (lvl === 'INFO') badgeCount = countInfo;
            if (lvl === 'WARN') badgeCount = countWarn;
            if (lvl === 'ERROR') badgeCount = countError;
            if (lvl === 'DEBUG') badgeCount = countDebug;

            return (
              <button
                key={lvl}
                type="button"
                onClick={() => setLevelFilter(lvl)}
                className={`btn btn-sm ${isActive ? 'btn-primary' : 'btn-secondary'}`}
                style={{
                  fontSize: 12,
                  padding: '4px 10px',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span>{lvl === 'all' ? 'Tất Cả' : lvl}</span>
                <span
                  style={{
                    fontSize: 10,
                    opacity: 0.85,
                    padding: '1px 5px',
                    borderRadius: 10,
                    background: isActive ? 'rgba(255,255,255,0.25)' : 'var(--bg-input)',
                  }}
                >
                  {badgeCount}
                </span>
              </button>
            );
          })}
        </div>

        {/* Search & Auto-scroll options */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
          {/* Search box */}
          <div style={{ position: 'relative', width: 220 }}>
            <input
              type="text"
              placeholder="Lọc từ khóa / module..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              style={{
                width: '100%',
                padding: '6px 12px 6px 30px',
                fontSize: 12,
                borderRadius: 'var(--radius-sm, 6px)',
                background: 'var(--bg-input)',
                color: 'var(--text-primary)',
                border: '1px solid var(--border-subtle)',
              }}
            />
            <IconSearch
              size={13}
              style={{
                position: 'absolute',
                left: 10,
                top: '50%',
                transform: 'translateY(-50%)',
                color: 'var(--text-muted)',
              }}
            />
          </div>

          {/* Limit selector */}
          <select
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value))}
            style={{
              fontSize: 12,
              padding: '6px 8px',
              borderRadius: 'var(--radius-sm, 6px)',
              background: 'var(--bg-input)',
              color: 'var(--text-primary)',
              border: '1px solid var(--border-subtle)',
            }}
          >
            <option value={50}>50 dòng</option>
            <option value={100}>100 dòng</option>
            <option value={200}>200 dòng</option>
            <option value={500}>500 dòng</option>
          </select>

          {/* Auto-scroll toggle */}
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              fontSize: 12,
              color: 'var(--text-secondary)',
              cursor: 'pointer',
              userSelect: 'none',
            }}
          >
            <input
              type="checkbox"
              checked={autoScroll}
              onChange={(e) => setAutoScroll(e.target.checked)}
              style={{ cursor: 'pointer' }}
            />
            <span>Tự cuộn xuống</span>
          </label>
        </div>
      </div>

      {/* Console Output Screen */}
      <div
        ref={consoleContainerRef}
        className="card"
        style={{
          padding: 0,
          background: '#090d16',
          borderRadius: 'var(--radius-md, 8px)',
          border: '1px solid var(--border-subtle, rgba(255,255,255,0.08))',
          overflow: 'hidden',
          display: 'flex',
          flexDirection: 'column',
          minHeight: 480,
          maxHeight: 'calc(100vh - 270px)',
        }}
      >
        {/* Terminal Header bar */}
        <div
          style={{
            padding: '8px 16px',
            background: '#0d1322',
            borderBottom: '1px solid rgba(255,255,255,0.06)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            fontFamily: 'var(--font-mono, monospace)',
            fontSize: 11.5,
            color: 'var(--text-muted, #94a3b8)',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <span style={{ display: 'inline-block', width: 10, height: 10, borderRadius: '50%', background: '#ef4444' }} />
            <span style={{ display: 'inline-block', width: 10, height: 10, borderRadius: '50%', background: '#f59e0b' }} />
            <span style={{ display: 'inline-block', width: 10, height: 10, borderRadius: '50%', background: '#10b981' }} />
            <span style={{ marginLeft: 8, color: '#e2e8f0', fontWeight: 600 }}>ezRouter System Output</span>
          </div>
          <div>
            <span>Hiển thị: </span>
            <strong style={{ color: '#38bdf8' }}>{logs.length}</strong>
            <span> / {total} sự kiện</span>
          </div>
        </div>

        {/* Terminal Body */}
        <div
          style={{
            padding: '12px 16px',
            flex: 1,
            overflowY: 'auto',
            overflowX: 'auto',
            maxWidth: '100%',
            WebkitOverflowScrolling: 'touch',
            fontFamily: 'var(--font-mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace)',
            fontSize: 12.5,
            lineHeight: 1.6,
            color: '#e2e8f0',
          }}
        >
          {loading && logs.length === 0 ? (
            <div style={{ textAlign: 'center', padding: 48, color: 'var(--text-muted)' }}>
              <div className="spinner" style={{ margin: '0 auto 12px' }} />
              <p>Đang tải log hệ thống...</p>
            </div>
          ) : logs.length === 0 ? (
            <div style={{ textAlign: 'center', padding: 48, color: 'var(--text-muted)' }}>
              <p>Không có log nào phù hợp với bộ lọc hiện tại.</p>
              {searchQuery && (
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  style={{ marginTop: 8 }}
                  onClick={() => setSearchQuery('')}
                >
                  Xóa bộ lọc tìm kiếm
                </button>
              )}
            </div>
          ) : (
            logs.map((log) => {
              const lvl = log.level.toUpperCase();
              let levelColor = '#38bdf8'; // INFO - cyan
              let levelBg = 'rgba(56, 189, 248, 0.12)';
              if (lvl === 'WARN') {
                levelColor = '#f59e0b'; // amber
                levelBg = 'rgba(245, 158, 11, 0.15)';
              } else if (lvl === 'ERROR') {
                levelColor = '#f43f5e'; // red/rose
                levelBg = 'rgba(244, 63, 94, 0.18)';
              } else if (lvl === 'DEBUG') {
                levelColor = '#a855f7'; // purple
                levelBg = 'rgba(168, 85, 247, 0.12)';
              }

              // Extract time part for compact display
              let timeStr = log.timestamp;
              if (timeStr.includes('T')) {
                timeStr = timeStr.split('T')[1]?.replace('Z', '') || timeStr;
              }

              return (
                <div
                  key={log.id}
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 10,
                    padding: '3px 0',
                    borderBottom: '1px solid rgba(255,255,255,0.02)',
                    wordBreak: 'break-word',
                  }}
                >
                  {/* Timestamp */}
                  <span
                    style={{
                      color: '#64748b',
                      fontSize: 11.5,
                      whiteSpace: 'nowrap',
                      userSelect: 'none',
                      flexShrink: 0,
                    }}
                  >
                    {timeStr}
                  </span>

                  {/* Level Badge */}
                  <span
                    style={{
                      fontSize: 10.5,
                      fontWeight: 700,
                      padding: '1px 6px',
                      borderRadius: 4,
                      color: levelColor,
                      backgroundColor: levelBg,
                      whiteSpace: 'nowrap',
                      flexShrink: 0,
                      textTransform: 'uppercase',
                      letterSpacing: '0.04em',
                    }}
                  >
                    {lvl}
                  </span>

                  {/* Target/Module */}
                  <span
                    style={{
                      color: '#a78bfa',
                      fontSize: 11.5,
                      whiteSpace: 'nowrap',
                      flexShrink: 0,
                    }}
                  >
                    [{log.target}]:
                  </span>

                  {/* Message */}
                  <span
                    style={{
                      color: lvl === 'ERROR' ? '#fca5a5' : lvl === 'WARN' ? '#fde68a' : '#f1f5f9',
                      flex: 1,
                      whiteSpace: 'pre-wrap',
                    }}
                  >
                    {log.message}
                  </span>
                </div>
              );
            })
          )}
          <div ref={consoleEndRef} />
        </div>
      </div>
    </div>
  );
};
