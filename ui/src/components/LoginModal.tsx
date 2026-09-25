import React, { useState } from 'react';
import { api, setStoredAdminKey } from '../api';
import { IconKey, IconAlertCircle, EzRouterMark } from '../icons';

interface LoginModalProps {
  onSuccess: (key: string) => void;
}

export const LoginModal: React.FC<LoginModalProps> = ({ onSuccess }) => {
  const [keyInput, setKeyInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = keyInput.trim();
    if (!trimmed) {
      setError('Vui lòng nhập Khóa Quản Trị API.');
      return;
    }

    setLoading(true);
    setError(null);
    try {
      // Test key against /admin/stats
      await api.getStats(trimmed);
      setStoredAdminKey(trimmed);
      onSuccess(trimmed);
    } catch (err: any) {
      setError(err?.message || 'Khóa quản trị không hợp lệ hoặc máy chủ từ chối kết nối.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="modal-backdrop">
      <div className="modal-card login-card" style={{ maxWidth: 420 }}>
        <div style={{ textAlign: 'center', marginBottom: 24 }}>
          <div
            style={{
              display: 'inline-flex',
              padding: 10,
              borderRadius: 14,
              background: 'rgba(16, 185, 129, 0.1)',
              border: '1px solid rgba(16, 185, 129, 0.25)',
              boxShadow: '0 8px 24px rgba(16, 185, 129, 0.15)',
              marginBottom: 14,
            }}
          >
            <EzRouterMark size={40} />
          </div>
          <h2 style={{ fontSize: 20, fontWeight: 700, color: '#f8fafc', letterSpacing: '-0.02em' }}>
            Đăng Nhập <span style={{ color: '#10b981' }}>ezRouter</span>
          </h2>
          <p style={{ fontSize: 13, color: '#94a3b8', marginTop: 4 }}>
            Nhập khóa quản trị (Admin Key) để mở bảng điều khiển
          </p>
        </div>

        {error && (
          <div className="alert alert-error">
            <IconAlertCircle size={16} />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label htmlFor="adminKey">Khóa Quản Trị (Admin Bearer Key)</label>
            <div style={{ position: 'relative' }}>
              <input
                id="adminKey"
                type="password"
                placeholder="sk-... hoặc admin key"
                value={keyInput}
                onChange={(e) => setKeyInput(e.target.value)}
                autoFocus
                style={{ width: '100%', paddingLeft: 38 }}
              />
              <span
                style={{
                  position: 'absolute',
                  left: 12,
                  top: '50%',
                  transform: 'translateY(-50%)',
                  color: '#10b981',
                  display: 'flex',
                  alignItems: 'center',
                }}
              >
                <IconKey size={16} />
              </span>
            </div>
            <span style={{ fontSize: 12, color: '#64748b', marginTop: 6, display: 'block' }}>
              Khóa được lưu bảo mật trong bộ nhớ trình duyệt cục bộ.
            </span>
          </div>

          <button
            type="submit"
            className="btn btn-primary"
            style={{ width: '100%', marginTop: 8 }}
            disabled={loading}
          >
            {loading ? <div className="spinner" /> : 'Xác Thực & Truy Cập'}
          </button>
        </form>
      </div>
    </div>
  );
};
