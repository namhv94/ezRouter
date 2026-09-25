import React, { useState } from 'react';
import { ApiKey } from '../types';
import { api } from '../api';
import {
  IconPlus,
  IconCheck,
  IconAlertCircle,
  IconTrash,
  IconKey,
  IconX,
} from '../icons';

interface ApiKeysTabProps {
  apiKeys: ApiKey[];
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

export const ApiKeysTab: React.FC<ApiKeysTabProps> = ({
  apiKeys,
  loading,
  error,
  onRefresh,
}) => {
  const [showAddModal, setShowAddModal] = useState(false);
  const [name, setName] = useState('');
  const [customKey, setCustomKey] = useState('');

  const [createdKeySecret, setCreatedKeySecret] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState(false);
  const [actionMessage, setActionMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(
    null
  );

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setActionMessage({ type: 'error', text: 'Vui lòng nhập tên định danh cho khóa API.' });
      return;
    }

    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.createApiKey({
        name: name.trim(),
        key: customKey.trim() || undefined,
      });
      setCreatedKeySecret(res.key);
      setActionMessage({ type: 'success', text: `Đã tạo khóa API "${res.name}" thành công.` });
      setName('');
      setCustomKey('');
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi khi tạo khóa API.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleToggle = async (key: ApiKey) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.updateApiKey(key.id, !key.is_active);
      setActionMessage({
        type: 'success',
        text: `Đã ${key.is_active ? 'vô hiệu hóa' : 'kích hoạt'} khóa "${key.name}".`,
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể cập nhật trạng thái khóa.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (key: ApiKey) => {
    if (!window.confirm(`Xác nhận xóa khóa API "${key.name}"? Yêu cầu sử dụng khóa này sẽ bị chặn.`)) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteApiKey(key.id);
      setActionMessage({ type: 'success', text: `Đã xóa khóa API "${key.name}".` });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể xóa khóa API.' });
    } finally {
      setActionLoading(false);
    }
  };

  return (
    <div>
      <div className="section-header">
        <div>
          <h2 className="section-title">Quản Lý Khóa API Khách Hàng (API Keys)</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            Quản lý và cấp quyền truy cập tới proxy qua Authorization: Bearer &lt;key&gt;
          </span>
        </div>
        <button className="btn btn-primary" onClick={() => setShowAddModal(true)}>
          <IconPlus size={16} />
          <span>Tạo Khóa Mới</span>
        </button>
      </div>

      {actionMessage && (
        <div className={`alert ${actionMessage.type === 'success' ? 'alert-success' : 'alert-error'}`}>
          {actionMessage.type === 'success' ? <IconCheck size={16} /> : <IconAlertCircle size={16} />}
          <span>{actionMessage.text}</span>
        </div>
      )}

      {error && (
        <div className="alert alert-error">
          <IconAlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      {/* Newly Created Key Alert */}
      {createdKeySecret && (
        <div
          className="alert alert-info"
          style={{ flexDirection: 'column', alignItems: 'flex-start', gap: 8 }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontWeight: 600 }}>
            <IconKey size={16} />
            <span>Khóa API vừa tạo (Hãy lưu lại ngay vì khóa sẽ được ẩn sau khi tải lại trang):</span>
          </div>
          <div
            style={{
              backgroundColor: 'var(--bg-app)',
              padding: '8px 12px',
              borderRadius: 'var(--radius-sm)',
              fontFamily: 'var(--font-mono)',
              fontSize: 13,
              color: '#93c5fd',
              width: '100%',
              userSelect: 'all',
              wordBreak: 'break-all',
              overflowWrap: 'anywhere',
            }}
          >
            {createdKeySecret}
          </div>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => setCreatedKeySecret(null)}
            style={{ marginTop: 4 }}
          >
            Tôi đã lưu khóa an toàn
          </button>
        </div>
      )}

      <div className="card">
        {loading && apiKeys.length === 0 ? (
          <div className="state-container">
            <div className="spinner" />
            <p>Đang tải danh sách khóa API...</p>
          </div>
        ) : apiKeys.length === 0 ? (
          <div className="state-container">
            <p>Chưa có khóa API nào trong hệ thống.</p>
            <button className="btn btn-secondary btn-sm" onClick={() => setShowAddModal(true)}>
              Tạo khóa API đầu tiên
            </button>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Tên Định Danh</th>
                  <th>Khóa API (Đã Ẩn)</th>
                  <th>Trạng Thái</th>
                  <th>Tổng Yêu Cầu</th>
                  <th>Ngày Tạo</th>
                  <th>Hành Động</th>
                </tr>
              </thead>
              <tbody>
                {apiKeys.map((k) => (
                  <tr key={k.id}>
                    <td>
                      <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>{k.name}</div>
                      <div className="badge badge-neutral font-mono" style={{ fontSize: 11, marginTop: 4 }}>
                        id: {k.id.slice(0, 8)}...
                      </div>
                    </td>
                    <td className="font-mono text-break" style={{ fontSize: 12 }}>
                      {k.key}
                    </td>
                    <td>
                      <span className={`badge ${k.is_active ? 'badge-success' : 'badge-neutral'}`}>
                        {k.is_active ? 'Hoạt động' : 'Đã khóa'}
                      </span>
                    </td>
                    <td className="font-mono">{k.total_requests.toLocaleString('vi-VN')}</td>
                    <td className="font-mono" style={{ fontSize: 12 }}>
                      {k.created_at}
                    </td>
                    <td>
                      <div className="table-actions">
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleToggle(k)}
                          disabled={actionLoading}
                        >
                          {k.is_active ? 'Khóa' : 'Mở'}
                        </button>
                        <button
                          className="btn btn-danger btn-sm btn-icon-only"
                          onClick={() => handleDelete(k)}
                          disabled={actionLoading}
                          title="Xóa khóa"
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

      {/* Add API Key Modal */}
      {showAddModal && (
        <div className="modal-backdrop" onClick={() => setShowAddModal(false)}>
          <div className="modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">Tạo Khóa API Mới</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleCreate}>
              <div className="form-group">
                <label>Tên Ứng Dụng / Người Dùng (Name)</label>
                <input
                  type="text"
                  placeholder="VD: App Frontend, CLI Dev, Bot Telegram..."
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                  autoFocus
                />
              </div>

              <div className="form-group">
                <label>Khóa Tùy Chọn (Để trống để tự động sinh ngẫu nhiên sk-...)</label>
                <input
                  type="text"
                  placeholder="sk-my-custom-key..."
                  value={customKey}
                  onChange={(e) => setCustomKey(e.target.value)}
                />
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
                  {actionLoading ? <div className="spinner" /> : 'Tạo Khóa'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
