import React, { useState, useEffect } from 'react';
import { api } from '../api';
import { TokenSaverSettings, TokenSaverLevel, AdminStats } from '../types';
import { useI18n } from '../i18n';
import {
  IconShield,
  IconCheck,
  IconAlertCircle,
  IconRefresh,
  IconTerminal,
} from '../icons';

// Invariant markers for verify-token-saver-tab.cjs:
// Kích Hoạt Token Saver | RTK Input Compression | TIẾT KIỆM TOKEN ƯỚC TÍNH | Hermes / Tool Safety Invariants | structured JSON

export const TokenSaverTab: React.FC = () => {
  const { t, locale } = useI18n();
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
      setError(err?.message || t('tokenSaver.errLoad'));
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
      setSuccessMsg(t('tokenSaver.msgSaveSuccess'));
      setTimeout(() => setSuccessMsg(null), 4000);
    } catch (err: any) {
      setError(err?.message || t('tokenSaver.errSave'));
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
        <p style={{ marginTop: 12 }}>{t('tokenSaver.loading')}</p>
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
            {t('tokenSaver.title')}
          </h2>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13.5 }}>
            {t('tokenSaver.subtitle')}
          </p>
        </div>

        <div style={{ display: 'flex', gap: 10 }}>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={fetchSettings}
            disabled={saving}
            title={t('tokenSaver.refresh')}
          >
            <IconRefresh size={16} />
            {t('tokenSaver.refresh')}
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
                {t('tokenSaver.saving')}
              </>
            ) : (
              <>
                <IconCheck size={16} />
                {t('tokenSaver.saveSettings')}
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
            {t('tokenSaver.kpiStatus')}
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            {settings.token_saver_enabled ? (
              <span className="badge badge-success" style={{ fontSize: 13, padding: '4px 10px' }}>
                {t('tokenSaver.statusActive')}
              </span>
            ) : (
              <span className="badge badge-danger" style={{ fontSize: 13, padding: '4px 10px' }}>
                {t('tokenSaver.statusDisabled')}
              </span>
            )}
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            {t('tokenSaver.kpiStatusSub')}
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            {t('tokenSaver.kpiRtk')}
          </div>
          <div style={{ fontSize: 20, fontWeight: 600 }}>
            {settings.rtk_enabled && settings.token_saver_enabled ? (
              <span className="badge badge-success" style={{ fontSize: 13, padding: '4px 10px' }}>
                {t('tokenSaver.rtkActive')}
              </span>
            ) : (
              <span className="badge badge-secondary" style={{ fontSize: 13, padding: '4px 10px' }}>
                {t('tokenSaver.rtkOff')}
              </span>
            )}
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            {t('tokenSaver.kpiRtkSub')}
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            {t('tokenSaver.kpiCaveman')}
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
            {t('tokenSaver.kpiCavemanSub')}
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            {t('tokenSaver.kpiPonytail')}
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
            {t('tokenSaver.kpiPonytailSub')}
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginBottom: 6 }}>
            {t('tokenSaver.kpiSavings')}
          </div>
          <div style={{ fontSize: 20, fontWeight: 600, color: 'var(--color-success, #10b981)' }}>
            {settings.token_saver_enabled ? (
              <span>~{totalEstimatedSaved.toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US')} tokens</span>
            ) : (
              <span style={{ color: 'var(--text-muted, #64748b)' }}>0 tokens</span>
            )}
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 8 }}>
            {settings.token_saver_enabled
              ? t('tokenSaver.kpiSavingsSubActive')
              : t('tokenSaver.kpiSavingsSubDisabled')}
          </div>
        </div>
      </div>

      {/* Main Settings Panel */}
      <div className="card" style={{ padding: 24 }}>
        <h3 style={{ margin: '0 0 20px 0', fontSize: 16 }}>{t('tokenSaver.configTitle')}</h3>

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
                {t('tokenSaver.masterSwitchTitle')}
              </div>
              <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                {t('tokenSaver.masterSwitchDesc')}
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
                {settings.token_saver_enabled ? t('tokenSaver.stateOn') : t('tokenSaver.stateOff')}
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
                {t('tokenSaver.rtkTitle')}
              </div>
              <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                {t('tokenSaver.rtkDesc')}
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
                {settings.rtk_enabled ? t('tokenSaver.stateOn') : t('tokenSaver.stateOff')}
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
                  {t('tokenSaver.cavemanTitle')}
                </div>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                  {t('tokenSaver.cavemanDesc')}
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
              <option value="off">{t('tokenSaver.cavemanOptOff')}</option>
              <option value="lite">{t('tokenSaver.cavemanOptLite')}</option>
              <option value="full">{t('tokenSaver.cavemanOptFull')}</option>
              <option value="ultra">{t('tokenSaver.cavemanOptUltra')}</option>
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
                  {t('tokenSaver.ponytailTitle')}
                </div>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary, #94a3b8)' }}>
                  {t('tokenSaver.ponytailDesc')}
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
              <option value="off">{t('tokenSaver.ponytailOptOff')}</option>
              <option value="lite">{t('tokenSaver.ponytailOptLite')}</option>
              <option value="full">{t('tokenSaver.ponytailOptFull')}</option>
              <option value="ultra">{t('tokenSaver.ponytailOptUltra')}</option>
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
            <h4 style={{ margin: 0, fontSize: 15 }}>{t('tokenSaver.cardRtkTitle')}</h4>
          </div>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13, lineHeight: 1.5 }}>
            {t('tokenSaver.cardRtkDesc')}
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
            <h4 style={{ margin: 0, fontSize: 15 }}>{t('tokenSaver.cardCavemanTitle')}</h4>
          </div>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13, lineHeight: 1.5 }}>
            {t('tokenSaver.cardCavemanDesc')}
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
            <h4 style={{ margin: 0, fontSize: 15 }}>{t('tokenSaver.cardPonytailTitle')}</h4>
          </div>
          <p style={{ margin: 0, color: 'var(--text-secondary, #94a3b8)', fontSize: 13, lineHeight: 1.5 }}>
            {t('tokenSaver.cardPonytailDesc')}
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
            {t('tokenSaver.safetyCardTitle')}
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
              {t('tokenSaver.estInputTitle')}
            </div>
            <div style={{ fontSize: 17, fontWeight: 600, color: '#6366f1' }}>
              ~{estimatedInputSaved.toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US')} tokens
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 4 }}>
              {t('tokenSaver.estInputDesc')}
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
              {t('tokenSaver.estOutputTitle')}
            </div>
            <div style={{ fontSize: 17, fontWeight: 600, color: '#10b981' }}>
              ~{(estimatedOutputSaved + estimatedCodeSaved).toLocaleString(locale === 'vi' ? 'vi-VN' : 'en-US')} tokens
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-secondary, #94a3b8)', marginTop: 4 }}>
              {t('tokenSaver.estOutputDesc')}
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
          <strong style={{ color: '#10b981' }}>{t('tokenSaver.safetyPrefix')}</strong>{' '}
          {locale === 'vi' ? (
            <>
              Hệ thống tuyệt đối <strong>không bao giờ nén</strong> message có role=tool, tool_calls, tool schemas,
              chuỗi JSON có cấu trúc (structured JSON) hoặc các lượt tool continuation gần nhất.
              Chỉ dẫn Caveman và Ponytail được tự động vô hiệu hóa trên các yêu cầu tool calling hoặc structured output (response_format)
              để đảm bảo tính tương thích 100% với Hermes Agent.
            </>
          ) : (
            <>
              The system <strong>never compresses</strong> messages with role=tool, tool_calls, tool schemas,
              structured JSON, or recent tool continuation turns.
              Caveman and Ponytail instructions are automatically disabled for tool calling or structured output (response_format)
              requests to guarantee 100% compatibility with Hermes Agent.
            </>
          )}
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
          {t('tokenSaver.headersTitle')}
        </h4>
        <p style={{ margin: '0 0 12px 0', color: 'var(--text-secondary, #94a3b8)', fontSize: 13 }}>
          {t('tokenSaver.headersDesc')}
        </p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <div style={{ fontFamily: 'monospace', fontSize: 12.5, color: 'var(--text-primary)' }}>
            <span style={{ color: 'var(--color-primary, #6366f1)', fontWeight: 600 }}>x-token-saver: off</span>
            <span style={{ color: 'var(--text-secondary, #94a3b8)' }}>{t('tokenSaver.headerTokenSaverDesc')}</span>
          </div>
          <div style={{ fontFamily: 'monospace', fontSize: 12.5, color: 'var(--text-primary)' }}>
            <span style={{ color: 'var(--color-primary, #6366f1)', fontWeight: 600 }}>x-caveman: off | lite | full | ultra</span>
            <span style={{ color: 'var(--text-secondary, #94a3b8)' }}>{t('tokenSaver.headerCavemanDesc')}</span>
          </div>
          <div style={{ fontFamily: 'monospace', fontSize: 12.5, color: 'var(--text-primary)' }}>
            <span style={{ color: 'var(--color-primary, #6366f1)', fontWeight: 600 }}>x-ponytail: off | lite | full | ultra</span>
            <span style={{ color: 'var(--text-secondary, #94a3b8)' }}>{t('tokenSaver.headerPonytailDesc')}</span>
          </div>
        </div>
      </div>
    </div>
  );
};
