import React, { useState, useEffect } from 'react';
import { ComboRecord, ModelEntry } from '../types';
import { api } from '../api';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconTrash,
  IconEdit,
} from '../icons';

export const CombosTab: React.FC = () => {
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
      setError(err?.message || 'Không thể tải dữ liệu điều phối');
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
      setActionMessage({ type: 'error', text: 'Vui lòng nhập tên Combo.' });
      return;
    }
    if (selectedModels.length === 0) {
      setActionMessage({ type: 'error', text: 'Vui lòng chọn ít nhất 1 model cho combo.' });
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
        setActionMessage({ type: 'success', text: `Đã cập nhật combo "${name}".` });
      } else {
        await api.createCombo({
          name: name.trim(),
          strategy,
          models: selectedModels,
        });
        setActionMessage({ type: 'success', text: `Đã tạo combo "${name}".` });
      }
      setShowModal(false);
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi lưu thông tin combo.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDelete = async (c: ComboRecord) => {
    if (!window.confirm(`Xác nhận xóa combo điều phối "${c.name}"?`)) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteCombo(c.id);
      setActionMessage({ type: 'success', text: `Đã xóa combo "${c.name}".` });
      loadData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể xóa combo.' });
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
          <h2 className="section-title">Cấu Hình Combo Model</h2>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            Định tuyến dự phòng (Fallback) hoặc cân bằng tải vòng tròn (Round-Robin) giữa các upstream
          </span>
        </div>
        <button className="btn btn-primary" onClick={openAddModal}>
          <IconPlus size={16} />
          <span>Tạo Combo Mới</span>
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
            Danh Sách Combo
          </h3>
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
            {combos.length} cấu hình đã thiết lập
          </span>
        </div>

        {loading && combos.length === 0 ? (
          <div className="state-container">
            <div className="spinner" />
            <p>Đang tải danh sách combo...</p>
          </div>
        ) : combos.length === 0 ? (
          <div className="state-container">
            <p>Chưa có cấu hình combo nào. Nhấn "Tạo Combo Mới" để thiết lập.</p>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Tên Combo</th>
                  <th>Chiến Lược</th>
                  <th>Models Thành Viên</th>
                  <th>Hành Động</th>
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
                            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Chưa có model</span>
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
                            title="Sửa combo"
                          >
                            <IconEdit size={14} />
                          </button>
                          <button
                            className="btn btn-danger btn-sm btn-icon-only"
                            onClick={() => handleDelete(c)}
                            disabled={actionLoading}
                            title="Xóa combo"
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
              Danh Sách Model Khả Dụng (/v1/models)
            </h3>
            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
              Các model được phát hiện từ Antigravity, Codex và Upstream Providers
            </span>
          </div>
          <span className="badge badge-neutral font-mono">{models.length} models</span>
        </div>

        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>Mã Model (ID)</th>
                <th>Phân Loại</th>
                <th>Nguồn Cung Cấp</th>
              </tr>
            </thead>
            <tbody>
              {models.length === 0 ? (
                <tr>
                  <td colSpan={3} style={{ textAlign: 'center', padding: 24 }}>
                    Chưa có model nào trong bộ định tuyến.
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
              <h3 className="modal-title">{editingCombo ? 'Sửa Combo Điều Phối' : 'Tạo Combo Mới'}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleSave}>
              <div className="form-group">
                <label>Tên Combo</label>
                <input
                  type="text"
                  placeholder="VD: combo-fast, auto-fallback"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                />
              </div>

              <div className="form-group">
                <label>Chiến Lược Điều Phối</label>
                <select value={strategy} onChange={(e) => setStrategy(e.target.value)}>
                  <option value="round-robin">Round-Robin (Cân bằng tải xoay vòng)</option>
                  <option value="fallback">Fallback (Ưu tiên theo thứ tự, lỗi chuyển tiếp)</option>
                </select>
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)', marginTop: 4, display: 'block' }}>
                  {strategy === 'fallback'
                    ? '💡 Model #1 được gọi trước. Nếu lỗi hoặc hết quota, router tự động chuyển sang Model #2, #3 theo đúng thứ tự.'
                    : '💡 Yêu cầu mới sẽ được luân phiên phân bổ lần lượt qua từng model theo thứ tự vòng tròn.'}
                </span>
              </div>

              {/* Priority Reordering Section */}
              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
                  <label style={{ marginBottom: 0 }}>
                    Thứ Tự Ưu Tiên Các Model Đã Chọn ({selectedModels.length})
                  </label>
                  {selectedModels.length > 0 && (
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={() => setSelectedModels([])}
                      style={{ fontSize: 11, padding: '2px 8px' }}
                    >
                      Bỏ chọn tất cả
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
                    Chưa có model nào được chọn. Hãy tích chọn từ danh sách bên dưới.
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
                              Chính
                            </span>
                          )}
                          {idx > 0 && strategy === 'fallback' && (
                            <span className="badge badge-neutral" style={{ fontSize: 10, flexShrink: 0 }}>
                              Dự phòng {idx}
                            </span>
                          )}
                        </div>

                        <div style={{ display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0 }}>
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm btn-icon-only"
                            onClick={() => moveModelUp(idx)}
                            disabled={idx === 0}
                            title="Di chuyển lên vị trí ưu tiên hơn"
                            style={{ width: 26, height: 26, padding: 0 }}
                          >
                            ▲
                          </button>
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm btn-icon-only"
                            onClick={() => moveModelDown(idx)}
                            disabled={idx === selectedModels.length - 1}
                            title="Di chuyển xuống vị trí dự phòng sau"
                            style={{ width: 26, height: 26, padding: 0 }}
                          >
                            ▼
                          </button>
                          <button
                            type="button"
                            className="btn btn-danger btn-sm btn-icon-only"
                            onClick={() => removeSelectedModel(modelId)}
                            title="Gỡ khỏi combo"
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
                <label>Tìm & Chọn Thêm Model</label>
                <input
                  type="text"
                  placeholder="Tìm kiếm model..."
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
                  Hủy
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : editingCombo ? 'Cập Nhật' : 'Tạo Mới'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
