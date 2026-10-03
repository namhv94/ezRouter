import React, { useState, useEffect } from 'react';
import { CodexAccountRecord, CodexStatusResponse } from '../types';
import { api } from '../api';
import { useI18n } from '../i18n';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconTrash,
  IconCodex,
} from '../icons';

export const CodexTab: React.FC = () => {
  const { t } = useI18n();
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
      setError(err?.message || t('codex.msgLoadError'));
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
      setActionMessage({ type: 'error', text: t('codex.msgAuthPathRequired') });
      return;
    }

    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.createCodexAccount({
        auth_path: authPath.trim(),
        email: email.trim() || undefined,
      });
      setActionMessage({ type: 'success', text: t('codex.msgAddSuccess') });
      setShowAddModal(false);
      setAuthPath('');
      setEmail('');
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('codex.msgAddError') });
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
        text: t('codex.msgToggleSuccess', {
          action: acc.is_active ? t('codex.btnTurnOff').toLowerCase() : t('codex.btnTurnOn').toLowerCase(),
          name: acc.email || acc.id,
        }),
      });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('codex.msgToggleError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleReset = async (acc: CodexAccountRecord) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.resetCodexAccount(acc.id);
      setActionMessage({ type: 'success', text: t('codex.msgResetSuccess', { name: acc.email || acc.id }) });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('codex.msgResetError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (acc: CodexAccountRecord) => {
    if (!window.confirm(t('codex.confirmDelete', { name: acc.email || acc.id }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteCodexAccount(acc.id);
      setActionMessage({ type: 'success', text: t('codex.msgDeleteSuccess') });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('codex.msgDeleteError') });
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
      setActionMessage({ type: 'error', text: err?.message || t('codex.msgOAuthInitError') });
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
      setActionMessage({ type: 'success', text: t('codex.msgOAuthSuccess') });
      setShowOAuthModal(false);
      setOauthTicket(null);
      setOauthCode('');
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('codex.msgOAuthError') });
    } finally {
      setActionLoading(false);
    }
  };

  return (
    <div>
      <div className="section-header">
        <div>
          <h2 className="section-title">{t('codex.title')}</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('codex.subtitle')}
          </span>
        </div>
        <div className="section-actions">
          <button className="btn btn-secondary" onClick={handleStartOAuth} disabled={actionLoading}>
            <span>{t('codex.startOAuth')}</span>
          </button>
          <button className="btn btn-primary" onClick={() => setShowAddModal(true)}>
            <IconPlus size={16} />
            <span>{t('codex.addAccount')}</span>
          </button>
        </div>
      </div>

      {/* KPI Cards for Codex */}
      <div className="kpi-grid" style={{ marginBottom: 20 }}>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>{t('codex.kpiTotalAccounts')}</span>
            <IconCodex size={16} />
          </div>
          <div className="kpi-value">{status?.total_accounts ?? accounts.length}</div>
        </div>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>{t('codex.kpiActive')}</span>
            <span className="badge badge-success">{t('codex.badgeActive')}</span>
          </div>
          <div className="kpi-value" style={{ color: '#10b981' }}>
            {status?.active_accounts ?? 0}
          </div>
        </div>
        <div className="card" style={{ padding: '14px 18px' }}>
          <div className="kpi-label">
            <span>{t('codex.kpiCooldown')}</span>
            <span className="badge badge-warning">{t('codex.badgeCooldown')}</span>
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
            <p>{t('codex.loading')}</p>
          </div>
        ) : accounts.length === 0 ? (
          <div className="state-container">
            <p>{t('codex.empty')}</p>
            <button className="btn btn-secondary btn-sm" onClick={() => setShowAddModal(true)}>
              {t('codex.btnAddFirst')}
            </button>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>{t('codex.thEmailId')}</th>
                  <th>{t('codex.thAuthPath')}</th>
                  <th>{t('codex.thStatus')}</th>
                  <th>{t('codex.thLastError')}</th>
                  <th>{t('codex.thActions')}</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((acc) => (
                  <tr key={acc.id}>
                    <td>
                      <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                        {acc.email || t('codex.unidentified')}
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
                        {acc.is_active ?? acc.active ? t('codex.statusActive') : t('codex.statusPaused')}
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
                        <span style={{ color: 'var(--text-muted)', fontSize: 12 }}>{t('codex.statusNormal')}</span>
                      )}
                    </td>
                    <td>
                      <div className="table-actions">
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleToggle(acc)}
                          disabled={actionLoading}
                        >
                          {acc.is_active ? t('codex.btnTurnOff') : t('codex.btnTurnOn')}
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleReset(acc)}
                          disabled={actionLoading}
                          title={t('codex.resetTitle')}
                        >
                          {t('codex.btnReset')}
                        </button>
                        <button
                          className="btn btn-danger btn-sm btn-icon-only"
                          onClick={() => handleDelete(acc)}
                          disabled={actionLoading}
                          title={t('codex.deleteTitle')}
                          aria-label={t('codex.deleteTitle')}
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
              <h3 className="modal-title">{t('codex.modalAddTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddModal(false)}
                aria-label={t('actions.close')}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleAddAccount}>
              <div className="form-group">
                <label>{t('codex.labelEmail')}</label>
                <input
                  type="text"
                  placeholder="user@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label>{t('codex.labelAuthPath')}</label>
                <input
                  type="text"
                  placeholder="/home/namhv/.codex/auth.json"
                  value={authPath}
                  onChange={(e) => setAuthPath(e.target.value)}
                  required
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  {t('codex.authPathNote')}
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
                  {actionLoading ? <div className="spinner" /> : t('codex.saveAccount')}
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
              <h3 className="modal-title">{t('codex.modalOAuthTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowOAuthModal(false)}
                aria-label={t('actions.close')}
              >
                <IconX size={16} />
              </button>
            </div>

            <div>
              <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
                {t('codex.oauthStep1')}
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
                  <label>{t('codex.oauthStep2')}</label>
                  <input
                    type="text"
                    placeholder={t('codex.oauthCodePlaceholder')}
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
                    {t('actions.cancel')}
                  </button>
                  <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                    {actionLoading ? <div className="spinner" /> : t('codex.completeOAuth')}
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
