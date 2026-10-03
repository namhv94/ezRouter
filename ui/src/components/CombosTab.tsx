import React, { useState, useEffect } from 'react';
import { ComboRecord, ModelEntry } from '../types';
import { api } from '../api';
import { useI18n } from '../i18n';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconTrash,
  IconEdit,
} from '../icons';

export const CombosTab: React.FC = () => {
  const { t } = useI18n();
  const [combos, setCombos] = useState<ComboRecord[]>([]);
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [showModal, setShowModal] = useState(false);
  const [editingCombo, setEditingCombo] = useState<ComboRecord | null>(null);

  // Form fields
  const [name, setName] = useState('');
  const [strategy, setStrategy] = useState('round-robin');
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [modelSearch, setModelSearch] = useState('');

  const [actionLoading, setActionLoading] = useState(false);
  const [actionMessage, setActionMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(
    null
  );

  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [combosRes, modelsRes] = await Promise.all([
        api.getCombos(),
        api.getModels().catch(() => ({ object: 'list', data: [] })),
      ]);
      setCombos(combosRes);
      setModels(modelsRes.data || []);
    } catch (err: any) {
      setError(err?.message || t('combos.msgLoadError'));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const openAddModal = () => {
    setEditingCombo(null);
    setName('');
    setStrategy('round-robin');
    setSelectedModels([]);
    setShowModal(true);
  };

  const openEditModal = (c: ComboRecord) => {
    setEditingCombo(c);
    setName(c.name);
    setStrategy(c.strategy || 'round-robin');
    const parsed = Array.isArray(c.models)
      ? c.models
      : typeof c.models === 'string'
      ? JSON.parse(c.models)
      : [];
    setSelectedModels(parsed);
    setShowModal(true);
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setActionMessage({ type: 'error', text: t('combos.msgNameRequired') });
      return;
    }
    if (selectedModels.length === 0) {
      setActionMessage({ type: 'error', text: t('combos.msgMinModel') });
      return;
    }

    setActionLoading(true);
    setActionMessage(null);
    try {
      if (editingCombo) {
        await api.updateCombo(editingCombo.id, {
          name: name.trim(),
          strategy,
          models: selectedModels,
        });
        setActionMessage({ type: 'success', text: t('combos.msgUpdateSuccess', { name }) });
      } else {
        await api.createCombo({
          name: name.trim(),
          strategy,
          models: selectedModels,
        });
        setActionMessage({ type: 'success', text: t('combos.msgCreateSuccess', { name }) });
      }
      setShowModal(false);
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('combos.msgSaveError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (c: ComboRecord) => {
    if (!window.confirm(t('combos.confirmDelete', { name: c.name }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteCombo(c.id);
      setActionMessage({ type: 'success', text: t('combos.msgDeleteSuccess', { name: c.name }) });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('combos.msgDeleteError') });
    } finally {
      setActionLoading(false);
    }
  };

  const toggleModelSelection = (modelId: string) => {
    if (selectedModels.includes(modelId)) {
      setSelectedModels(selectedModels.filter((m) => m !== modelId));
    } else {
      setSelectedModels([...selectedModels, modelId]);
    }
  };

  const moveModelUp = (index: number) => {
    if (index <= 0) return;
    const next = [...selectedModels];
    const temp = next[index - 1];
    next[index - 1] = next[index];
    next[index] = temp;
    setSelectedModels(next);
  };

  const moveModelDown = (index: number) => {
    if (index >= selectedModels.length - 1) return;
    const next = [...selectedModels];
    const temp = next[index + 1];
    next[index + 1] = next[index];
    next[index] = temp;
    setSelectedModels(next);
  };

  const removeSelectedModel = (modelId: string) => {
    setSelectedModels(selectedModels.filter((m) => m !== modelId));
  };

  const filteredModels = models.filter((m) =>
    m.id.toLowerCase().includes(modelSearch.toLowerCase())
  );

  return (
    <div>
      <div className="section-header">
        <div>
          <h2 className="section-title">{t('combos.title')}</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('combos.subtitle')}
          </span>
        </div>
        <button className="btn btn-primary" onClick={openAddModal}>
          <IconPlus size={16} />
          <span>{t('combos.createCombo')}</span>
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

      {/* Combos List */}
      <div className="card" style={{ marginBottom: 24 }}>
        <div className="section-header">
          <h3 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)' }}>
            {t('combos.combosListTitle')}
          </h3>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {t('combos.combosCount', { count: combos.length })}
          </span>
        </div>

        {loading && combos.length === 0 ? (
          <div className="state-container">
            <div className="spinner" />
            <p>{t('combos.loading')}</p>
          </div>
        ) : combos.length === 0 ? (
          <div className="state-container">
            <p>{t('combos.empty')}</p>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>{t('combos.thName')}</th>
                  <th>{t('combos.thStrategy')}</th>
                  <th>{t('combos.thModels')}</th>
                  <th>{t('combos.thActions')}</th>
                </tr>
              </thead>
              <tbody>
                {combos.map((c) => {
                  const mList: string[] = Array.isArray(c.models)
                    ? c.models
                    : typeof c.models === 'string'
                    ? JSON.parse(c.models)
                    : [];
                  return (
                    <tr key={c.id}>
                      <td>
                        <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>{c.name}</div>
                        <div className="badge badge-neutral font-mono" style={{ fontSize: 11, marginTop: 4 }}>
                          id: {c.id}
                        </div>
                      </td>
                      <td>
                        <span className="badge badge-neutral" style={{ textTransform: 'uppercase' }}>
                          {c.strategy}
                        </span>
                      </td>
                      <td>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                          {mList.length === 0 ? (
                            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>{t('combos.noModels')}</span>
                          ) : (
                            mList.map((m, idx) => (
                              <div key={m} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                                <span
                                  className="badge badge-primary font-mono"
                                  style={{ fontSize: 10, padding: '1px 6px', minWidth: 22, textAlign: 'center' }}
                                >
                                  #{idx + 1}
                                </span>
                                <span className="font-mono" style={{ fontSize: 12, color: 'var(--text-primary)' }}>
                                  {m}
                                </span>
                                {idx < mList.length - 1 && (
                                  <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 2 }}>↓</span>
                                )}
                              </div>
                            ))
                          )}
                        </div>
                      </td>
                      <td>
                        <div className="table-actions">
                          <button
                            className="btn btn-secondary btn-sm btn-icon-only"
                            onClick={() => openEditModal(c)}
                            title={t('combos.editTitle')}
                            aria-label={t('combos.editTitle')}
                          >
                            <IconEdit size={14} />
                          </button>
                          <button
                            className="btn btn-danger btn-sm btn-icon-only"
                            onClick={() => handleDelete(c)}
                            disabled={actionLoading}
                            title={t('combos.deleteTitle')}
                            aria-label={t('combos.deleteTitle')}
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

      {/* Available Models List */}
      <div className="card">
        <div className="section-header">
          <div>
            <h3 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)' }}>
              {t('combos.availableModelsTitle')}
            </h3>
            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
              {t('combos.availableModelsSubtitle')}
            </span>
          </div>
          <span className="badge badge-neutral font-mono">{t('combos.modelsCount', { count: models.length })}</span>
        </div>

        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>{t('combos.thModelId')}</th>
                <th>{t('combos.thModelCategory')}</th>
                <th>{t('combos.thModelSource')}</th>
              </tr>
            </thead>
            <tbody>
              {models.length === 0 ? (
                <tr>
                  <td colSpan={3} style={{ textAlign: 'center', padding: 24 }}>
                    {t('combos.emptyModels')}
                  </td>
                </tr>
              ) : (
                models.map((m) => (
                  <tr key={m.id}>
                    <td style={{ fontWeight: 600, color: 'var(--text-primary)' }} className="font-mono">
                      {m.id}
                    </td>
                    <td>
                      <span className="badge badge-neutral">{m.object}</span>
                    </td>
                    <td className="font-mono" style={{ color: 'var(--text-secondary)' }}>
                      {m.owned_by}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Add / Edit Combo Modal */}
      {showModal && (
        <div className="modal-backdrop" onClick={() => setShowModal(false)}>
          <div className="modal-card" style={{ maxWidth: 540 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">{editingCombo ? t('combos.modalTitleEdit') : t('combos.modalTitleCreate')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowModal(false)}
                aria-label={t('actions.close')}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleSave}>
              <div className="form-group">
                <label>{t('combos.labelName')}</label>
                <input
                  type="text"
                  placeholder={t('combos.namePlaceholder')}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                />
              </div>

              <div className="form-group">
                <label>{t('combos.labelStrategy')}</label>
                <select value={strategy} onChange={(e) => setStrategy(e.target.value)}>
                  <option value="round-robin">{t('combos.optRoundRobin')}</option>
                  <option value="fallback">{t('combos.optFallback')}</option>
                </select>
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)', marginTop: 4, display: 'block' }}>
                  {strategy === 'fallback'
                    ? t('combos.hintFallback')
                    : t('combos.hintRoundRobin')}
                </span>
              </div>

              {/* Priority Reordering Section */}
              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
                  <label style={{ marginBottom: 0 }}>
                    {t('combos.labelSelectedModels', { count: selectedModels.length })}
                  </label>
                  {selectedModels.length > 0 && (
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={() => setSelectedModels([])}
                      style={{ fontSize: 11, padding: '2px 8px' }}
                    >
                      {t('combos.clearAll')}
                    </button>
                  )}
                </div>

                {selectedModels.length === 0 ? (
                  <div
                    style={{
                      padding: '12px 14px',
                      background: 'var(--bg-secondary)',
                      borderRadius: 'var(--radius-sm)',
                      border: '1px dashed var(--border)',
                      color: 'var(--text-muted)',
                      fontSize: 12.5,
                      textAlign: 'center',
                    }}
                  >
                    {t('combos.noModelsSelected')}
                  </div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 180, overflowY: 'auto' }}>
                    {selectedModels.map((modelId, idx) => (
                      <div
                        key={modelId}
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'space-between',
                          padding: '6px 10px',
                          background: 'var(--bg-secondary)',
                          border: '1px solid var(--border-subtle)',
                          borderRadius: 'var(--radius-sm)',
                        }}
                      >
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0, flex: 1 }}>
                          <span
                            className="badge badge-primary font-mono"
                            style={{ fontSize: 11, minWidth: 26, textAlign: 'center', flexShrink: 0 }}
                          >
                            #{idx + 1}
                          </span>
                          <span
                            className="font-mono"
                            style={{
                              fontSize: 12,
                              fontWeight: 500,
                              whiteSpace: 'nowrap',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              color: 'var(--text-primary)',
                            }}
                            title={modelId}
                          >
                            {modelId}
                          </span>
                          {idx === 0 && strategy === 'fallback' && (
                            <span className="badge badge-success" style={{ fontSize: 10, flexShrink: 0 }}>
                              {t('combos.badgePrimary')}
                            </span>
                          )}
                          {idx > 0 && strategy === 'fallback' && (
                            <span className="badge badge-neutral" style={{ fontSize: 10, flexShrink: 0 }}>
                              {t('combos.badgeFallback', { idx })}
                            </span>
                          )}
                        </div>

                        <div style={{ display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0 }}>
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm btn-icon-only"
                            onClick={() => moveModelUp(idx)}
                            disabled={idx === 0}
                            title={t('combos.btnMoveUp')}
                            aria-label={t('combos.btnMoveUp')}
                            style={{ width: 26, height: 26, padding: 0 }}
                          >
                            ▲
                          </button>
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm btn-icon-only"
                            onClick={() => moveModelDown(idx)}
                            disabled={idx === selectedModels.length - 1}
                            title={t('combos.btnMoveDown')}
                            aria-label={t('combos.btnMoveDown')}
                            style={{ width: 26, height: 26, padding: 0 }}
                          >
                            ▼
                          </button>
                          <button
                            type="button"
                            className="btn btn-danger btn-sm btn-icon-only"
                            onClick={() => removeSelectedModel(modelId)}
                            title={t('combos.btnRemove')}
                            aria-label={t('combos.btnRemove')}
                            style={{ width: 26, height: 26, padding: 0 }}
                          >
                            ✕
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="form-group">
                <label>{t('combos.labelSearch')}</label>
                <input
                  type="text"
                  placeholder={t('combos.searchPlaceholder')}
                  value={modelSearch}
                  onChange={(e) => setModelSearch(e.target.value)}
                  style={{ marginBottom: 8 }}
                />
                <div className="model-checklist-container">
                  {filteredModels.map((m) => {
                    const isChecked = selectedModels.includes(m.id);
                    return (
                      <label
                        key={m.id}
                        className={`model-checklist-item ${isChecked ? 'selected' : ''}`}
                      >
                        <input
                          type="checkbox"
                          checked={isChecked}
                          onChange={() => toggleModelSelection(m.id)}
                        />
                        <span className="font-mono text-break" style={{ fontSize: 12 }}>
                          {m.id}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowModal(false)}
                >
                  {t('actions.cancel')}
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : editingCombo ? t('combos.btnUpdate') : t('combos.btnCreate')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
