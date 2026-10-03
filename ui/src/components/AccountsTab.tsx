import React, { useState } from 'react';
import { AccountResponse } from '../types';
import { api } from '../api';
import { useI18n } from '../i18n';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconRefresh,
  IconTrash,
  IconUsers,
} from '../icons';

interface AccountsTabProps {
  accounts: AccountResponse[];
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

export const AccountsTab: React.FC<AccountsTabProps> = ({
  accounts,
  loading,
  error,
  onRefresh,
}) => {
  const { t } = useI18n();
  const [showAddModal, setShowAddModal] = useState(false);
  const [email, setEmail] = useState('');
  const [refreshToken, setRefreshToken] = useState('');

  const [actionLoading, setActionLoading] = useState(false);
  const [actionMessage, setActionMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(
    null
  );

  const [selectedQuota, setSelectedQuota] = useState<{ email: string; quota: any } | null>(null);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!refreshToken.trim()) {
      setActionMessage({ type: 'error', text: t('accounts.msgTokenRequired') });
      return;
    }

    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.createAccount({
        email: email.trim() || undefined,
        refresh_token: refreshToken.trim(),
      });
      setActionMessage({ type: 'success', text: t('accounts.msgAddSuccess') });
      setShowAddModal(false);
      setEmail('');
      setRefreshToken('');
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('accounts.msgAddError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleResetCooldown = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.resetAccount(acc.id);
      setActionMessage({ type: 'success', text: t('accounts.msgResetSuccess', { email: acc.email }) });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('accounts.msgResetError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleRefreshQuota = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.refreshAccountQuota(acc.id);
      setActionMessage({ type: 'success', text: t('accounts.msgRefreshSuccess', { email: acc.email }) });
      setSelectedQuota({ email: acc.email, quota: res.quota });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('accounts.msgRefreshError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleTest = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.testAccount(acc.id);
      setActionMessage({
        type: 'success',
        text: t('accounts.msgTestSuccess', { email: acc.email, detail: JSON.stringify(res) }),
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: t('accounts.msgTestError', { email: acc.email, error: err?.message || 'Lỗi xác thực' }),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (acc: AccountResponse) => {
    if (!window.confirm(t('accounts.confirmDelete', { email: acc.email }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteAccount(acc.id);
      setActionMessage({ type: 'success', text: t('accounts.msgDeleteSuccess', { email: acc.email }) });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('accounts.msgDeleteError') });
    } finally {
      setActionLoading(false);
    }
  };

  const activeCount = accounts.filter((a) => a.is_active && a.cooldown_remaining <= 0).length;
  const cooldownCount = accounts.filter((a) => a.cooldown_remaining > 0).length;

  return (
    <div>
      <div className="section-header">
        <div>
          <h2 className="section-title">{t('accounts.title')}</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('accounts.subtitle')}
          </span>
        </div>
        <button className="btn btn-primary" onClick={() => setShowAddModal(true)}>
          <IconPlus size={16} />
          <span>{t('accounts.addAccount')}</span>
        </button>
      </div>

      {/* Pool Health Summary Bar */}
      <div className="kpi-grid" style={{ marginBottom: 20 }}>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>{t('accounts.kpiTotalAccounts')}</span>
            <IconUsers size={16} />
          </div>
          <div className="kpi-value">{accounts.length}</div>
        </div>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>{t('accounts.kpiReady')}</span>
            <span className="badge badge-success">{t('accounts.badgeActive')}</span>
          </div>
          <div className="kpi-value" style={{ color: '#10b981' }}>
            {activeCount}
          </div>
        </div>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>{t('accounts.kpiCooldown')}</span>
            <span className="badge badge-warning">{t('accounts.badgeWaiting')}</span>
          </div>
          <div className="kpi-value" style={{ color: cooldownCount > 0 ? '#f59e0b' : 'inherit' }}>
            {cooldownCount}
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
          <IconAlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      <div className="card">
        {loading && accounts.length === 0 ? (
          <div className="state-container">
            <div className="spinner" />
            <p>{t('accounts.loading')}</p>
          </div>
        ) : accounts.length === 0 ? (
          <div className="state-container">
            <p>{t('accounts.empty')}</p>
            <button className="btn btn-secondary btn-sm" onClick={() => setShowAddModal(true)}>
              {t('accounts.btnAddGoogle')}
            </button>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>{t('accounts.thEmailId')}</th>
                  <th>{t('accounts.thStatus')}</th>
                  <th>{t('accounts.thCooldown')}</th>
                  <th>{t('accounts.thTotalRequests')}</th>
                  <th>{t('accounts.thErrors')}</th>
                  <th>{t('accounts.thRecentRpm')}</th>
                  <th>{t('accounts.thQuota')}</th>
                  <th>{t('accounts.thActions')}</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((acc) => {
                  const isCooling = acc.cooldown_remaining > 0;
                  return (
                    <tr key={acc.id}>
                      <td>
                        <div style={{ fontWeight: 600, color: 'var(--text-primary)' }} className="text-break">{acc.email}</div>
                        <div className="badge badge-neutral font-mono" style={{ fontSize: 11, marginTop: 4 }}>
                          id: {acc.id.slice(0, 10)}...
                        </div>
                      </td>
                      <td>
                        <span className={`badge ${acc.is_active ? 'badge-success' : 'badge-neutral'}`}>
                          {acc.is_active ? t('accounts.statusActive') : t('accounts.statusInactive')}
                        </span>
                      </td>
                      <td>
                        {isCooling ? (
                          <span className="badge badge-warning font-mono">
                            {t('accounts.cooldownWait', { sec: Math.ceil(acc.cooldown_remaining) })}
                          </span>
                        ) : (
                          <span className="badge badge-success">{t('accounts.statusReady')}</span>
                        )}
                      </td>
                      <td className="font-mono">{acc.total_requests}</td>
                      <td>
                        <span className={`badge ${acc.error_count > 0 ? 'badge-error' : 'badge-neutral'} font-mono`}>
                          {acc.error_count}
                        </span>
                        {acc.last_error && (
                          <div
                            style={{
                              fontSize: 11,
                              color: '#f87171',
                              maxWidth: 160,
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                              marginTop: 2,
                            }}
                            title={acc.last_error}
                          >
                            {acc.last_error}
                          </div>
                        )}
                      </td>
                      <td className="font-mono">{acc.recent_rpm} rpm</td>
                      <td>
                        {acc.quota && Object.keys(acc.quota).length > 0 ? (
                          <button
                            className="btn btn-secondary btn-sm font-mono"
                            style={{ padding: '2px 8px', fontSize: 11 }}
                            onClick={() => setSelectedQuota({ email: acc.email, quota: acc.quota })}
                          >
                            {t('accounts.viewQuota')}
                          </button>
                        ) : (
                          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>{t('accounts.quotaNotLoaded')}</span>
                        )}
                      </td>
                      <td>
                        <div className="table-actions">
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => handleTest(acc)}
                            disabled={actionLoading}
                            title={t('accounts.testTitle')}
                          >
                            {t('accounts.testBtn')}
                          </button>
                          {isCooling && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => handleResetCooldown(acc)}
                              disabled={actionLoading}
                              title={t('accounts.resetTitle')}
                            >
                              {t('accounts.resetBtn')}
                            </button>
                          )}
                          <button
                            className="btn btn-secondary btn-sm btn-icon-only"
                            onClick={() => handleRefreshQuota(acc)}
                            disabled={actionLoading}
                            title={t('accounts.refreshQuotaTitle')}
                            aria-label={t('accounts.refreshQuotaTitle')}
                          >
                            <IconRefresh size={14} />
                          </button>
                          <button
                            className="btn btn-danger btn-sm btn-icon-only"
                            onClick={() => handleDelete(acc)}
                            disabled={actionLoading}
                            title={t('accounts.deleteTitle')}
                            aria-label={t('accounts.deleteTitle')}
                          >
                            <IconTrash size={14} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Add Account Modal */}
      {showAddModal && (
        <div className="modal-backdrop" onClick={() => setShowAddModal(false)}>
          <div className="modal-card" style={{ maxWidth: 500 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">{t('accounts.modalAddTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddModal(false)}
                aria-label={t('actions.close')}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleCreate}>
              <div className="form-group">
                <label>{t('accounts.labelEmail')}</label>
                <input
                  type="text"
                  placeholder="user@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label>{t('accounts.labelRefreshToken')}</label>
                <textarea
                  rows={4}
                  placeholder="1//04..."
                  value={refreshToken}
                  onChange={(e) => setRefreshToken(e.target.value)}
                  required
                  style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  {t('accounts.tokenSecurityNote')}
                </span>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowAddModal(false)}
                >
                  {t('actions.cancel')}
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : t('accounts.addToPool')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Quota Modal */}
      {selectedQuota && (
        <div className="modal-backdrop" onClick={() => setSelectedQuota(null)}>
          <div className="modal-card" style={{ maxWidth: 560 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">{t('accounts.modalQuotaTitle', { email: selectedQuota.email })}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setSelectedQuota(null)}
                aria-label={t('actions.close')}
              >
                <IconX size={16} />
              </button>
            </div>
            <pre className="code-viewer-box">
              {JSON.stringify(selectedQuota.quota, null, 2)}
            </pre>
            <div className="modal-actions">
              <button className="btn btn-secondary btn-sm" onClick={() => setSelectedQuota(null)}>
                {t('actions.close')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
