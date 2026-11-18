import React, { useState, useEffect } from 'react';
import { ProviderResponse, CreateProviderRequest } from '../types';
import { api } from '../api';
import { useI18n } from '../i18n';
import { OperationalProvidersView, OperationalProviderTab } from './OperationalProvidersView';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconRefresh,
  IconTrash,
  IconEdit,
  IconServer,
  IconChevronDown,
  IconOpenRouter,
  IconKey,
  IconZap,
  IconSearch,
} from '../icons';

interface ProvidersTabProps {
  providers: ProviderResponse[];
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

export const ProvidersTab: React.FC<ProvidersTabProps> = ({
  providers,
  loading: parentLoading,
  error: parentError,
  onRefresh,
}) => {
  const { t } = useI18n();

  // Operational Theme & Segmented Nav State
  const [operationalTab, setOperationalTab] = useState<OperationalProviderTab>('openrouter');
  const [isOperational, setIsOperational] = useState<boolean>(() => {
    if (typeof document !== 'undefined') {
      return document.documentElement.dataset.theme === 'operational';
    }
    return false;
  });

  useEffect(() => {
    if (typeof document === 'undefined') return;
    const updateTheme = () => {
      setIsOperational(document.documentElement.dataset.theme === 'operational');
    };
    updateTheme();

    const observer = new MutationObserver((mutations) => {
      for (const m of mutations) {
        if (m.type === 'attributes' && (m.attributeName === 'data-theme' || m.attributeName === 'class')) {
          updateTheme();
        }
      }
    });

    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme', 'class'],
    });

    window.addEventListener('storage', updateTheme);
    return () => {
      observer.disconnect();
      window.removeEventListener('storage', updateTheme);
    };
  }, []);

  const handleSelectProviderTab = (tab: OperationalProviderTab) => {
    setOperationalTab(tab);
    if (!isOperational) {
      document.getElementById(`provider-${tab}`)?.scrollIntoView({ behavior: 'smooth' });
    }
  };

  // Accordion drawer states
  const [showUpstreamDetails, setShowUpstreamDetails] = useState<boolean>(false);

  // Modals visibility
  const [showAddProviderModal, setShowAddProviderModal] = useState(false);
  const [editingProvider, setEditingProvider] = useState<ProviderResponse | null>(null);

  // Upstream Form fields
  const [name, setName] = useState('');
  const [prefix, setPrefix] = useState('');
  const [providerType, setProviderType] = useState('openai');
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [modelsInput, setModelsInput] = useState('');
  const [isActive, setIsActive] = useState(true);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsLoadedFor, setModelsLoadedFor] = useState('');

  // Actions & Feedback
  const [actionLoading, setActionLoading] = useState(false);
  const [actionMessage, setActionMessage] = useState<{
    type: 'success' | 'error' | 'warning';
    text: string;
  } | null>(null);

  // OpenRouter Provider separation & state
  const openrouterProv = providers.find((p) => p.type === 'openrouter' || p.prefix === 'openrouter');
  const otherProviders = providers.filter((p) => p.type !== 'openrouter' && p.prefix !== 'openrouter');

  const CURATED_OPENROUTER_MODELS = [
    'deepseek/deepseek-r1',
    'deepseek/deepseek-chat',
    'meta-llama/llama-3.3-70b-instruct',
    'openai/gpt-4o',
    'openai/gpt-4o-mini',
    'google/gemini-2.5-pro',
    'google/gemini-2.5-flash',
    'qwen/qwen-2.5-coder-32b-instruct',
    'anthropic/claude-3.5-sonnet',
    'anthropic/claude-3.7-sonnet',
  ];

  const [openrouterCredits, setOpenrouterCredits] = useState<{ total_credits?: number; total_usage?: number; updated_at?: number } | null>(null);
  const [openrouterCreditsLoading, setOpenrouterCreditsLoading] = useState(false);
  const [showOpenRouterKeyModal, setShowOpenRouterKeyModal] = useState(false);
  const [openrouterKeyInput, setOpenrouterKeyInput] = useState('');
  const [openrouterKeySaving, setOpenrouterKeySaving] = useState(false);
  const [showOpenRouterModelModal, setShowOpenRouterModelModal] = useState(false);
  const [selectedOpenRouterModels, setSelectedOpenRouterModels] = useState<string[]>([]);
  const [allOpenRouterModels, setAllOpenRouterModels] = useState<Array<{ id: string; name?: string; context_length?: number }>>([]);
  const [allModelsLoading, setAllModelsLoading] = useState(false);
  const [openrouterModelSearch, setOpenrouterModelSearch] = useState('');
  const [showOpenRouterDetails, setShowOpenRouterDetails] = useState(true);

  const fetchOpenRouterCredits = async () => {
    setOpenrouterCreditsLoading(true);
    try {
      // 1. Fetch from server static cache (auto-synced by cron/refresh worker)
      try {
        const res = await fetch(`/assets/openrouter-credits.json?t=${Date.now()}`);
        if (res.ok) {
          const data = await res.json();
          if (data && typeof data.total_credits === 'number') {
            setOpenrouterCredits(data);
          }
        }
      } catch (_) {}

      // 2. Trigger test provider to ensure connection is live and get updated stats
      if (openrouterProv) {
        const testRes = (await api.testProvider(openrouterProv.id)) as any;
        if (testRes && testRes.credits) {
          setOpenrouterCredits(testRes.credits);
        }
      }
    } catch (e) {
      console.error('Failed to fetch openrouter credits:', e);
    } finally {
      setOpenrouterCreditsLoading(false);
    }
  };

  const loadAllOpenRouterModels = async () => {
    setAllModelsLoading(true);
    try {
      const [res, imgRes] = await Promise.allSettled([
        fetch('https://openrouter.ai/api/v1/models'),
        fetch('https://openrouter.ai/api/v1/images/models'),
      ]);
      const list: any[] = [];
      const seen = new Set<string>();

      if (res.status === 'fulfilled' && res.value.ok) {
        const json = await res.value.json();
        if (Array.isArray(json.data)) {
          for (const m of json.data) {
            if (m?.id && !seen.has(m.id)) {
              seen.add(m.id);
              list.push({
                id: m.id,
                name: m.name || m.id,
                context_length: m.context_length,
              });
            }
          }
        }
      }

      if (imgRes.status === 'fulfilled' && imgRes.value.ok) {
        const json = await imgRes.value.json();
        if (Array.isArray(json.data)) {
          for (const m of json.data) {
            if (m?.id && !seen.has(m.id)) {
              seen.add(m.id);
              list.push({
                id: m.id,
                name: m.name ? `${m.name} [Image]` : m.id,
                context_length: 0,
              });
            }
          }
        }
      }

      if (list.length > 0) {
        setAllOpenRouterModels(list);
      }
    } catch (e) {
      console.error('Failed to load OpenRouter models:', e);
    } finally {
      setAllModelsLoading(false);
    }
  };

  const handleToggleOpenRouterActive = async (prov: ProviderResponse) => {
    setActionLoading(true);
    try {
      await api.updateProvider(prov.id, {
        is_active: !prov.is_active,
      });
      setActionMessage({
        type: 'success',
        text: prov.is_active ? 'Đã tạm tắt OpenRouter.' : 'Đã kích hoạt OpenRouter.',
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Lỗi cập nhật trạng thái: ${err.message || err}`,
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleTestOpenRouter = async () => {
    if (!openrouterProv) return;
    setActionLoading(true);
    try {
      const res = await api.testProvider(openrouterProv.id);
      if (res.success) {
        setActionMessage({
          type: 'success',
          text: `Test kết nối OpenRouter thành công! Độ trễ: ${res.latency_ms.toFixed(1)}ms`,
        });
        fetchOpenRouterCredits();
      } else {
        setActionMessage({
          type: 'error',
          text: `Kết nối OpenRouter thất bại: ${res.message || 'Lỗi không xác định'}`,
        });
      }
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Lỗi test kết nối: ${err.message || err}`,
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleSaveOpenRouterKey = async () => {
    if (!openrouterKeyInput.trim()) return;
    setOpenrouterKeySaving(true);
    try {
      const cleanKey = openrouterKeyInput.trim();
      localStorage.setItem('ag_openrouter_raw_key', cleanKey);
      if (openrouterProv) {
        await api.updateProvider(openrouterProv.id, {
          api_key: cleanKey,
        });
      } else {
        await api.createProvider({
          name: 'OpenRouter',
          prefix: 'openrouter',
          type: 'openrouter',
          base_url: 'https://openrouter.ai/api/v1',
          api_key: cleanKey,
          models: CURATED_OPENROUTER_MODELS,
          is_active: true,
        });
      }
      setShowOpenRouterKeyModal(false);
      setActionMessage({
        type: 'success',
        text: 'Đã lưu API Key OpenRouter thành công!',
      });
      onRefresh();
      fetchOpenRouterCredits();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Lỗi lưu API Key: ${err.message || err}`,
      });
    } finally {
      setOpenrouterKeySaving(false);
    }
  };

  const handleSaveSelectedModels = async () => {
    if (!openrouterProv) return;
    setActionLoading(true);
    try {
      await api.updateProvider(openrouterProv.id, {
        models: selectedOpenRouterModels,
      });
      setShowOpenRouterModelModal(false);
      setActionMessage({
        type: 'success',
        text: `Đã lưu danh sách hiển thị: ${selectedOpenRouterModels.length} model OpenRouter!`,
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Lỗi lưu model: ${err.message || err}`,
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleRemoveSingleModel = async (modelId: string) => {
    if (!openrouterProv) return;
    const current = Array.isArray(openrouterProv.models) ? openrouterProv.models : [];
    const updated = current.filter((m: string) => m !== modelId);
    try {
      await api.updateProvider(openrouterProv.id, {
        models: updated,
      });
      setActionMessage({
        type: 'success',
        text: `Đã gỡ model ${modelId} khỏi danh sách hiển thị!`,
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Lỗi gỡ model: ${err.message || err}`,
      });
    }
  };

  // Background refresh of the OpenRouter credit snapshot
  useEffect(() => {
    fetchOpenRouterCredits();
    const interval = setInterval(async () => {
      try {
        const res = await fetch(`/assets/openrouter-credits.json?t=${Date.now()}`);
        if (!res.ok) return;
        const data = await res.json();
        if (data && typeof data.total_credits === 'number') {
          setOpenrouterCredits(data);
        }
      } catch {
        // Keep silent on background poll
      }
    }, 30000);
    return () => clearInterval(interval);
  }, []);

  // Upstream Handlers
  const resetUpstreamForm = () => {
    setName('');
    setPrefix('');
    setProviderType('openai');
    setBaseUrl('');
    setApiKey('');
    setModelsInput('');
    setModelsLoadedFor('');
    setIsActive(true);
    setEditingProvider(null);
  };

  const openAddUpstreamModal = () => {
    resetUpstreamForm();
    setShowAddProviderModal(true);
  };

  const handleFetchProviderModels = async (force = false) => {
    const url = baseUrl.trim();
    if (!url || modelsLoading || (!force && modelsLoadedFor === url)) return;
    setModelsLoading(true);
    setActionMessage(null);
    try {
      const res = await api.fetchProviderModels({
        type: providerType,
        base_url: url,
        api_key: apiKey.trim(),
      });
      const models = Array.isArray(res.models) ? res.models.filter(Boolean) : [];
      if (models.length === 0) {
        setActionMessage({ type: 'warning', text: t('providers.msgUpstreamNoModels') });
      } else {
        setModelsInput(JSON.stringify(models, null, 2));
        setActionMessage({ type: 'success', text: t('providers.msgUpstreamModelsFetched', { count: models.length }) });
      }
      setModelsLoadedFor(url);
    } catch (err: any) {
      setActionMessage({
        type: 'warning',
        text: t('providers.msgUpstreamFetchError', { error: err?.message || 'Lỗi kết nối upstream' }),
      });
    } finally {
      setModelsLoading(false);
    }
  };

  const openEditUpstreamModal = (p: ProviderResponse) => {
    setEditingProvider(p);
    setName(p.name);
    setPrefix(p.prefix);
    setProviderType(p.type);
    setBaseUrl(p.base_url);
    setApiKey(p.api_key);
    setModelsInput(
      typeof p.models === 'object'
        ? JSON.stringify(p.models, null, 2)
        : String(p.models || '')
    );
    setIsActive(p.is_active);
    setShowAddProviderModal(true);
  };

  const handleSaveUpstream = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !baseUrl.trim() || (!editingProvider && !prefix.trim())) {
      setActionMessage({
        type: 'error',
        text: t('providers.msgUpstreamFieldsRequired'),
      });
      return;
    }

    let parsedModels: any = [];
    if (modelsInput.trim()) {
      try {
        parsedModels = JSON.parse(modelsInput);
      } catch {
        parsedModels = modelsInput
          .split(',')
          .map((m) => m.trim())
          .filter(Boolean);
      }
    }

    setActionLoading(true);
    setActionMessage(null);
    try {
      if (editingProvider) {
        await api.updateProvider(editingProvider.id, {
          name: name.trim(),
          prefix: prefix.trim(),
          type: providerType,
          base_url: baseUrl.trim(),
          api_key: apiKey.trim(),
          models: parsedModels,
          is_active: isActive,
        });
        setActionMessage({
          type: 'success',
          text: t('providers.msgUpstreamUpdateSuccess', { name }),
        });
      } else {
        const payload: CreateProviderRequest = {
          name: name.trim(),
          prefix: prefix.trim(),
          type: providerType,
          base_url: baseUrl.trim(),
          api_key: apiKey.trim(),
          models: parsedModels,
          is_active: isActive,
        };
        await api.createProvider(payload);
        setActionMessage({
          type: 'success',
          text: t('providers.msgUpstreamCreateSuccess', { name }),
        });
      }
      setShowAddProviderModal(false);
      resetUpstreamForm();
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgUpstreamSaveError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDeleteUpstream = async (p: ProviderResponse) => {
    if (!window.confirm(t('providers.confirmUpstreamDelete', { name: p.name, prefix: p.prefix }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteProvider(p.id);
      setActionMessage({ type: 'success', text: t('providers.msgUpstreamDeleteSuccess', { name: p.name }) });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgUpstreamDeleteError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleTestUpstream = async (p: ProviderResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.testProvider(p.id);
      setActionMessage({
        type: 'success',
        text: t('providers.msgUpstreamTestSuccess', { name: p.name, detail: JSON.stringify(res) }),
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: t('providers.msgUpstreamTestError', { name: p.name, error: err?.message || 'Lỗi upstream' }),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleSyncUpstream = async (p: ProviderResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.syncModels(p.id);
      setActionMessage({
        type: 'success',
        text: t('providers.msgUpstreamSyncSuccess', { name: p.name, detail: JSON.stringify(res) }),
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: t('providers.msgUpstreamSyncError', { name: p.name, error: err?.message || 'Lỗi đồng bộ' }),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const activeProviderCount = otherProviders.filter((p) => p.is_active).length;
  const totalProviderCount = otherProviders.length + (openrouterProv ? 1 : 0);
  const activePoolCount = activeProviderCount + (openrouterProv?.is_active ? 1 : 0);

  // SVG Health Ring Helper with mount animation & non-duplicated labeling
  const renderHealthRing = (percent: number | null, label: string, subLabel?: string) => {
    const size = 68;
    const strokeWidth = 5;
    const radius = (size - strokeWidth) / 2;
    const circumference = 2 * Math.PI * radius;
    const validPct = percent !== null ? Math.max(0, Math.min(100, percent)) : 0;
    const offset = circumference - (validPct / 100) * circumference;
    const color =
      percent === null
        ? 'rgba(255,255,255,0.2)'
        : validPct >= 50
        ? '#10b981'
        : validPct >= 20
        ? '#f59e0b'
        : '#ef4444';

    return (
      <div className="visual-ring-col">
        <div style={{ position: 'relative', width: size, height: size }}>
          <svg width={size} height={size} className="health-ring-svg">
            <circle
              cx={size / 2}
              cy={size / 2}
              r={radius}
              strokeWidth={strokeWidth}
              className="health-ring-bg"
              fill="none"
            />
            <circle
              cx={size / 2}
              cy={size / 2}
              r={radius}
              strokeWidth={strokeWidth}
              stroke={color}
              fill="none"
              strokeDasharray={circumference}
              strokeDashoffset={percent === null ? circumference : offset}
              strokeLinecap="round"
              className={`health-ring-meter ${
                percent === null
                  ? 'meter-empty'
                  : validPct >= 50
                  ? 'meter-good'
                  : validPct >= 20
                  ? 'meter-warn'
                  : 'meter-bad'
              }`}
              style={{
                '--ring-circumference': `${circumference}px`,
                '--target-offset': `${percent === null ? circumference : offset}px`,
              } as React.CSSProperties}
            />
          </svg>
          <div
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: size,
              height: size,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontFamily: 'var(--font-mono)',
              fontSize: 14,
              fontWeight: 700,
              color: 'var(--text-primary)',
            }}
          >
            {percent !== null ? `${validPct}%` : 'N/A'}
          </div>
        </div>
        <div className="health-ring-label-group">
          <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)' }}>
            {label}
          </span>
          <span className="health-ring-desc">
            {percent !== null ? (subLabel || t('providers.quotaAvailable')) : t('providers.quotaNotLoaded')}
          </span>
        </div>
      </div>
    );
  };

  return (
    <div className="providers-view-container">
      {/* 0. OPERATIONAL-ONLY CONSOLE VIEW (TOOLBAR + SEGMENTED NAV + FOCUS SUMMARY) */}
      {isOperational && (
        <OperationalProvidersView
          activeTab={operationalTab}
          onSelectTab={setOperationalTab}
          onFullRefresh={() => {
            onRefresh();
          }}
          fullRefreshLoading={actionLoading || parentLoading}
          activePoolCount={activePoolCount}
          totalPoolCount={totalProviderCount}
          openrouterModelCount={openrouterProv && Array.isArray(openrouterProv.models) ? openrouterProv.models.length : 0}
          openrouterBalance={typeof openrouterCredits?.total_credits === 'number' ? `$${((openrouterCredits.total_credits || 0) - (openrouterCredits.total_usage || 0)).toFixed(2)}` : null}
          openrouterActive={!!openrouterProv?.is_active}
          upstreamActiveCount={otherProviders.filter((p) => p.is_active).length}
          upstreamTotalCount={otherProviders.length}
        />
      )}

      {/* 1. LEGACY COMPACT HEADER */}
      {!isOperational && (
        <div className="providers-compact-header">
          <div className="providers-header-info">
            <h2 className="providers-header-title">
              <span>{t('providers.headerTitle')}</span>
            </h2>
            <span className="providers-header-sub">
              {t('providers.headerSub')}
            </span>
          </div>
          <div className="section-actions">
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => {
                onRefresh();
              }}
              disabled={actionLoading || parentLoading}
              title={t('providers.refreshTitle')}
            >
              <IconRefresh size={15} />
              <span>{t('providers.refreshBtn')}</span>
            </button>
          </div>
        </div>
      )}

      {/* 2. LEGACY UNIFIED HORIZONTAL STATUS STRIP (3-SECOND SCAN) */}
      {!isOperational && (
        <div className="status-legend-bar">
          <div className="route-pulse-wrapper">
            <span
              className={`route-pulse-dot ${
                activePoolCount === 0 ? 'error' : ''
              }`}
            />
            <span>
              {activePoolCount > 0
                ? t('providers.routingReady')
                : t('providers.noActiveAccounts')}
            </span>
            <span style={{ color: 'var(--text-muted)', fontWeight: 400, fontSize: 11.5, marginLeft: 4 }}>
              {t('providers.readyCount', {
                active: activePoolCount,
                total: totalProviderCount,
              })}
            </span>
          </div>

          <div className="status-strip-chips">
            <button
              type="button"
              className="status-segment-chip"
              onClick={() => handleSelectProviderTab('openrouter')}
              title="Cuộn tới OpenRouter"
            >
              <IconOpenRouter size={14} />
              <span>
                OpenRouter: <strong>{openrouterProv && Array.isArray(openrouterProv.models) ? openrouterProv.models.length : 0} models</strong>
                {typeof openrouterCredits?.total_credits === 'number' && ` • $${((openrouterCredits.total_credits || 0) - (openrouterCredits.total_usage || 0)).toFixed(2)}`}
              </span>
              <span className={`status-dot-mini ${openrouterProv?.is_active ? 'active' : 'neutral'}`} />
            </button>

            <button
              type="button"
              className="status-segment-chip"
              onClick={() => handleSelectProviderTab('upstream')}
              title={t('providers.scrollToUpstream')}
            >
              <IconServer size={14} />
              <span>
                Upstream: <strong>{otherProviders.filter((p) => p.is_active).length}/{otherProviders.length}</strong>
              </span>
              <span className="status-dot-mini active" />
            </button>
          </div>

          <div className="status-legend-items">
            <span className="status-legend-item">
              <span className="status-dot-mini active" />
              <span>{t('providers.legendActive')}</span>
            </span>
            <span className="status-legend-item">
              <span className="status-dot-mini neutral" />
              <span>{t('providers.legendDisabled')}</span>
            </span>
          </div>
        </div>
      )}

      {/* Alerts */}
      {actionMessage && (
        <div
          className={`alert ${
            actionMessage.type === 'success'
              ? 'alert-success'
              : actionMessage.type === 'warning'
              ? 'alert-warning'
              : 'alert-error'
          }`}
          style={{ marginBottom: 4 }}
        >
          {actionMessage.type === 'success' ? (
            <IconCheck size={16} />
          ) : (
            <IconAlertCircle size={16} />
          )}
          <span>{actionMessage.text}</span>
        </div>
      )}

      {parentError && (
        <div className="alert alert-error" style={{ marginBottom: 4 }}>
          <IconAlertCircle size={16} />
          <span>{parentError}</span>
        </div>
      )}

      {/* Loading Skeleton */}
      {parentLoading && totalProviderCount === 0 && (
        <div className="provider-segment-container">
          <div className="skeleton-card">
            <div className="skeleton-bar" style={{ height: 32, width: '40%' }} />
            <div className="skeleton-bar" style={{ height: 80, width: '100%' }} />
            <div className="skeleton-bar" style={{ height: 40, width: '80%' }} />
          </div>
        </div>
      )}

      {/* TẤT CẢ PROVIDERS ĐƯỢC HIỂN THỊ ĐỒNG THỜI (KHÔNG CẦN CHIA TAB) */}
      <div className="provider-segment-container">
        {/* CARD 2: OPENROUTER FIRST-CLASS PROVIDER */}
        <div
          id="provider-openrouter"
          className={`visual-card op-provider-panel ${isOperational && operationalTab !== 'openrouter' ? 'op-hidden' : ''}`}
          role={isOperational ? 'tabpanel' : undefined}
          aria-labelledby={isOperational ? 'op-tab-openrouter' : undefined}
        >
          {/* Header */}
          <div className="visual-card-header">
            <div className="visual-brand-group">
              <div className="visual-logo-badge openrouter">
                <IconOpenRouter size={22} />
              </div>
              <div className="visual-brand-meta">
                <div className="visual-card-title">
                  <span>OpenRouter</span>
                </div>
                <div className="visual-chips-row">
                  <span className="model-family-chip">openrouter/*</span>
                  <span className="model-sub-chip">Claude 3.5</span>
                  <span className="model-sub-chip">DeepSeek R1</span>
                  <span className="model-sub-chip">Llama 3.3</span>
                  <span className="model-sub-chip">GPT-4o</span>
                </div>
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              {openrouterProv && (
                <span
                  className={`badge ${
                    !openrouterProv.api_key
                      ? 'badge-warning'
                      : !openrouterProv.is_active
                      ? 'badge-neutral'
                      : 'badge-success'
                  }`}
                >
                  {!openrouterProv.api_key
                    ? 'Chưa gắn Key'
                    : !openrouterProv.is_active
                    ? 'Tạm tắt'
                    : 'Đang hoạt động'}
                </span>
              )}
            </div>
          </div>

          {/* Metrics Row: Balance & Models */}
          <div className="visual-metrics-row">
            {/* Balance Card */}
            <div
              className="openrouter-balance-card"
              style={{
                flex: 1,
                minWidth: 220,
                padding: '12px 16px',
                borderRadius: 'var(--radius-md)',
                background: 'var(--bg-secondary)',
                border: '1px solid var(--border)',
                display: 'flex',
                flexDirection: 'column',
                gap: 4,
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)', fontWeight: 600, letterSpacing: '0.04em' }}>
                  SỐ DƯ TÀI KHOẢN (CREDITS)
                </span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  style={{ padding: '2px 8px', fontSize: 11, display: 'inline-flex', alignItems: 'center', gap: 4 }}
                  onClick={fetchOpenRouterCredits}
                  disabled={openrouterCreditsLoading}
                  title="Kiểm tra số dư mới nhất"
                >
                  <IconRefresh size={11} className={openrouterCreditsLoading ? 'spinner' : ''} />
                  <span>{openrouterCreditsLoading ? 'Đang tải...' : 'Làm mới'}</span>
                </button>
              </div>
              <div
                className={`openrouter-balance-val ${(openrouterCredits && typeof openrouterCredits.total_credits === 'number' && (openrouterCredits.total_credits - (openrouterCredits.total_usage || 0)) <= 0.05) ? 'balance-warn' : 'balance-good'}`}
                style={{ fontSize: 24, fontWeight: 700, color: (openrouterCredits && typeof openrouterCredits.total_credits === 'number' && (openrouterCredits.total_credits - (openrouterCredits.total_usage || 0)) <= 0.05) ? '#f59e0b' : '#10b981' }}
              >
                {openrouterCredits && typeof openrouterCredits.total_credits === 'number'
                  ? `$${Math.max(0, openrouterCredits.total_credits - (openrouterCredits.total_usage || 0)).toFixed(2)}`
                  : openrouterCreditsLoading
                  ? 'Đang kiểm tra...'
                  : '$0.00'}
              </div>
              <div style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                {openrouterCredits && typeof openrouterCredits.total_credits === 'number'
                  ? `Tổng nạp: $${openrouterCredits.total_credits.toFixed(2)} · Đã dùng: $${(openrouterCredits.total_usage || 0).toFixed(2)}${
                      openrouterCredits.updated_at
                        ? ` · Cập nhật: ${new Date(openrouterCredits.updated_at * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
                        : ''
                    }`
                  : openrouterProv?.api_key
                  ? `Key: ${openrouterProv.api_key}`
                  : 'Chưa cấu hình API Key'}
              </div>
              {openrouterCredits && typeof openrouterCredits.total_credits === 'number' && (openrouterCredits.total_credits - (openrouterCredits.total_usage || 0)) <= 0.01 && (
                <div style={{ marginTop: 4 }}>
                  <span className="badge badge-warning openrouter-low-balance-badge" style={{ fontSize: 10, padding: '1px 6px', background: 'rgba(245, 158, 11, 0.15)', color: '#f59e0b', borderColor: 'rgba(245, 158, 11, 0.3)' }}>
                    ⚡ Số dư: $0.00 (Chỉ dùng được model :free, cần nạp thêm để dùng model trả phí)
                  </span>
                </div>
              )}
            </div>

            {/* Stat Numbers */}
            <div className="visual-stat-numbers">
              <div className="stat-num-box">
                <span className="stat-num-val active">
                  {Array.isArray(openrouterProv?.models) ? openrouterProv.models.length : 0}
                </span>
                <span className="stat-num-lbl">Model Kích Hoạt</span>
              </div>
              <div className="stat-num-box">
                <span className="stat-num-val total">
                  458
                </span>
                <span className="stat-num-lbl">Tổng Model OR</span>
              </div>
            </div>
          </div>

          {/* Action Bar */}
          <div className="segment-action-bar">
            <div className="segment-actions-group" style={{ flexWrap: 'wrap', gap: 8 }}>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={() => {
                  setOpenrouterKeyInput(localStorage.getItem('ag_openrouter_raw_key') || '');
                  setShowOpenRouterKeyModal(true);
                }}
              >
                <IconKey size={14} />
                <span>{openrouterProv?.api_key ? 'Đổi API Key' : 'Gắn API Key'}</span>
              </button>

              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => {
                  setSelectedOpenRouterModels(
                    Array.isArray(openrouterProv?.models) ? [...openrouterProv.models] : []
                  );
                  setShowOpenRouterModelModal(true);
                  if (allOpenRouterModels.length === 0) {
                    loadAllOpenRouterModels();
                  }
                }}
              >
                <IconServer size={14} />
                <span>Chọn Model Hiển Thị ({Array.isArray(openrouterProv?.models) ? openrouterProv.models.length : 0})</span>
              </button>

              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={handleTestOpenRouter}
                disabled={actionLoading}
              >
                <IconZap size={14} />
                <span>Test Kết Nối</span>
              </button>

              {openrouterProv && (
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => handleToggleOpenRouterActive(openrouterProv)}
                  disabled={actionLoading}
                >
                  {openrouterProv.is_active ? 'Tạm Tắt' : 'Kích Hoạt'}
                </button>
              )}
            </div>

            <button
              className="details-toggle-btn"
              onClick={() => setShowOpenRouterDetails(!showOpenRouterDetails)}
              aria-expanded={showOpenRouterDetails}
            >
              <span>
                {showOpenRouterDetails
                  ? 'Thu gọn danh sách model'
                  : `Xem model đã chọn (${Array.isArray(openrouterProv?.models) ? openrouterProv.models.length : 0})`}
              </span>
              <IconChevronDown
                size={14}
                className={`details-toggle-chevron ${showOpenRouterDetails ? 'open' : ''}`}
              />
            </button>
          </div>

          {/* Expandable Model List Drawer */}
          <div
            className={`details-drawer-wrapper ${showOpenRouterDetails ? 'expanded' : 'collapsed'}`}
            aria-hidden={!showOpenRouterDetails}
          >
            <div className="details-drawer-content" style={{ padding: '14px 16px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
                <span style={{ fontSize: 13, fontWeight: 600 }}>
                  Danh sách model OpenRouter hiển thị trong Router ({Array.isArray(openrouterProv?.models) ? openrouterProv.models.length : 0}):
                </span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => {
                    setSelectedOpenRouterModels(
                      Array.isArray(openrouterProv?.models) ? [...openrouterProv.models] : []
                    );
                    setShowOpenRouterModelModal(true);
                    if (allOpenRouterModels.length === 0) {
                      loadAllOpenRouterModels();
                    }
                  }}
                >
                  <IconPlus size={13} />
                  <span>Chọn Thêm / Bớt Model</span>
                </button>
              </div>

              {(!openrouterProv?.models || !Array.isArray(openrouterProv.models) || openrouterProv.models.length === 0) ? (
                <div className="empty-graphic-box" style={{ padding: 20 }}>
                  <p style={{ margin: 0, fontSize: 13 }}>Chưa có model nào được chọn hiển thị.</p>
                  <button
                    className="btn btn-secondary btn-sm"
                    style={{ marginTop: 8 }}
                    onClick={() => {
                      setSelectedOpenRouterModels([...CURATED_OPENROUTER_MODELS]);
                      handleSaveSelectedModels();
                    }}
                  >
                    ⚡ Kích hoạt top model phổ biến
                  </button>
                </div>
              ) : (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {openrouterProv.models.map((m: string) => (
                    <div
                      key={m}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: 8,
                        padding: '6px 12px',
                        background: 'var(--bg-secondary)',
                        border: '1px solid var(--border)',
                        borderRadius: 'var(--radius-sm)',
                        fontSize: 12.5,
                      }}
                    >
                      <span className="badge badge-neutral openrouter-model-badge" style={{ fontSize: 10, padding: '1px 5px', color: '#818cf8', borderColor: 'rgba(99, 102, 241, 0.3)' }}>
                        OR
                      </span>
                      <span className="font-mono" style={{ fontWeight: 600 }}>
                        openrouter/{m}
                      </span>
                      <button
                        type="button"
                        style={{
                          background: 'none',
                          border: 'none',
                          color: 'var(--text-muted)',
                          cursor: 'pointer',
                          padding: 0,
                          display: 'flex',
                        }}
                        title="Bỏ model này"
                        onClick={() => handleRemoveSingleModel(m)}
                      >
                        <IconX size={14} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* CARD 4: UPSTREAM CUSTOM PROVIDERS */}
        <div
          id="provider-upstream"
          className={`visual-card op-provider-panel ${isOperational && operationalTab !== 'upstream' ? 'op-hidden' : ''}`}
          role={isOperational ? 'tabpanel' : undefined}
          aria-labelledby={isOperational ? 'op-tab-upstream' : undefined}
        >
            {/* Header */}
            <div className="visual-card-header">
              <div className="visual-brand-group">
                <div className="visual-logo-badge upstream">
                  <IconServer size={22} />
                </div>
                <div className="visual-brand-meta">
                  <div className="visual-card-title">
                    <span>Upstream Tùy Biến (Custom Providers)</span>
                  </div>
                  <div className="visual-chips-row">
                    <span className="model-family-chip">upstream/*</span>
                    <span className="model-sub-chip">OpenAI / DeepSeek / Claude</span>
                  </div>
                </div>
              </div>
              <span className="badge badge-neutral font-mono">{t('providers.sourcesCount', { count: otherProviders.length })}</span>
            </div>

            {/* Health Ring & Big Numbers */}
            <div className="visual-metrics-row">
              {renderHealthRing(
                otherProviders.length > 0
                  ? Math.round((otherProviders.filter((p) => p.is_active).length / otherProviders.length) * 100)
                  : 0,
                t('providers.upstreamActiveRate'),
                t('providers.sourcesReady')
              )}
              <div className="visual-stat-numbers">
                <div className="stat-num-box">
                  <span className="stat-num-val active">
                    {otherProviders.filter((p) => p.is_active).length}
                  </span>
                  <span className="stat-num-lbl">{t('providers.statActive')}</span>
                </div>
                <div className="stat-num-box">
                  <span className="stat-num-val total">{otherProviders.length}</span>
                  <span className="stat-num-lbl">{t('providers.statConfigured')}</span>
                </div>
              </div>
            </div>

            {/* Consolidated Action Bar */}
            <div className="segment-action-bar">
              <div className="segment-actions-group">
                <button
                  className="action-icon-btn primary"
                  onClick={openAddUpstreamModal}
                  disabled={actionLoading}
                  title={t('providers.addUpstreamTitle')}
                  aria-label={t('providers.addUpstreamTitle')}
                >
                  <IconPlus size={14} />
                  <span>{t('providers.addUpstreamBtn')}</span>
                </button>
              </div>

              <button
                className="details-toggle-btn"
                onClick={() => setShowUpstreamDetails(!showUpstreamDetails)}
                aria-expanded={showUpstreamDetails}
                aria-controls="upstream-details-drawer"
              >
                <span>
                  {showUpstreamDetails
                    ? t('providers.collapseUpstream')
                    : t('providers.upstreamDetails', { count: otherProviders.length })}
                </span>
                <IconChevronDown
                  size={14}
                  className={`details-toggle-chevron ${showUpstreamDetails ? 'open' : ''}`}
                />
              </button>
            </div>

            {/* Expandable Upstream Details Panel with Accessible CSS Transition */}
            <div
              id="upstream-details-drawer"
              className={`details-drawer-wrapper ${showUpstreamDetails ? 'expanded' : 'collapsed'}`}
              aria-hidden={!showUpstreamDetails}
            >
              <div className="details-drawer-content">
                {otherProviders.length === 0 ? (
                  <div className="empty-graphic-box">
                    <p style={{ margin: 0, fontSize: 13 }}>{t('providers.emptyUpstream')}</p>
                    <button className="btn btn-secondary btn-sm" onClick={openAddUpstreamModal}>
                      {t('providers.addFirstUpstream')}
                    </button>
                  </div>
                ) : (
                  <div className="table-container">
                    <table>
                      <thead>
                        <tr>
                          <th>{t('providers.thNamePrefix')}</th>
                          <th>{t('providers.thType')}</th>
                          <th>{t('providers.thBaseUrl')}</th>
                          <th>{t('providers.thApiKey')}</th>
                          <th>{t('providers.thModels')}</th>
                          <th>{t('providers.thStatus')}</th>
                          <th>{t('providers.thActions')}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {otherProviders.map((p) => {
                          const modelsArr = Array.isArray(p.models)
                            ? p.models
                            : typeof p.models === 'object' && p.models
                            ? Object.keys(p.models)
                            : [];
                          return (
                            <tr key={p.id}>
                              <td data-label={t('providers.thNamePrefix')}>
                                <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>{p.name}</div>
                                <span className="badge badge-neutral font-mono" style={{ fontSize: 10, marginTop: 2 }}>
                                  {p.prefix}/*
                                </span>
                              </td>
                              <td data-label={t('providers.thType')}>
                                <span className="badge badge-neutral">{p.type}</span>
                              </td>
                              <td data-label={t('providers.thBaseUrl')} className="font-mono text-break" style={{ fontSize: 12, maxWidth: 180 }}>
                                {p.base_url}
                              </td>
                              <td data-label={t('providers.thApiKey')}>
                                <span className="font-mono" style={{ fontSize: 11, color: 'var(--text-muted)' }}>
                                  {p.api_key ? '••••••••' : t('providers.emptyValue')}
                                </span>
                              </td>
                              <td data-label={t('providers.thModels')}>
                                <span className="badge badge-neutral" style={{ fontSize: 10 }}>
                                  {modelsArr.length > 0 ? t('providers.modelsCount', { count: modelsArr.length }) : t('providers.modelsDefault')}
                                </span>
                              </td>
                              <td data-label={t('providers.thStatus')}>
                                <span className={`badge ${p.is_active ? 'badge-success' : 'badge-neutral'}`}>
                                  {p.is_active ? t('providers.statusOn') : t('providers.statusOff')}
                                </span>
                              </td>
                              <td data-label={t('providers.thActions')}>
                                <div className="table-actions">
                                  <button
                                    className="btn btn-secondary btn-sm btn-icon-only"
                                    onClick={() => openEditUpstreamModal(p)}
                                    title={t('providers.editConfig')}
                                  >
                                    <IconEdit size={14} />
                                  </button>
                                  <button
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => handleTestUpstream(p)}
                                    disabled={actionLoading}
                                    title={t('providers.testConnection')}
                                  >
                                    {t('providers.btnTest')}
                                  </button>
                                  <button
                                    className="btn btn-secondary btn-sm btn-icon-only"
                                    onClick={() => handleSyncUpstream(p)}
                                    disabled={actionLoading}
                                    title={t('providers.syncModels')}
                                  >
                                    <IconRefresh size={14} />
                                  </button>
                                  <button
                                    className="btn btn-danger btn-sm btn-icon-only"
                                    onClick={() => handleDeleteUpstream(p)}
                                    disabled={actionLoading}
                                    title={t('providers.deleteProvider')}
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
            </div>
          </div>
      </div>

      {/* MODAL: UPSTREAM ADD/EDIT */}
      {showAddProviderModal && (
        <div className="modal-backdrop" onClick={() => setShowAddProviderModal(false)}>
          <div
            className="modal-card"
            style={{ maxWidth: 540 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title">
                {editingProvider ? t('providers.modalUpstreamEditTitle') : t('providers.modalUpstreamAddTitle')}
              </h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddProviderModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleSaveUpstream}>
              {!editingProvider && (
                <div style={{ display: 'flex', gap: 6, marginBottom: 14, flexWrap: 'wrap', alignItems: 'center' }}>
                  <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Mẫu nhanh:</span>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    style={{ fontSize: 12, padding: '3px 8px' }}
                    onClick={() => {
                      setName('OpenRouter');
                      setPrefix('openrouter');
                      setProviderType('openrouter');
                      setBaseUrl('https://openrouter.ai/api/v1');
                    }}
                  >
                    ⚡ OpenRouter
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    style={{ fontSize: 12, padding: '3px 8px' }}
                    onClick={() => {
                      setName('DeepSeek');
                      setPrefix('deepseek');
                      setProviderType('openai');
                      setBaseUrl('https://api.deepseek.com/v1');
                    }}
                  >
                    ⚡ DeepSeek
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    style={{ fontSize: 12, padding: '3px 8px' }}
                    onClick={() => {
                      setName('Groq');
                      setPrefix('groq');
                      setProviderType('openai');
                      setBaseUrl('https://api.groq.com/openai/v1');
                    }}
                  >
                    ⚡ Groq
                  </button>
                </div>
              )}

              <div className="form-group">
                <label>{t('providers.labelProviderName')}</label>
                <input
                  type="text"
                  placeholder={t('providers.placeholderProviderName')}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                />
              </div>

              <div className="form-group">
                <label>{t('providers.labelRoutingPrefix')}</label>
                <input
                  type="text"
                  placeholder={t('providers.placeholderRoutingPrefix')}
                  value={prefix}
                  onChange={(e) => setPrefix(e.target.value)}
                  disabled={!!editingProvider}
                  required
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  {t('providers.helpRoutingPrefix', { prefix: prefix || 'prefix' })}
                </span>
              </div>

              <div className="form-group">
                <label>{t('providers.labelProviderType')}</label>
                <select
                  value={providerType}
                  onChange={(e) => {
                    const nextType = e.target.value;
                    setProviderType(nextType);
                    if (!editingProvider) {
                      if (nextType === 'openrouter') {
                        if (!name || name === 'DeepSeek' || name === 'Groq' || name === 'OpenAI') setName('OpenRouter');
                        if (!prefix || prefix === 'deepseek' || prefix === 'groq' || prefix === 'openai') setPrefix('openrouter');
                        if (!baseUrl || baseUrl.includes('deepseek') || baseUrl.includes('groq')) setBaseUrl('https://openrouter.ai/api/v1');
                      } else if (nextType === 'openai') {
                        if (name === 'OpenRouter') setName('');
                        if (prefix === 'openrouter') setPrefix('');
                        if (baseUrl === 'https://openrouter.ai/api/v1') setBaseUrl('');
                      } else if (nextType === 'gemini') {
                        if (!name || name === 'OpenRouter') setName('Google Gemini');
                        if (!prefix || prefix === 'openrouter') setPrefix('gemini');
                        if (!baseUrl || baseUrl === 'https://openrouter.ai/api/v1') setBaseUrl('https://generativelanguage.googleapis.com/v1beta');
                      } else if (nextType === 'anthropic') {
                        if (!name || name === 'OpenRouter') setName('Anthropic Claude');
                        if (!prefix || prefix === 'openrouter') setPrefix('anthropic');
                        if (!baseUrl || baseUrl === 'https://openrouter.ai/api/v1') setBaseUrl('https://api.anthropic.com/v1');
                      }
                    }
                  }}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: 'var(--radius-sm)',
                    border: '1px solid var(--border)',
                    background: 'var(--bg-secondary)',
                    color: 'var(--text-primary)',
                  }}
                >
                  <option value="openai">OpenAI Compatible</option>
                  <option value="gemini">Google Gemini</option>
                  <option value="anthropic">Anthropic Claude</option>
                  <option value="openrouter">OpenRouter</option>
                </select>
              </div>

              <div className="form-group">
                <label>{t('providers.labelBaseUrl')}</label>
                <input
                  type="text"
                  placeholder="https://api.deepseek.com/v1"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  onBlur={() => { void handleFetchProviderModels(); }}
                  required
                />
              </div>

              <div className="form-group">
                <label>{t('providers.labelApiKey')}</label>
                <input
                  type="text"
                  placeholder="sk-..."
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
                  <label style={{ marginBottom: 0 }}>{t('providers.labelModelsList')}</label>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => { void handleFetchProviderModels(true); }}
                    disabled={modelsLoading || !baseUrl.trim()}
                  >
                    {modelsLoading ? <div className="spinner" /> : <IconRefresh size={14} />}
                    <span>{modelsLoading ? t('providers.btnLoadingModels') : t('providers.btnFetchModels')}</span>
                  </button>
                </div>
                <textarea
                  rows={3}
                  placeholder='["deepseek-chat", "deepseek-coder"]'
                  value={modelsInput}
                  onChange={(e) => setModelsInput(e.target.value)}
                  style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                />
              </div>

              <div
                className="form-group"
                style={{ display: 'flex', alignItems: 'center', gap: 10 }}
              >
                <input
                  type="checkbox"
                  id="upIsActive"
                  checked={isActive}
                  onChange={(e) => setIsActive(e.target.checked)}
                  style={{ width: 18, height: 18 }}
                />
                <label htmlFor="upIsActive" style={{ marginBottom: 0, cursor: 'pointer' }}>
                  {t('providers.labelActivateProvider')}
                </label>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowAddProviderModal(false)}
                >
                  {t('providers.btnCancel')}
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : t('providers.btnSaveProvider')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* MODAL: OPENROUTER API KEY */}
      {showOpenRouterKeyModal && (
        <div className="modal-backdrop" onClick={() => setShowOpenRouterKeyModal(false)}>
          <div className="modal-card" style={{ maxWidth: 480 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3 className="modal-title">Cấu hình API Key OpenRouter</h3>
              <button
                type="button"
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowOpenRouterKeyModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>
            <div className="form-group" style={{ marginTop: 12 }}>
              <label className="form-label" style={{ fontWeight: 600 }}>
                OpenRouter API Key (sk-or-v1-...)
              </label>
              <input
                type="password"
                className="form-input font-mono"
                placeholder="sk-or-v1-..."
                value={openrouterKeyInput}
                onChange={(e) => setOpenrouterKeyInput(e.target.value)}
                autoFocus
              />
              <span className="form-help" style={{ marginTop: 6, display: 'block', fontSize: 12 }}>
                Lấy API key tại{' '}
                <a
                  href="https://openrouter.ai/settings/keys"
                  target="_blank"
                  rel="noreferrer"
                  style={{ color: 'var(--accent-primary)', textDecoration: 'underline' }}
                >
                  openrouter.ai/settings/keys
                </a>
                . Key được dùng để xác thực và truy vấn số dư tài khoản.
              </span>
            </div>
            <div className="modal-actions" style={{ marginTop: 16 }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setShowOpenRouterKeyModal(false)}
              >
                Hủy
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleSaveOpenRouterKey}
                disabled={openrouterKeySaving || !openrouterKeyInput.trim()}
              >
                {openrouterKeySaving ? <div className="spinner" /> : 'Lưu & Kiểm Tra Số Dư'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* MODAL: OPENROUTER MODEL SELECTOR */}
      {showOpenRouterModelModal && (
        <div className="modal-backdrop" onClick={() => setShowOpenRouterModelModal(false)}>
          <div
            className="modal-card"
            style={{ maxWidth: 680, maxHeight: '85vh', display: 'flex', flexDirection: 'column' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <div>
                <h3 className="modal-title">Chọn Model OpenRouter Hiển Thị</h3>
                <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 2 }}>
                  Chỉ các model được chọn mới xuất hiện trong <code>/v1/models</code>, tránh làm loãng danh sách ({selectedOpenRouterModels.length} đã chọn).
                </div>
              </div>
              <button
                type="button"
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowOpenRouterModelModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            {/* Quick Actions & Search */}
            <div style={{ padding: '12px 0', borderBottom: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: 10 }}>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center', justifyContent: 'space-between' }}>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => {
                      const merged = Array.from(new Set([...selectedOpenRouterModels, ...CURATED_OPENROUTER_MODELS]));
                      setSelectedOpenRouterModels(merged);
                    }}
                  >
                    ⭐ Thêm Top Model Hot
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => setSelectedOpenRouterModels([])}
                  >
                    Bỏ chọn tất cả
                  </button>
                </div>
                <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                  Đã chọn: <strong style={{ color: 'var(--accent-primary)' }}>{selectedOpenRouterModels.length}</strong> model
                </span>
              </div>

              <div style={{ position: 'relative' }}>
                <input
                  type="text"
                  className="form-input"
                  style={{ paddingLeft: 32 }}
                  placeholder="Tìm kiếm model (vd: claude, deepseek, llama, qwen, gpt, flash, pro...)"
                  value={openrouterModelSearch}
                  onChange={(e) => setOpenrouterModelSearch(e.target.value)}
                />
                <span style={{ position: 'absolute', left: 10, top: '50%', transform: 'translateY(-50%)', color: 'var(--text-muted)' }}>
                  <IconSearch size={14} />
                </span>
              </div>
            </div>

            {/* Scrollable Model List */}
            <div style={{ flex: 1, overflowY: 'auto', padding: '10px 0', minHeight: 300, maxHeight: 420 }}>
              {allModelsLoading ? (
                <div style={{ textAlign: 'center', padding: 40, color: 'var(--text-muted)' }}>
                  <div className="spinner" style={{ margin: '0 auto 8px' }} />
                  Đang tải danh sách model từ OpenRouter...
                </div>
              ) : (() => {
                const modelList = allOpenRouterModels.length > 0
                  ? allOpenRouterModels
                  : Array.from(new Set([...CURATED_OPENROUTER_MODELS, ...selectedOpenRouterModels])).map((id) => ({
                      id,
                      name: id,
                      context_length: undefined,
                    }));

                const query = openrouterModelSearch.trim().toLowerCase();
                const filtered = modelList.filter((m) =>
                  !query || m.id.toLowerCase().includes(query) || (m.name && m.name.toLowerCase().includes(query))
                );

                if (filtered.length === 0) {
                  return (
                    <div style={{ textAlign: 'center', padding: 30, color: 'var(--text-muted)', fontSize: 13 }}>
                      Không tìm thấy model khớp với "{openrouterModelSearch}"
                    </div>
                  );
                }

                return (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {filtered.map((m) => {
                      const isSelected = selectedOpenRouterModels.includes(m.id);
                      return (
                        <label
                          key={m.id}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            padding: '8px 12px',
                            borderRadius: 'var(--radius-sm)',
                            background: isSelected ? 'rgba(99, 102, 241, 0.08)' : 'var(--bg-secondary)',
                            border: `1px solid ${isSelected ? 'rgba(99, 102, 241, 0.3)' : 'var(--border-subtle)'}`,
                            cursor: 'pointer',
                          }}
                        >
                          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                            <input
                              type="checkbox"
                              checked={isSelected}
                              onChange={(e) => {
                                if (e.target.checked) {
                                  setSelectedOpenRouterModels([...selectedOpenRouterModels, m.id]);
                                } else {
                                  setSelectedOpenRouterModels(selectedOpenRouterModels.filter((id) => id !== m.id));
                                }
                              }}
                            />
                            <div>
                              <div className={`font-mono openrouter-picker-model ${isSelected ? 'is-selected' : ''}`} style={{ fontSize: 13, fontWeight: isSelected ? 600 : 500, color: isSelected ? '#818cf8' : 'var(--text-primary)' }}>
                                openrouter/{m.id}
                              </div>
                              {m.name && m.name !== m.id && (
                                <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>{m.name}</div>
                              )}
                            </div>
                          </div>
                          {m.context_length && (
                            <span className="badge badge-neutral font-mono" style={{ fontSize: 10 }}>
                              {Math.round(m.context_length / 1024)}k ctx
                            </span>
                          )}
                        </label>
                      );
                    })}
                  </div>
                );
              })()}
            </div>

            {/* Modal Footer */}
            <div className="modal-actions" style={{ borderTop: '1px solid var(--border)', paddingTop: 12 }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setShowOpenRouterModelModal(false)}
              >
                Hủy
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleSaveSelectedModels}
                disabled={actionLoading}
              >
                {actionLoading ? <div className="spinner" /> : `Lưu ${selectedOpenRouterModels.length} Model Đã Chọn`}
              </button>
            </div>
          </div>
        </div>
      )}


    </div>
  );
};
