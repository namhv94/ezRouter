import React, { useState, useEffect } from 'react';
import { api } from '../api';
import { TokenSaverSettings, TokenSaverLevel, AdminStats } from '../types';
import {
  IconShield,
  IconCheck,
  IconAlertCircle,
  IconRefresh,
  IconTerminal,
} from '../icons';

export const TokenSaverTab: React.FC = () => {
  const [settings, setSettings] = useState<TokenSaverSettings>({
    token_saver_enabled: true,
    rtk_enabled: true,
    caveman_level: 'lite',
    ponytail_level: 'full',
  });
  const [stats, setStats] = useState<AdminStats | null>(null);

  const [loading, setLoading] = useState<boolean>(true);
  const [saving, setSaving] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  const fetchSettings = async () => {
    setLoading(true);
    setError(null);
    try {
      const [res, statsRes] = await Promise.all([
        api.getSettings(),
        api.getStats().catch(() => null),
      ]);
      setSettings(res);
      if (statsRes) {
        setStats(statsRes);
      }
    } catch (err: any) {
      setError(err?.message || 'Không thể tải cài đặt Token Saver.');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchSettings();
  }, []);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setSuccessMsg(null);
    try {
      const res = await api.updateSettings(settings);
      setSettings({
        token_saver_enabled: res.token_saver_enabled,
        rtk_enabled: res.rtk_enabled,
        caveman_level: res.caveman_level,
        ponytail_level: res.ponytail_level,
      });
      setSuccessMsg('Đã lưu cấu hình Token Saver thành công!');
      setTimeout(() => setSuccessMsg(null), 4000);
    } catch (err: any) {
      setError(err?.message || 'Lỗi khi lưu cài đặt Token Saver.');
    } finally {
      setSaving(false);
    }
  };

  const promptTokens = stats?.prompt_tokens || 0;
  const completionTokens = stats?.completion_tokens || 0;

  // Estimated savings calculation based on active modes
  const estimatedInputSaved =
    settings.token_saver_enabled && settings.rtk_enabled
      ? Math.round(promptTokens * 0.35)
      : 0;
  const estimatedOutputSaved =
    settings.token_saver_enabled && settings.caveman_level !== 'off'
      ? Math.round(completionTokens * 0.25)
      : 0;
  const estimatedCodeSaved =
    settings.token_saver_enabled && settings.ponytail_level !== 'off'
      ? Math.round(completionTokens * 0.15)
      : 0;
  const totalEstimatedSaved =
    estimatedInputSaved + estimatedOutputSaved + estimatedCodeSaved;

  if (loading) {
    return (
      <div className="state-container" style={{ padding: 60 }}>
        <div className="spinner" />
        <p style={{ marginTop: 12 }}>Đang tải cài đặt Token Saver...</p>
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 24, paddingBottom: 40 }}>
      {/* Top Section Header */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'flex-start',
          flexWrap: 'wrap',
          gap: 16,
        }}
      >
        <div>
          <h2 style={{ margin: '0 0 6px 0', fontSize: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <IconShield size={22} style={{ color: 'var(--color-primary, #6366f1)' }} />
            Tối Ưu Chi Phí & Token Saver
          </h2>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13.5 }}>
            Giảm thiểu token input (RTK) và token output (Caveman & Ponytail) để tối ưu chi phí và tăng tốc độ xử lý.
          </p>
        </div>

        <div style={{ display: 'flex', gap: 10 }}>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={fetchSettings}
            disabled={saving}
            title="Làm mới"
          >
            <IconRefresh size={16} />
            Làm Mới
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving}
            style={{ minWidth: 120 }}
          >
            {saving ? (
              <>
                <div className="spinner" style={{ width: 14, height: 14 }} />
                Đang Lưu...
              </>
            ) : (
              <>
                <IconCheck size={16} />
                Lưu Cài Đặt
              </>
            )}
          </button>
        </div>
      </div>

      {/* Notifications */}
      {successMsg && (
        <div
          style={{
            backgroundColor: 'rgba(16, 185, 129, 0.12)',
            border: '1px solid #10b981',
            borderRadius: 'var(--radius-md, 8px)',
            padding: '12px 16px',
            color: '#10b981',
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            fontSize: 13.5,
          }}
        >
          <IconCheck size={18} />
          {successMsg}
        </div>
      )}

      {error && (
        <div
          style={{
            backgroundColor: 'rgba(239, 68, 68, 0.12)',
            border: '1px solid #ef4444',
            borderRadius: 'var(--radius-md, 8px)',
            padding: '12px 16px',
            color: '#ef4444',
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            fontSize: 13.5,
          }}
        >
          <IconAlertCircle size={18} />
          {error}
        </div>
      )}

      {/* Summary KPI Cards */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: 16,
        }}
      >
        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            TRẠNG THÁI TOKEN SAVER
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            {settings.token_saver_enabled ? (
              <span className="badge badge-success" style={{ fontSize: 13, padding: '4px 10px' }}>
                Đang Kích Hoạt
              </span>
            ) : (
              <span className="badge badge-danger" style={{ fontSize: 13, padding: '4px 10px' }}>
                Đang Tắt
              </span>
            )}
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            Master Switch toàn bộ hệ thống proxy
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            RTK INPUT COMPRESSION
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            {settings.rtk_enabled && settings.token_saver_enabled ? (
              <span className="badge badge-success" style={{ fontSize: 13, padding: '4px 10px' }}>
                Bật (Active)
              </span>
            ) : (
              <span className="badge badge-secondary" style={{ fontSize: 13, padding: '4px 10px' }}>
                Tắt (Off)
              </span>
            )}
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            Nén log, diff, git status & file thừa
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            CAVEMAN OUTPUT MODE
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            <span
              className={
                settings.caveman_level === 'off'
                  ? 'badge badge-secondary'
                  : 'badge badge-primary'
              }
              style={{ fontSize: 13, padding: '4px 10px', textTransform: 'uppercase' }}
            >
              {settings.caveman_level}
            </span>
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            Rút gọn câu chữ, loại bỏ filler
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            PONYTAIL CODE MODE
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            <span
              className={
                settings.ponytail_level === 'off'
                  ? 'badge badge-secondary'
                  : 'badge badge-primary'
              }
              style={{ fontSize: 13, padding: '4px 10px', textTransform: 'uppercase' }}
            >
              {settings.ponytail_level}
            </span>
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            Ưu tiên stdlib/native, triệt tiêu boilerplate
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            TIẾT KIỆM TOKEN ƯỚC TÍNH (SAVINGS)
          </div>
          <div style={{ fontSize: 20, fontWeight: 600, color: 'var(--color-success, #10b981)' }}>
            {settings.token_saver_enabled ? (
              <span>~{totalEstimatedSaved.toLocaleString()} tokens</span>
            ) : (
              <span style={{ color: 'var(--text-muted, #64748b)' }}>0 tokens</span>
            )}
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            {settings.token_saver_enabled
              ? 'Tiết kiệm ~30-50% tổng token xử lý'
              : 'Token Saver đang tắt'}
          </div>
        </div>
      </div>

      {/* Main Settings Panel */}
      <div className="card" style={{ padding: 24 }}>
        <h3 style={{ margin: '0 0 20px 0', fontSize: 16 }}>Cài Đặt Cấu Hình Token Saver</h3>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
          {/* Master Switch */}
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              padding: '16px 20px',
              backgroundColor: 'var(--bg-input, rgba(255,255,255,0.02))',
              borderRadius: 'var(--radius-md, 8px)',
              border: '1px solid var(--border-subtle, rgba(255,255,255,0.06))',
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: 14.5, marginBottom: 4 }}>
                Kích Hoạt Token Saver (Master Switch)
              </div>
              <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                Bật hoặc tắt toàn bộ các tính năng nén và tối ưu hóa token trên router.
              </div>
            </div>
            <label style={{ display: 'flex', alignItems: 'center', cursor: 'pointer', gap: 10 }}>
              <input
                type="checkbox"
                checked={settings.token_saver_enabled}
                onChange={(e) =>
                  setSettings({ ...settings, token_saver_enabled: e.target.checked })
                }
                style={{ width: 20, height: 20, cursor: 'pointer' }}
              />
              <span style={{ fontSize: 13.5, fontWeight: 500 }}>
                {settings.token_saver_enabled ? 'Đang Bật' : 'Đang Tắt'}
              </span>
            </label>
          </div>

          {/* RTK Input Compression Switch */}
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              padding: '16px 20px',
              backgroundColor: 'var(--bg-input, rgba(255,255,255,0.02))',
              borderRadius: 'var(--radius-md, 8px)',
              border: '1px solid var(--border-subtle, rgba(255,255,255,0.06))',
              opacity: settings.token_saver_enabled ? 1 : 0.6,
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: 14.5, marginBottom: 4 }}>
                RTK Input Compression
              </div>
              <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                Nén log lặp, git diff dài, git status, file thừa (node_modules, .git, venv, build) và output build trước khi gửi đến model upstream.
              </div>
            </div>
            <label style={{ display: 'flex', alignItems: 'center', cursor: 'pointer', gap: 10 }}>
              <input
                type="checkbox"
                checked={settings.rtk_enabled}
                disabled={!settings.token_saver_enabled}
                onChange={(e) =>
                  setSettings({ ...settings, rtk_enabled: e.target.checked })
                }
                style={{ width: 20, height: 20, cursor: 'pointer' }}
              />
              <span style={{ fontSize: 13.5, fontWeight: 500 }}>
                {settings.rtk_enabled ? 'Đang Bật' : 'Đang Tắt'}
              </span>
            </label>
          </div>

          {/* Caveman Selector */}
          <div
            className="form-group"
            style={{
              padding: '16px 20px',
              backgroundColor: 'var(--bg-input, rgba(255,255,255,0.02))',
              borderRadius: 'var(--radius-md, 8px)',
              border: '1px solid var(--border-subtle, rgba(255,255,255,0.06))',
              opacity: settings.token_saver_enabled ? 1 : 0.6,
              marginBottom: 0,
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: 14.5, marginBottom: 4 }}>
                  Caveman Mode (Tối Giản Câu Chữ Output)
                </div>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                  Cắt bỏ kính ngữ và filler words (just, really, pleasantries), giữ nguyên mã nguồn, lệnh, đường dẫn, URLs và lỗi.
                </div>
              </div>
            </div>
            <select
              value={settings.caveman_level}
              disabled={!settings.token_saver_enabled}
              onChange={(e) =>
                setSettings({ ...settings, caveman_level: e.target.value as TokenSaverLevel })
              }
              style={{ maxWidth: 450 }}
            >
              <option value="off">Tắt (off) — Không thay đổi phong cách trả lời</option>
              <option value="lite">Lite (Khuyên Dùng) — Ngắn gọn súc tích, giữ ngữ pháp, bỏ kính ngữ & filler</option>
              <option value="full">Full — Terse caveman, cắt bỏ tối đa từ thừa, cấu trúc ngắn gọn</option>
              <option value="ultra">Ultra — Cực ngắn, tối đa nén, phong cách điện tín</option>
            </select>
          </div>

          {/* Ponytail Selector */}
          <div
            className="form-group"
            style={{
              padding: '16px 20px',
              backgroundColor: 'var(--bg-input, rgba(255,255,255,0.02))',
              borderRadius: 'var(--radius-md, 8px)',
              border: '1px solid var(--border-subtle, rgba(255,255,255,0.06))',
              opacity: settings.token_saver_enabled ? 1 : 0.6,
              marginBottom: 0,
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: 14.5, marginBottom: 4 }}>
                  Ponytail Mode (Tối Giản Code / YAGNI)
                </div>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                  Ép mô hình áp dụng kỷ luật lazy senior dev: stdlib và native trước, diff ngắn nhất, không sinh abstraction sớm.
                </div>
              </div>
            </div>
            <select
              value={settings.ponytail_level}
              disabled={!settings.token_saver_enabled}
              onChange={(e) =>
                setSettings({ ...settings, ponytail_level: e.target.value as TokenSaverLevel })
              }
              style={{ maxWidth: 450 }}
            >
              <option value="off">Tắt (off) — Mô hình viết code như bình thường</option>
              <option value="lite">Lite — Gợi ý phương án lười/tối giản trong 1 dòng</option>
              <option value="full">Full (Khuyên Dùng) — Bậc thang YAGNI, stdlib/native first, diff ngắn nhất</option>
              <option value="ultra">Ultra — YAGNI cực đoan, xóa code trước khi thêm, code one-liner</option>
            </select>
          </div>
        </div>
      </div>

      {/* Feature Explanations Grid */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
          gap: 16,
        }}
      >
        <div className="card" style={{ padding: 20 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
            <div
              style={{
                width: 32,
                height: 32,
                borderRadius: 6,
                backgroundColor: 'rgba(99, 102, 241, 0.15)',
                color: 'var(--color-primary, #6366f1)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontWeight: 700,
                fontSize: 12,
              }}
            >
              RTK
            </div>
            <h4 style={{ margin: 0, fontSize: 15 }}>Nén Input Chọn Lọc</h4>
          </div>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13, lineHeight: 1.5 }}>
            Tự động phát hiện log lặp, diff git quá 80 dòng, commit log quá 20 mục, danh sách file chứa thư mục rác (node_modules, target, build), và log build dài. Giữ lại thông tin quan trọng và các dòng lỗi trong khi cắt giảm tới 80% token input.
          </p>
        </div>

        <div className="card" style={{ padding: 20 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
            <div
              style={{
                width: 32,
                height: 32,
                borderRadius: 6,
                backgroundColor: 'rgba(16, 185, 129, 0.15)',
                color: '#10b981',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontWeight: 700,
                fontSize: 12,
              }}
            >
              TXT
            </div>
            <h4 style={{ margin: 0, fontSize: 15 }}>Caveman Output Mode</h4>
          </div>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13, lineHeight: 1.5 }}>
            Inject chỉ dẫn tối ưu vào system/developer prompt. Yêu cầu mô hình loại bỏ các câu mở đầu/kết thúc lịch sự rườm rà, tập trung trực diện vào hành động, lý do và bước tiếp theo. Bảo toàn 100% độ chính xác cho đường dẫn, lệnh và mã code.
          </p>
        </div>

        <div className="card" style={{ padding: 20 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
            <div
              style={{
                width: 32,
                height: 32,
                borderRadius: 6,
                backgroundColor: 'rgba(245, 158, 11, 0.15)',
                color: '#f59e0b',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontWeight: 700,
                fontSize: 12,
              }}
            >
              {'{ }'}
            </div>
            <h4 style={{ margin: 0, fontSize: 15 }}>Ponytail Code Discipline</h4>
          </div>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13, lineHeight: 1.5 }}>
            Kỷ luật code tối giản: 1) Có cần tồn tại không? (YAGNI). 2) Thư viện chuẩn có sẵn? Dùng nó. 3) Tính năng native hệ điều hành? Dùng nó. 4) Dependency đã cài? Dùng nó. Không sinh interface hay abstraction khi chưa có yêu cầu.
          </p>
        </div>
      </div>

      {/* Savings & Hermes Tool Safety Invariants Card */}
      <div
        className="card"
        style={{
          padding: 22,
          backgroundColor: 'rgba(99, 102, 241, 0.03)',
          border: '1px solid rgba(99, 102, 241, 0.15)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 14 }}>
          <IconShield size={20} style={{ color: 'var(--color-primary, #6366f1)' }} />
          <h4 style={{ margin: 0, fontSize: 15, fontWeight: 600 }}>
            Hiệu Quả Tiết Kiệm & Bảo Toàn An Toàn Hermes / Tool Calling
          </h4>
        </div>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
            gap: 16,
            marginBottom: 16,
          }}
        >
          <div
            style={{
              padding: 14,
              backgroundColor: 'var(--bg-input, rgba(255,255,255,0.02))',
              borderRadius: 'var(--radius-md, 8px)',
              border: '1px solid var(--border-subtle, rgba(255,255,255,0.05))',
            }}
          >
            <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 4 }}>
              Ước Tính Tiết Kiệm Input (RTK)
            </div>
            <div style={{ fontSize: 17, fontWeight: 600, color: '#6366f1' }}>
              ~{estimatedInputSaved.toLocaleString()} tokens
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 4 }}>
              Cắt giảm 30% - 70% trên log, diff, git status & build output
            </div>
          </div>

          <div
            style={{
              padding: 14,
              backgroundColor: 'var(--bg-input, rgba(255,255,255,0.02))',
              borderRadius: 'var(--radius-md, 8px)',
              border: '1px solid var(--border-subtle, rgba(255,255,255,0.05))',
            }}
          >
            <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 4 }}>
              Ước Tính Tiết Kiệm Output (Caveman + Ponytail)
            </div>
            <div style={{ fontSize: 17, fontWeight: 600, color: '#10b981' }}>
              ~{(estimatedOutputSaved + estimatedCodeSaved).toLocaleString()} tokens
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 4 }}>
              Giảm 20% - 40% output câu chữ & loại bỏ code boilerplate
            </div>
          </div>
        </div>

        <div
          style={{
            padding: '12px 16px',
            backgroundColor: 'rgba(16, 185, 129, 0.08)',
            border: '1px solid rgba(16, 185, 129, 0.25)',
            borderRadius: 'var(--radius-md, 8px)',
            fontSize: 13,
            color: 'var(--text-primary)',
            lineHeight: 1.5,
          }}
        >
          <strong style={{ color: '#10b981' }}>Bảo Toàn An Toàn Hermes / Tool Safety Invariants:</strong>{' '}
          Hệ thống tuyệt đối <strong>không bao giờ nén</strong> message có role=tool, tool_calls, tool schemas,
          chuỗi JSON có cấu trúc (structured JSON) hoặc các lượt tool continuation gần nhất.
          Chỉ dẫn Caveman và Ponytail được tự động vô hiệu hóa trên các yêu cầu tool calling hoặc structured output (response_format)
          để đảm bảo tính tương thích 100% với Hermes Agent.
        </div>
      </div>

      {/* Per-Request Header Overrides Guide */}
      <div
        className="card"
        style={{
          padding: 20,
          backgroundColor: 'rgba(255, 255, 255, 0.015)',
          border: '1px solid var(--border-subtle, rgba(255,255,255,0.06))',
        }}
      >
        <h4 style={{ margin: '0 0 10px 0', fontSize: 14, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconTerminal size={16} />
          Ghi Đè Tùy Biến Theo Từng Request (HTTP Headers)
        </h4>
        <p style={{ margin: '0 0 12px 0', color: 'var(--text-secondary, #94a3b8)', fontSize: 13 }}>
          Client (Hermes Agent, curl, IDEs) có thể ghi đè cài đặt Token Saver cho từng yêu cầu thông qua các HTTP headers:
        </p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <div style={{ fontFamily: 'monospace', fontSize: 12.5, color: 'var(--text-primary)' }}>
            <span style={{ color: 'var(--color-primary, #6366f1)', fontWeight: 600 }}>x-token-saver: off</span>
            <span style={{ color: 'var(--text-secondary, #94a3b8)' }}> — Tắt hoàn toàn Token Saver cho request này</span>
          </div>
          <div style={{ fontFamily: 'monospace', fontSize: 12.5, color: 'var(--text-primary)' }}>
            <span style={{ color: 'var(--color-primary, #6366f1)', fontWeight: 600 }}>x-caveman: off | lite | full | ultra</span>
            <span style={{ color: 'var(--text-secondary, #94a3b8)' }}> — Thiết lập cấp độ Caveman cho request này</span>
          </div>
          <div style={{ fontFamily: 'monospace', fontSize: 12.5, color: 'var(--text-primary)' }}>
            <span style={{ color: 'var(--color-primary, #6366f1)', fontWeight: 600 }}>x-ponytail: off | lite | full | ultra</span>
            <span style={{ color: 'var(--text-secondary, #94a3b8)' }}> — Thiết lập cấp độ Ponytail cho request này</span>
          </div>
        </div>
      </div>
    </div>
  );
};
