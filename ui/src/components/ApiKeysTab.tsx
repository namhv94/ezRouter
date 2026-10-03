import React, { useState } from 'react';
import { ApiKey } from '../types';
import { api } from '../api';
import { useI18n } from '../i18n';
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
  const { t, locale } = useI18n();
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
      setActionMessage({ type: 'error', text: t('apiKeys.errNameRequired') });
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
      setActionMessage({ type: 'success', text: t('apiKeys.msgCreateSuccess', { name: res.name }) });
      setName('');
      setCustomKey('');
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('apiKeys.errCreate') });
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
        text: t(key.is_active ? 'apiKeys.msgDeactivated' : 'apiKeys.msgActivated', { name: key.name }),
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('apiKeys.errUpdateStatus') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (key: ApiKey) => {
    if (!window.confirm(t('apiKeys.confirmDelete', { name: key.name }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteApiKey(key.id);
      setActionMessage({ type: 'success', text: t('apiKeys.msgDeleteSuccess', { name: key.name }) });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('apiKeys.errDelete') });
    } finally {
      setActionLoading(false);
    }
  };

  return (
    <div>
      <div className="section-header">
        <div>
          <h2 className="section-title">{t('apiKeys.title')}</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('apiKeys.subtitle')}
          </span>
        </div>
        <button className="btn btn-primary" onClick={() => setShowAddModal(true)}>
          <IconPlus size={16} />
          <span>{t('apiKeys.createButton')}</span>
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
            <span>{t('apiKeys.createdKeyAlert')}</span>
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
            {t('apiKeys.savedKeyDismiss')}
          </button>
        </div>
      )}

      <div className="card">
        {loading && apiKeys.length === 0 ? (
          <div className="state-container">
            <div className="spinner" />
            <p>{t('apiKeys.loading')}</p>
          </div>
        ) : apiKeys.length === 0 ? (
          <div className="state-container">
            <p>{t('apiKeys.empty')}</p>
            <button className="btn btn-secondary btn-sm" onClick={() => setShowAddModal(true)}>
              {t('apiKeys.createFirst')}
            </button>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>{t('apiKeys.colName')}</th>
                  <th>{t('apiKeys.colKeyMasked')}</th>
                  <th>{t('apiKeys.colStatus')}</th>
                  <th>{t('apiKeys.colTotalRequests')}</th>
                  <th>{t('apiKeys.colCreatedAt')}</th>
                  <th>{t('apiKeys.colActions')}</th>
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
                        {k.is_active ? t('apiKeys.statusActive') : t('apiKeys.statusLocked')}
                      </span>
                    </td>
                    <td className="font-mono">{k.total_requests.toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US')}</td>
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
                          {k.is_active ? t('apiKeys.actionLock') : t('apiKeys.actionUnlock')}
                        </button>
                        <button
                          className="btn btn-danger btn-sm btn-icon-only"
                          onClick={() => handleDelete(k)}
                          disabled={actionLoading}
                          title={t('apiKeys.actionDeleteTitle')}
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
              <h3 className="modal-title">{t('apiKeys.modalTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleCreate}>
              <div className="form-group">
                <label>{t('apiKeys.formNameLabel')}</label>
                <input
                  type="text"
                  placeholder={t('apiKeys.formNamePlaceholder')}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                  autoFocus
                />
              </div>

              <div className="form-group">
                <label>{t('apiKeys.formCustomKeyLabel')}</label>
                <input
                  type="text"
                  placeholder={t('apiKeys.formCustomKeyPlaceholder')}
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
                  {t('actions.cancel')}
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : t('apiKeys.btnSubmitCreate')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
