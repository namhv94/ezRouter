import React, { useState, useEffect } from 'react';
import { CodexAccountRecord, CodexStatusResponse } from '../types';
import { api } from '../api';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconTrash,
  IconCodex,
} from '../icons';

export const CodexTab: React.FC = () => {
  const [status, setStatus] = useState<CodexStatusResponse | null>(null);
  const [accounts, setAccounts] = useState<CodexAccountRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [showAddModal, setShowAddModal] = useState(false);
  const [showOAuthModal, setShowOAuthModal] = useState(false);

  // Form fields
  const [authPath, setAuthPath] = useState('');
  const [email, setEmail] = useState('');

  // OAuth fields
  const [oauthTicket, setOauthTicket] = useState<{ ticket_id: string; authorize_url: string } | null>(
    null
  );
  const [oauthCode, setOauthCode] = useState('');

  const [actionLoading, setActionLoading] = useState(false);
  const [actionMessage, setActionMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(
    null
  );

  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [statusRes, accountsRes] = await Promise.all([
        api.getCodexStatus(),
        api.getCodexAccounts(),
      ]);
      setStatus(statusRes);
      const accList = Array.isArray(accountsRes)
        ? accountsRes
        : (accountsRes as any)?.accounts || [];
      setAccounts(accList);
    } catch (err: any) {
      setError(err?.message || 'Không thể tải thông tin Codex');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleAddAccount = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authPath.trim()) {
      setActionMessage({ type: 'error', text: 'Vui lòng cung cấp đường dẫn tệp auth (auth_path).' });
      return;
    }

    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.createCodexAccount({
        auth_path: authPath.trim(),
        email: email.trim() || undefined,
      });
      setActionMessage({ type: 'success', text: 'Đã thêm tài khoản Codex thành công.' });
      setShowAddModal(false);
      setAuthPath('');
      setEmail('');
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi thêm tài khoản Codex.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleToggle = async (acc: CodexAccountRecord) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.toggleCodexAccount(acc.id);
      setActionMessage({
        type: 'success',
        text: `Đã ${acc.is_active ? 'tắt' : 'bật'} tài khoản Codex "${acc.email || acc.id}".`,
      });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi thay đổi trạng thái tài khoản.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleReset = async (acc: CodexAccountRecord) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.resetCodexAccount(acc.id);
      setActionMessage({ type: 'success', text: `Đã reset cooldown tài khoản "${acc.email || acc.id}".` });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi reset cooldown.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (acc: CodexAccountRecord) => {
    if (!window.confirm(`Xác nhận xóa tài khoản Codex "${acc.email || acc.id}"?`)) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteCodexAccount(acc.id);
      setActionMessage({ type: 'success', text: `Đã xóa tài khoản Codex.` });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể xóa tài khoản Codex.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleStartOAuth = async () => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.startCodexOAuth();
      setOauthTicket(res);
      setShowOAuthModal(true);
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể khởi tạo luồng OAuth.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleExchangeOAuth = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!oauthTicket || !oauthCode.trim()) return;

    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.exchangeCodexOAuth(oauthTicket.ticket_id, oauthCode.trim());
      setActionMessage({ type: 'success', text: 'Xác thực OAuth Codex hoàn tất và đã thêm vào pool.' });
      setShowOAuthModal(false);
      setOauthTicket(null);
      setOauthCode('');
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Đổi mã xác thực OAuth thất bại.' });
    } finally {
      setActionLoading(false);
    }
  };

  return (
    <div>
      <div className="section-header">
        <div>
          <h2 className="section-title">Quản Lý Pool Tài Khoản OpenAI Codex</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            Hệ thống phân phối và bảo vệ luồng truy cập qua token Codex / OAuth
          </span>
        </div>
        <div className="section-actions">
          <button className="btn btn-secondary" onClick={handleStartOAuth} disabled={actionLoading}>
            <span>Khởi Tạo OAuth</span>
          </button>
          <button className="btn btn-primary" onClick={() => setShowAddModal(true)}>
            <IconPlus size={16} />
            <span>Thêm Tài Khoản</span>
          </button>
        </div>
      </div>

      {/* KPI Cards for Codex */}
      <div className="kpi-grid" style={{ marginBottom: 20 }}>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>Tổng Tài Khoản Codex</span>
            <IconCodex size={16} />
          </div>
          <div className="kpi-value">{status?.total_accounts ?? accounts.length}</div>
        </div>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>Đang Hoạt Động</span>
            <span className="badge badge-success">Active</span>
          </div>
          <div className="kpi-value" style={{ color: '#10b981' }}>
            {status?.active_accounts ?? 0}
          </div>
        </div>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>Đang Chờ (Cooldown)</span>
            <span className="badge badge-warning">Cooldown</span>
          </div>
          <div className="kpi-value" style={{ color: (status?.cooldown_accounts ?? 0) > 0 ? '#f59e0b' : 'inherit' }}>
            {status?.cooldown_accounts ?? 0}
          </div>
        </div>
      </div>

      {actionMessage && (
        <div className={`alert ${actionMessage.type === 'success' ? 'alert-success' : 'alert-error'}`}>
          {actionMessage.type === 'success' ? <IconCheck size={16} /> : <IconAlertCircle size={16} />}
          <span>{actionMessage.text}</span>
        </div>
      )}

      {error && (
        <div className="alert alert-error">
          <IconAlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      <div className="card">
        {loading && accounts.length === 0 ? (
          <div className="state-container">
            <div className="spinner" />
            <p>Đang tải danh sách tài khoản Codex...</p>
          </div>
        ) : accounts.length === 0 ? (
          <div className="state-container">
            <p>Chưa có tài khoản Codex nào trong hệ thống.</p>
            <button className="btn btn-secondary btn-sm" onClick={() => setShowAddModal(true)}>
              Thêm tài khoản đầu tiên
            </button>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Email & ID</th>
                  <th>Đường Dẫn Xác Thực (Auth Path)</th>
                  <th>Trạng Thái</th>
                  <th>Lỗi Gần Nhất</th>
                  <th>Hành Động</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((acc) => (
                  <tr key={acc.id}>
                    <td>
                      <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                        {acc.email || 'Chưa định danh'}
                      </div>
                      <div className="badge badge-neutral font-mono" style={{ fontSize: 11, marginTop: 4 }}>
                        id: {acc.id.slice(0, 10)}...
                      </div>
                    </td>
                    <td className="font-mono" style={{ fontSize: 12, maxWidth: 260, wordBreak: 'break-all' }}>
                      {acc.auth_path || acc.account_id_suffix || '—'}
                    </td>
                    <td>
                      <span className={`badge ${acc.is_active ?? acc.active ? 'badge-success' : 'badge-neutral'}`}>
                        {acc.is_active ?? acc.active ? 'Kích hoạt' : 'Tạm dừng'}
                      </span>
                    </td>
                    <td>
                      {acc.last_error ? (
                        <div
                          style={{
                            fontSize: 11,
                            color: '#f87171',
                            maxWidth: 180,
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}
                          title={acc.last_error}
                        >
                          {acc.last_error}
                        </div>
                      ) : (
                        <span style={{ color: 'var(--text-muted)', fontSize: 12 }}>Bình thường</span>
                      )}
                    </td>
                    <td>
                      <div className="table-actions">
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleToggle(acc)}
                          disabled={actionLoading}
                        >
                          {acc.is_active ? 'Tắt' : 'Bật'}
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleReset(acc)}
                          disabled={actionLoading}
                          title="Reset cooldown"
                        >
                          Reset
                        </button>
                        <button
                          className="btn btn-danger btn-sm btn-icon-only"
                          onClick={() => handleDelete(acc)}
                          disabled={actionLoading}
                          title="Xóa tài khoản"
                        >
                          <IconTrash size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Add Codex Modal */}
      {showAddModal && (
        <div className="modal-backdrop" onClick={() => setShowAddModal(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">Thêm Tài Khoản Codex Mới</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleAddAccount}>
              <div className="form-group">
                <label>Email Người Dùng (tùy chọn)</label>
                <input
                  type="text"
                  placeholder="user@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label>Đường Dẫn Tệp Auth (auth_path)</label>
                <input
                  type="text"
                  placeholder="/home/user/.codex/auth.json"
                  value={authPath}
                  onChange={(e) => setAuthPath(e.target.value)}
                  required
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  Đường dẫn tệp JSON lưu trữ token và refresh token của Codex.
                </span>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowAddModal(false)}
                >
                  Hủy
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : 'Lưu Tài Khoản'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* OAuth Modal */}
      {showOAuthModal && oauthTicket && (
        <div className="modal-backdrop" onClick={() => setShowOAuthModal(false)}>
          <div className="modal-card" style={{ maxWidth: 540 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">Xác Thực Codex Qua OAuth</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowOAuthModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <div>
              <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
                1. Mở liên kết bên dưới trên trình duyệt để cấp quyền tài khoản OpenAI/Codex:
              </p>
              <div className="oauth-url-box">
                <a
                  href={oauthTicket.authorize_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: '#818cf8', fontSize: 12 }}
                >
                  {oauthTicket.authorize_url}
                </a>
              </div>

              <form onSubmit={handleExchangeOAuth}>
                <div className="form-group">
                  <label>2. Dán mã Authorization Code hoặc URL trả về vào đây:</label>
                  <input
                    type="text"
                    placeholder="Nhập code từ URL chuyển hướng..."
                    value={oauthCode}
                    onChange={(e) => setOauthCode(e.target.value)}
                    required
                  />
                </div>

                <div className="modal-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowOAuthModal(false)}
                  >
                    Hủy
                  </button>
                  <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                    {actionLoading ? <div className="spinner" /> : 'Hoàn Tất Xác Thực'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
