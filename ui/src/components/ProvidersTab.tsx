import React, { useState, useEffect } from 'react';
import {
  ProviderResponse,
  CreateProviderRequest,
  AccountResponse,
  CodexAccountRecord,
  CodexStatusResponse,
  QuotaRefreshStatus,
} from '../types';
import { api } from '../api';
import { useI18n } from '../i18n';
import {
  IconPlus,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconRefresh,
  IconTrash,
  IconEdit,
  IconServer,
  IconShield,
  IconGoogle,
  IconOpenAI,
  IconChevronDown,
  IconOpenRouter,
  IconKey,
  IconZap,
  IconSearch,
} from '../icons';

interface ProvidersTabProps {
  providers: ProviderResponse[];
  accounts?: AccountResponse[];
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

export const ProvidersTab: React.FC<ProvidersTabProps> = ({
  providers,
  accounts = [],
  loading: parentLoading,
  error: parentError,
  onRefresh,
}) => {
  const { locale, t } = useI18n();
  // Accordion drawer states
  const [showGoogleDetails, setShowGoogleDetails] = useState<boolean>(false);
  const [showCodexDetails, setShowCodexDetails] = useState<boolean>(false);
  const [showUpstreamDetails, setShowUpstreamDetails] = useState<boolean>(false);

  // Codex state
  const [codexStatus, setCodexStatus] = useState<CodexStatusResponse | null>(null);
  const [codexAccounts, setCodexAccounts] = useState<CodexAccountRecord[]>([]);
  const [codexLoading, setCodexLoading] = useState(false);

  // Modals visibility
  const [showAddProviderModal, setShowAddProviderModal] = useState(false);
  const [editingProvider, setEditingProvider] = useState<ProviderResponse | null>(null);
  const [showGoogleAddModal, setShowGoogleAddModal] = useState(false);
  const [showGoogleOAuthModal, setShowGoogleOAuthModal] = useState(false);
  const [showCodexAddModal, setShowCodexAddModal] = useState(false);
  const [showCodexOAuthModal, setShowCodexOAuthModal] = useState(false);
  const [selectedQuota, setSelectedQuota] = useState<{
    title: string;
    quota: any;
    onRefreshQuota?: () => void;
  } | null>(null);

  // Codex Reset Credit Modal state
  const [resetCreditModalAccount, setResetCreditModalAccount] = useState<CodexAccountRecord | null>(null);
  const [resetCreditsList, setResetCreditsList] = useState<any[]>([]);
  const [resetCreditsLoading, setResetCreditsLoading] = useState(false);
  const [selectedCreditId, setSelectedCreditId] = useState<string>('');
  const [consumingCredit, setConsumingCredit] = useState(false);

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

  // Google Form & OAuth fields
  const [googleEmail, setGoogleEmail] = useState('');
  const [googleRefreshToken, setGoogleRefreshToken] = useState('');
  const [googleOAuthUrl, setGoogleOAuthUrl] = useState('');
  const [googleOAuthCode, setGoogleOAuthCode] = useState('');
  const [googleOAuthTicket, setGoogleOAuthTicket] = useState<{
    state?: string;
    authorize_url?: string;
    redirect_uri?: string;
  } | null>(null);

  // Codex Form & OAuth fields
  const [codexAuthPath, setCodexAuthPath] = useState('');
  const [codexEmail, setCodexEmail] = useState('');
  const [codexOAuthTicket, setCodexOAuthTicket] = useState<{
    ticket_id: string;
    state?: string;
    authorize_url?: string;
    authorization_url?: string;
  } | null>(null);
  const [codexOAuthCode, setCodexOAuthCode] = useState('');
  const [codexOAuthStatus, setCodexOAuthStatus] = useState<string>('');

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
      const res = await fetch('https://openrouter.ai/api/v1/models');
      if (res.ok) {
        const json = await res.json();
        if (Array.isArray(json.data)) {
          setAllOpenRouterModels(
            json.data.map((m: any) => ({
              id: m.id,
              name: m.name || m.id,
              context_length: m.context_length,
            }))
          );
        }
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

  // Load Codex data
  const loadCodexData = async () => {
    setCodexLoading(true);
    try {
      const [statusRes, accRes] = await Promise.all([
        api.getCodexStatus(),
        api.getCodexAccounts(),
      ]);
      setCodexStatus(statusRes);
      const accList = Array.isArray(accRes) ? accRes : (accRes as any)?.accounts || [];
      setCodexAccounts(accList);
    } catch {
      // Keep silent on background poll
    } finally {
      setCodexLoading(false);
    }
  };

  // Auto Quota Refresh State
  const [quotaRefresh, setQuotaRefresh] = useState<QuotaRefreshStatus | null>(null);
  const [quotaRefreshLoading, setQuotaRefreshLoading] = useState(false);

  const loadQuotaRefreshStatus = async () => {
    try {
      const res = await api.getQuotaRefreshStatus();
      setQuotaRefresh(res);
      if (res.openrouter_credits) {
        setOpenrouterCredits(res.openrouter_credits);
      } else {
        try {
          const cRes = await fetch(`/assets/openrouter-credits.json?t=${Date.now()}`);
          if (cRes.ok) {
            const data = await cRes.json();
            if (data && typeof data.total_credits === 'number') {
              setOpenrouterCredits(data);
            }
          }
        } catch (_) {}
      }
    } catch {
      // Keep silent on background poll
    }
  };

  const handleToggleAutoRefresh = async () => {
    if (!quotaRefresh) return;
    const newEnabled = !quotaRefresh.enabled;
    setQuotaRefreshLoading(true);
    try {
      const res = await api.updateQuotaRefreshSettings({ enabled: newEnabled });
      setQuotaRefresh(res);
      setActionMessage({
        type: 'success',
        text: newEnabled
          ? t('providers.msgAutoRefreshEnabled')
          : t('providers.msgAutoRefreshDisabled'),
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgAutoRefreshUpdateError'),
      });
    } finally {
      setQuotaRefreshLoading(false);
    }
  };

  const handleChangeRefreshInterval = async (intervalSecs: number) => {
    setQuotaRefreshLoading(true);
    try {
      const res = await api.updateQuotaRefreshSettings({ interval_secs: intervalSecs });
      setQuotaRefresh(res);
      setActionMessage({
        type: 'success',
        text: t('providers.msgIntervalChanged', { min: intervalSecs / 60 }),
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgIntervalError'),
      });
    } finally {
      setQuotaRefreshLoading(false);
    }
  };

  const handleManualRunRefresh = async () => {
    setQuotaRefreshLoading(true);
    setActionMessage(null);
    try {
      const res = await api.runQuotaRefresh();
      setQuotaRefresh(res);
      setActionMessage({
        type: 'success',
        text: t('providers.msgManualRefreshSuccess', {
          duration: res.last_summary?.duration_ms || 0,
          google: res.last_summary?.google_refreshed || 0,
          codex: res.last_summary?.codex_refreshed || 0,
        }),
      });
      onRefresh();
      loadCodexData();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgManualRefreshError'),
      });
    } finally {
      setQuotaRefreshLoading(false);
    }
  };

  const formatTimestamp = (ts: number | null) => {
    if (!ts) return t('providers.neverRun');
    const date = new Date(ts * 1000);
    return date.toLocaleTimeString(locale === 'vi' ? 'vi-VN' : 'en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  };

  const formatCountdown = (nextTs: number | null) => {
    if (!nextTs || !quotaRefresh?.enabled) return '-';
    const diff = Math.round(nextTs - Date.now() / 1000);
    if (diff <= 0) return t('providers.autoRefreshDue');
    if (diff < 60) return t('providers.autoRefreshInSec', { sec: diff });
    const m = Math.floor(diff / 60);
    const s = diff % 60;
    return t('providers.autoRefreshInMinSec', { min: m, sec: s });
  };

  useEffect(() => {
    loadCodexData();
    loadQuotaRefreshStatus();
    fetchOpenRouterCredits();
    const interval = setInterval(() => {
      loadQuotaRefreshStatus();
      loadCodexData();
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  // Listen for Google OAuth popup message
  useEffect(() => {
    const handleWindowMessage = (event: MessageEvent) => {
      if (event.data && event.data.type === 'ag-account-added') {
        const addedEmail = event.data.email || '';
        const isNew = event.data.is_new !== false;
        setActionMessage({
          type: isNew ? 'success' : 'warning',
          text: isNew
            ? t('providers.msgGoogleAccountAdded', { email: addedEmail })
            : t('providers.msgGoogleAccountRefreshed', { email: addedEmail }),
        });
        setShowGoogleOAuthModal(false);
        setGoogleOAuthCode('');
        setGoogleOAuthTicket(null);
        onRefresh();
      }
    };
    window.addEventListener('message', handleWindowMessage);
    return () => window.removeEventListener('message', handleWindowMessage);
  }, [onRefresh, t]);

  // Poll Codex OAuth ticket status if modal is open and pending
  useEffect(() => {
    if (!showCodexOAuthModal || !codexOAuthTicket) return;
    const ticketId = codexOAuthTicket.ticket_id || codexOAuthTicket.state;
    if (!ticketId) return;

    let stopped = false;
    const interval = setInterval(async () => {
      if (stopped) return;
      try {
        const res = await api.getCodexOAuthStatus(ticketId);
        if (res?.status) {
          setCodexOAuthStatus(res.status);
          if (res.status === 'done' || res.status === 'completed') {
            stopped = true;
            clearInterval(interval);
            setActionMessage({
              type: 'success',
              text: t('providers.msgCodexOAuthSuccess', { email: res.email || '' }),
            });
            setShowCodexOAuthModal(false);
            setCodexOAuthTicket(null);
            loadCodexData();
          } else if (res.status === 'error') {
            stopped = true;
            clearInterval(interval);
            setActionMessage({
              type: 'error',
              text: t('providers.msgCodexOAuthError', { error: res.error || res.message || 'Thất bại' }),
            });
          }
        }
      } catch {
        // Continue polling
      }
    }, 3000);

    return () => {
      stopped = true;
      clearInterval(interval);
    };
  }, [showCodexOAuthModal, codexOAuthTicket, t]);

  // Google Handlers
  const handleStartGoogleOAuth = async () => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const origin = window.location.origin || '';
      const res = await api.startGoogleOAuth(origin);
      const ticket = {
        state: (res as any).state || '',
        authorize_url: (res as any).authorize_url || (res as any).authorization_url || '',
        redirect_uri: (res as any).redirect_uri || '',
      };
      setGoogleOAuthTicket(ticket);
      setGoogleOAuthUrl(ticket.authorize_url);
      setShowGoogleOAuthModal(true);

      if (ticket.authorize_url) {
        const popup = window.open(ticket.authorize_url, 'ag-oauth', 'width=680,height=760,popup=yes');
        if (popup) {
          setActionMessage({
            type: 'success',
            text: t('providers.msgGoogleOAuthPopupOpened'),
          });
        }
      }
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgGoogleOAuthInitError'),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleExchangeGoogleOAuth = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!googleOAuthCode.trim()) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.exchangeGoogleOAuth({
        code: googleOAuthCode.trim(),
        state: googleOAuthTicket?.state,
        redirect_uri: googleOAuthTicket?.redirect_uri,
      });
      const isNew = (res as any)?.is_new !== false;
      setActionMessage({
        type: isNew ? 'success' : 'warning',
        text: isNew
          ? t('providers.msgGoogleOAuthExchangeSuccess', { email: res.email })
          : t('providers.msgGoogleOAuthExchangeRefreshed', { email: res.email }),
      });
      setShowGoogleOAuthModal(false);
      setGoogleOAuthCode('');
      setGoogleOAuthTicket(null);
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgGoogleOAuthExchangeError'),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleAddGoogleAccount = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!googleRefreshToken.trim()) {
      setActionMessage({ type: 'error', text: t('providers.msgGoogleTokenRequired') });
      return;
    }
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.createAccount({
        email: googleEmail.trim() || undefined,
        refresh_token: googleRefreshToken.trim(),
      });
      setActionMessage({ type: 'success', text: t('providers.msgGoogleAddSuccess') });
      setShowGoogleAddModal(false);
      setGoogleEmail('');
      setGoogleRefreshToken('');
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgGoogleAddError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleResetGoogleCooldown = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.resetAccount(acc.id);
      setActionMessage({ type: 'success', text: t('providers.msgGoogleResetSuccess', { email: acc.email }) });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgGoogleResetError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleRefreshGoogleQuota = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.refreshAccountQuota(acc.id);
      setActionMessage({
        type: 'success',
        text: t('providers.msgGoogleQuotaUpdated', { email: acc.email }),
      });
      setSelectedQuota((prev) => ({
        title: t('providers.quotaModalTitleGoogle', { email: acc.email }),
        quota: res.quota,
        onRefreshQuota: prev?.onRefreshQuota || (() => handleRefreshGoogleQuota(acc)),
      }));
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgGoogleQuotaError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleRefreshAllGoogleQuota = async () => {
    if (accounts.length === 0) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      try {
        await api.refreshAllAccountsQuota();
      } catch {
        await Promise.all(accounts.map((a) => api.refreshAccountQuota(a.id)));
      }
      setActionMessage({
        type: 'success',
        text: t('providers.msgGoogleAllQuotaSuccess'),
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgGoogleAllQuotaError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleTestGoogleAccount = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.testAccount(acc.id);
      setActionMessage({
        type: 'success',
        text: t('providers.msgGoogleTestSuccess', { email: acc.email, detail: JSON.stringify(res) }),
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: t('providers.msgGoogleTestError', { email: acc.email, error: err?.message || 'Lỗi xác thực' }),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDeleteGoogleAccount = async (acc: AccountResponse) => {
    if (!window.confirm(t('providers.confirmGoogleDelete', { email: acc.email }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteAccount(acc.id);
      setActionMessage({ type: 'success', text: t('providers.msgGoogleDeleteSuccess', { email: acc.email }) });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgGoogleDeleteError') });
    } finally {
      setActionLoading(false);
    }
  };

  // Codex Handlers
  const handleStartCodexOAuth = async () => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.startCodexOAuth();
      const ticket = {
        ticket_id: (res as any).ticket_id || (res as any).state || '',
        state: (res as any).state || '',
        authorize_url: (res as any).authorize_url || (res as any).authorization_url || '',
      };
      setCodexOAuthTicket(ticket);
      setCodexOAuthStatus('pending');
      setShowCodexOAuthModal(true);
      if (ticket.authorize_url) {
        window.open(ticket.authorize_url, 'codex-oauth', 'width=600,height=720');
      }
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgCodexOAuthInitError'),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleExchangeCodexOAuth = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!codexOAuthTicket || !codexOAuthCode.trim()) return;
    const ticketId = codexOAuthTicket.ticket_id || codexOAuthTicket.state || '';
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.exchangeCodexOAuth(ticketId, codexOAuthCode.trim());
      setActionMessage({
        type: 'success',
        text: t('providers.msgCodexOAuthExchangeSuccess'),
      });
      setShowCodexOAuthModal(false);
      setCodexOAuthTicket(null);
      setCodexOAuthCode('');
      loadCodexData();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgCodexOAuthExchangeError'),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleAddCodexAccount = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!codexAuthPath.trim()) {
      setActionMessage({
        type: 'error',
        text: t('providers.msgCodexAuthPathRequired'),
      });
      return;
    }
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.createCodexAccount({
        auth_path: codexAuthPath.trim(),
        email: codexEmail.trim() || undefined,
      });
      setActionMessage({ type: 'success', text: t('providers.msgCodexAddSuccess') });
      setShowCodexAddModal(false);
      setCodexAuthPath('');
      setCodexEmail('');
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgCodexAddError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleToggleCodex = async (acc: CodexAccountRecord) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.toggleCodexAccount(acc.id);
      const isCurrentlyActive = acc.is_active ?? acc.active ?? true;
      setActionMessage({
        type: 'success',
        text: t('providers.msgCodexToggleSuccess', {
          action: isCurrentlyActive ? t('providers.statusOff').toLowerCase() : t('providers.statusOn').toLowerCase(),
          name: acc.email || acc.id,
        }),
      });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.msgCodexToggleError'),
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleResetCodex = async (acc: CodexAccountRecord) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.resetCodexAccount(acc.id);
      setActionMessage({
        type: 'success',
        text: t('providers.msgCodexResetSuccess', { name: acc.email || acc.id }),
      });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgCodexResetError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleRefreshCodexAccountQuota = async (acc: CodexAccountRecord) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      const res = await api.refreshCodexAccountQuota(acc.id);
      setActionMessage({
        type: 'success',
        text: t('providers.msgCodexAccountQuotaUpdated', { name: acc.email || acc.id }),
      });
      setSelectedQuota((prev) => ({
        title: t('providers.quotaModalTitleCodex', { email: acc.email || acc.id }),
        quota: res.quota,
        onRefreshQuota: prev?.onRefreshQuota || (() => handleRefreshCodexAccountQuota(acc)),
      }));
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgCodexAccountQuotaError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleOpenResetCreditModal = async (acc: CodexAccountRecord) => {
    setResetCreditModalAccount(acc);
    setSelectedCreditId('');
    setResetCreditsLoading(true);
    try {
      const res = await api.getCodexResetCredits(acc.id);
      if (res.ok && Array.isArray(res.data?.credits)) {
        const availableCredits = res.data.credits.filter(
          (c: any) => c && (!c.status || c.status === 'available')
        );
        setResetCreditsList(availableCredits);
        if (availableCredits.length > 0) {
          setSelectedCreditId(availableCredits[0].id);
        }
      } else {
        setResetCreditsList([]);
      }
    } catch {
      setResetCreditsList([]);
    } finally {
      setResetCreditsLoading(false);
    }
  };

  const handleConfirmConsumeResetCredit = async () => {
    if (!resetCreditModalAccount) return;
    setConsumingCredit(true);
    try {
      const res = await api.consumeCodexResetCredit(
        resetCreditModalAccount.id,
        selectedCreditId || undefined
      );
      if (res.ok) {
        setActionMessage({
          type: 'success',
          text: t('providers.consumeTicketSuccess'),
        });
        setResetCreditModalAccount(null);
        loadCodexData();
      } else {
        setActionMessage({
          type: 'error',
          text: res.error || t('providers.consumeTicketError'),
        });
      }
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || t('providers.consumeTicketError'),
      });
    } finally {
      setConsumingCredit(false);
    }
  };

  const handleRefreshAllCodexQuota = async () => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.refreshCodexQuota();
      setActionMessage({
        type: 'success',
        text: t('providers.msgCodexAllQuotaSuccess'),
      });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgCodexAllQuotaError') });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDeleteCodex = async (acc: CodexAccountRecord) => {
    if (!window.confirm(t('providers.confirmCodexDelete', { name: acc.email || acc.id }))) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteCodexAccount(acc.id);
      setActionMessage({ type: 'success', text: t('providers.msgCodexDeleteSuccess') });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || t('providers.msgCodexDeleteError') });
    } finally {
      setActionLoading(false);
    }
  };

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

  const formatResetTime = (input?: string | number) => {
    if (input == null || input === '') return null;
    try {
      let d: Date;
      if (typeof input === 'number') {
        d = new Date(input < 1e11 ? input * 1000 : input);
      } else if (!isNaN(Number(input))) {
        const num = Number(input);
        d = new Date(num < 1e11 ? num * 1000 : num);
      } else {
        d = new Date(input);
      }
      if (isNaN(d.getTime())) return null;
      const diffSec = Math.max(0, Math.floor((d.getTime() - Date.now()) / 1000));
      if (diffSec <= 0) return t('providers.resetSoon');
      if (diffSec < 60) return `${diffSec}s`;
      if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m`;
      if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ${Math.floor((diffSec % 3600) / 60)}m`;
      const days = Math.floor(diffSec / 86400);
      const hours = Math.floor((diffSec % 86400) / 3600);
      return `${days}d ${hours}h`;
    } catch {
      return null;
    }
  };

  const getCodexQuotaNumbers = (acc: CodexAccountRecord) => {
    if (!acc.quota || Object.keys(acc.quota).length === 0) {
      return {
        primaryUsed: null,
        primaryRemaining: null,
        primaryResetTime: null,
        weeklyUsed: null,
        weeklyRemaining: null,
        weeklyResetTime: null,
        availableTickets: 0,
        isStopped: false,
      };
    }
    const pWindow = acc.quota.primary_window;
    const pUsed =
      pWindow?.used_percent ??
      (acc.quota.primary_percent !== undefined ? 100 - acc.quota.primary_percent : null);
    const pRem = pUsed !== null ? Math.max(0, Math.min(100, Math.round(100 - pUsed))) : null;
    const pReset =
      pWindow?.reset_time ||
      pWindow?.reset_at ||
      (typeof pWindow?.reset_after_seconds === 'number'
        ? Date.now() / 1000 + pWindow.reset_after_seconds
        : null);

    const wWindow = acc.quota.weekly_window ?? acc.quota.secondary_window;
    const wUsed =
      wWindow?.used_percent ??
      (acc.quota.secondary_percent !== undefined ? 100 - acc.quota.secondary_percent : null);
    const wRem = wUsed !== null ? Math.max(0, Math.min(100, Math.round(100 - wUsed))) : null;
    const wReset =
      wWindow?.reset_time ||
      wWindow?.reset_at ||
      (typeof wWindow?.reset_after_seconds === 'number'
        ? Date.now() / 1000 + wWindow.reset_after_seconds
        : null);

    const resetCredits = acc.quota.rate_limit_reset_credits;
    const availableTickets = resetCredits?.available_count ?? 0;

    const isStopped = pUsed !== null && pUsed >= 98.0;
    return {
      primaryUsed: pUsed,
      primaryRemaining: pRem,
      primaryResetTime: pReset,
      weeklyUsed: wUsed,
      weeklyRemaining: wRem,
      weeklyResetTime: wReset,
      availableTickets,
      isStopped,
    };
  };

  // Google account counts & metrics
  const googleActiveCount = accounts.filter(
    (a) => a.is_active && a.cooldown_remaining <= 0
  ).length;
  const googleCooldownCount = accounts.filter((a) => a.is_active && a.cooldown_remaining > 0).length;

  const claude5hAccs = accounts.filter((a) => a.is_active && typeof a.quota?.claude_5h?.remaining_percent === 'number');
  const claude5hAvg =
    claude5hAccs.length > 0
      ? Math.round(claude5hAccs.reduce((sum, a) => sum + a.quota.claude_5h.remaining_percent, 0) / claude5hAccs.length)
      : null;

  const claudeWeeklyAccs = accounts.filter((a) => a.is_active && typeof a.quota?.claude_weekly?.remaining_percent === 'number');
  const claudeWeeklyAvg =
    claudeWeeklyAccs.length > 0
      ? Math.round(claudeWeeklyAccs.reduce((sum, a) => sum + a.quota.claude_weekly.remaining_percent, 0) / claudeWeeklyAccs.length)
      : null;

  const gemini5hAccs = accounts.filter((a) => a.is_active && typeof a.quota?.gemini_5h?.remaining_percent === 'number');
  const gemini5hAvg =
    gemini5hAccs.length > 0
      ? Math.round(gemini5hAccs.reduce((sum, a) => sum + a.quota.gemini_5h.remaining_percent, 0) / gemini5hAccs.length)
      : null;

  const geminiWeeklyAccs = accounts.filter((a) => a.is_active && typeof a.quota?.gemini_weekly?.remaining_percent === 'number');
  const geminiWeeklyAvg =
    geminiWeeklyAccs.length > 0
      ? Math.round(geminiWeeklyAccs.reduce((sum, a) => sum + a.quota.gemini_weekly.remaining_percent, 0) / geminiWeeklyAccs.length)
      : null;

  const googleClaudeStoppedCount = accounts.filter(
    (a) => a.is_active && typeof a.quota?.claude_5h?.remaining_percent === 'number' && a.quota.claude_5h.remaining_percent <= 0
  ).length;

  const googleGeminiStoppedCount = accounts.filter(
    (a) => a.is_active && typeof a.quota?.gemini_5h?.remaining_percent === 'number' && a.quota.gemini_5h.remaining_percent <= 0
  ).length;

  const googleStopState = (() => {
    if (accounts.length === 0) return { label: t('providers.stopStateNoAccounts'), badge: 'badge-neutral', status: 'neutral' };
    const activeAccounts = accounts.filter((a) => a.is_active);
    if (activeAccounts.length === 0) return { label: t('providers.stopStateDisabled'), badge: 'badge-neutral', status: 'neutral' };
    if (googleActiveCount === 0 && googleCooldownCount > 0) return { label: t('providers.stopStateCooldown'), badge: 'badge-warning', status: 'cooldown' };
    if (claude5hAccs.length > 0 && googleClaudeStoppedCount >= activeAccounts.length) {
      return { label: t('providers.stopStateClaudeStopped'), badge: 'badge-error', status: 'stopped' };
    }
    if (gemini5hAccs.length > 0 && googleGeminiStoppedCount >= activeAccounts.length) {
      return { label: t('providers.stopStateGeminiStopped'), badge: 'badge-error', status: 'stopped' };
    }
    if (googleClaudeStoppedCount > 0) {
      return { label: t('providers.stopStateClaudeDepleted', { count: googleClaudeStoppedCount }), badge: 'badge-warning', status: 'cooldown' };
    }
    if (googleGeminiStoppedCount > 0) {
      return { label: t('providers.stopStateGeminiDepleted', { count: googleGeminiStoppedCount }), badge: 'badge-warning', status: 'cooldown' };
    }
    return { label: t('providers.stopStateReady'), badge: 'badge-success', status: 'active' };
  })();

  // Average google quota for ring
  const googleQuotaAvg =
    claude5hAvg !== null && gemini5hAvg !== null
      ? Math.round((claude5hAvg + gemini5hAvg) / 2)
      : claude5hAvg !== null
      ? claude5hAvg
      : gemini5hAvg !== null
      ? gemini5hAvg
      : accounts.length > 0
      ? 100
      : null;

  // Codex account counts & metrics
  const codexTotalCount = codexAccounts.length > 0 ? codexAccounts.length : (codexStatus?.total_accounts ?? 0);
  const codexActiveCount = codexAccounts.length > 0
    ? codexAccounts.filter((a) => (a.is_active ?? a.active ?? true) && (a.cooldown_remaining ?? 0) <= 0).length
    : (codexStatus?.active_accounts ?? 0);
  const codexCooldownCount = codexAccounts.length > 0
    ? codexAccounts.filter((a) => (a.cooldown_remaining ?? 0) > 0).length
    : (codexStatus?.cooldown_accounts ?? 0);

  const codexParsedList = codexAccounts.map(getCodexQuotaNumbers);
  const codexPrimaryAccs = codexParsedList.filter((q) => q.primaryRemaining !== null);
  const codexPrimaryAvg =
    codexPrimaryAccs.length > 0
      ? Math.round(codexPrimaryAccs.reduce((sum, q) => sum + (q.primaryRemaining ?? 0), 0) / codexPrimaryAccs.length)
      : null;

  const codexWeeklyAccs = codexParsedList.filter((q) => q.weeklyRemaining !== null);
  const codexWeeklyAvg =
    codexWeeklyAccs.length > 0
      ? Math.round(codexWeeklyAccs.reduce((sum, q) => sum + (q.weeklyRemaining ?? 0), 0) / codexWeeklyAccs.length)
      : null;

  const codexStoppedCount = codexAccounts.filter(
    (a) => (a.is_active ?? a.active ?? true) && getCodexQuotaNumbers(a).isStopped
  ).length;

  const codexStopState = (() => {
    if (codexTotalCount === 0) return { label: t('providers.stopStateNoAccounts'), badge: 'badge-neutral', status: 'neutral' };
    const activeAccounts = codexAccounts.filter((a) => (a.is_active ?? a.active ?? true));
    if (activeAccounts.length === 0 && codexAccounts.length > 0) return { label: t('providers.stopStateDisabled'), badge: 'badge-neutral', status: 'neutral' };
    if (codexActiveCount === 0 && codexCooldownCount > 0) return { label: t('providers.stopStateCooldown'), badge: 'badge-warning', status: 'cooldown' };
    if (codexPrimaryAccs.length > 0 && codexStoppedCount >= (activeAccounts.length || 1)) {
      return { label: t('providers.stopStateCodexStopped'), badge: 'badge-error', status: 'stopped' };
    }
    if (codexStoppedCount > 0) {
      return { label: t('providers.stopStateCodexDepleted', { count: codexStoppedCount }), badge: 'badge-warning', status: 'cooldown' };
    }
    return { label: t('providers.stopStateReady'), badge: 'badge-success', status: 'active' };
  })();

  const totalActivePools = googleActiveCount + codexActiveCount;
  const totalCooling = googleCooldownCount + codexCooldownCount;

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
              className="health-ring-meter"
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

  // Neutral Activity Indicator Pulse Helper (replaces misleading synthetic sparkline offsets)
  const renderActivityPulse = (strokeColor: string, label: string = 'Nhịp hoạt động') => {
    const width = 240;
    const height = 24;
    // Neutral rhythmic heartbeat wave (no synthetic historical jitter)
    const points = '0,12 45,12 65,12 78,7 88,17 98,4 108,20 118,12 138,12 152,9 162,15 172,12 240,12';
    const fillPoints = `0,${height} 0,12 45,12 65,12 78,7 88,17 98,4 108,20 118,12 138,12 152,9 162,15 172,12 240,12 240,${height}`;
    const gradId = `pulseGrad-${strokeColor.replace('#', '')}`;

    return (
      <div
        className="sparkline-container"
        title={t('providers.pulseTitle', { label })}
        aria-label={`${label} ${t('providers.pulseIndicator')}`}
      >
        <div className="activity-indicator-badge">
          <span className="activity-pulse-dot" style={{ backgroundColor: strokeColor }} />
          <span>{label}</span>
          <span className="activity-indicator-sub">{t('providers.pulseIndicator')}</span>
        </div>
        <svg
          viewBox={`0 0 ${width} ${height}`}
          className="sparkline-svg activity-pulse-svg"
          preserveAspectRatio="none"
          role="img"
          aria-hidden="true"
        >
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={strokeColor} stopOpacity="0.25" />
              <stop offset="100%" stopColor={strokeColor} stopOpacity="0.0" />
            </linearGradient>
          </defs>
          <polygon points={fillPoints} fill={`url(#${gradId})`} />
          <polyline
            points={points}
            fill="none"
            stroke={strokeColor}
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="activity-pulse-line"
          />
        </svg>
      </div>
    );
  };

  return (
    <div className="providers-view-container">
      {/* 1. COMPACT HEADER */}
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
              loadCodexData();
            }}
            disabled={actionLoading || parentLoading || codexLoading}
            title={t('providers.refreshTitle')}
          >
            <IconRefresh size={15} />
            <span>{t('providers.refreshBtn')}</span>
          </button>
        </div>
      </div>

      {/* 2. UNIFIED HORIZONTAL STATUS STRIP (3-SECOND SCAN) */}
      <div className="status-legend-bar">
        <div className="route-pulse-wrapper">
          <span
            className={`route-pulse-dot ${
              totalActivePools === 0 ? (totalCooling > 0 ? 'cooldown' : 'error') : ''
            }`}
          />
          <span>
            {totalActivePools > 0
              ? t('providers.routingReady')
              : totalCooling > 0
              ? t('providers.waitingCooldown')
              : t('providers.noActiveAccounts')}
          </span>
          <span style={{ color: 'var(--text-muted)', fontWeight: 400, fontSize: 11.5, marginLeft: 4 }}>
            {t('providers.readyCount', {
              active: googleActiveCount + codexActiveCount,
              total: accounts.length + codexTotalCount,
            })}
          </span>
        </div>

        <div className="status-strip-chips">
          <button
            type="button"
            className="status-segment-chip"
            onClick={() => document.getElementById('provider-google')?.scrollIntoView({ behavior: 'smooth' })}
            title={t('providers.scrollToGoogle')}
          >
            <IconGoogle size={14} />
            <span>
              Google: <strong>{googleActiveCount}/{accounts.length}</strong>
              {googleQuotaAvg !== null && ` • ${googleQuotaAvg}%`}
            </span>
            <span className={`status-dot-mini ${googleStopState.status}`} />
          </button>

          <button
            type="button"
            className="status-segment-chip"
            onClick={() => document.getElementById('provider-codex')?.scrollIntoView({ behavior: 'smooth' })}
            title={t('providers.scrollToCodex')}
          >
            <IconOpenAI size={14} />
            <span>
              Codex: <strong>{codexActiveCount}/{codexTotalCount}</strong>
              {codexPrimaryAvg !== null && ` • ${codexPrimaryAvg}%`}
            </span>
            <span className={`status-dot-mini ${codexStopState.status}`} />
          </button>

          <button
            type="button"
            className="status-segment-chip"
            onClick={() => document.getElementById('provider-openrouter')?.scrollIntoView({ behavior: 'smooth' })}
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
            onClick={() => document.getElementById('provider-upstream')?.scrollIntoView({ behavior: 'smooth' })}
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
            <span className="status-dot-mini cooldown" />
            <span>{t('providers.legendCooldown')}</span>
          </span>
          <span className="status-legend-item">
            <span className="status-dot-mini stopped" />
            <span>{t('providers.legendStopped')}</span>
          </span>
          <span className="status-legend-item">
            <span className="status-dot-mini neutral" />
            <span>{t('providers.legendDisabled')}</span>
          </span>
        </div>
      </div>

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

      {/* Auto Quota Refresh Panel - Gọn Gàng Hơn */}
      <div className="auto-quota-compact-strip">
        <div className="auto-quota-strip-left">
          <div className="auto-quota-icon-badge">
            <IconRefresh size={15} className={quotaRefresh?.is_refreshing ? 'spinner' : ''} />
          </div>
          <div className="auto-quota-strip-title-wrap">
            <span className="auto-quota-strip-title">{t('providers.autoRefreshTitle')}</span>
            <span className="auto-quota-strip-meta">
              {quotaRefresh?.is_refreshing ? (
                <span className="highlight-refreshing">{t('providers.autoRefreshRefreshing')}</span>
              ) : quotaRefresh?.enabled ? (
                <>
                  <span>{t('providers.autoRefreshLast', { time: formatTimestamp(quotaRefresh?.last_refresh ?? null) })}</span>
                  <span style={{ margin: '0 4px', color: 'var(--text-muted)' }}>•</span>
                  <span>{t('providers.autoRefreshNext', { time: formatCountdown(quotaRefresh?.next_refresh ?? null) })}</span>
                </>
              ) : (
                <span style={{ color: 'var(--text-muted)' }}>{t('providers.autoRefreshDisabled')}</span>
              )}
            </span>
          </div>
        </div>

        <div className="auto-quota-strip-right">
          <div className="auto-quota-interval-group" role="radiogroup" aria-label={t('providers.toggleAutoRefreshAria')}>
            {[
              { label: '1m', val: 60, title: t('providers.interval1m') },
              { label: '5m', val: 300, title: t('providers.interval5m') },
              { label: '15m', val: 900, title: t('providers.interval15m') },
              { label: '30m', val: 1800, title: t('providers.interval30m') },
            ].map(({ label, val, title }) => (
              <button
                key={val}
                type="button"
                title={title}
                className={`interval-pill-btn ${quotaRefresh?.interval_secs === val ? 'active' : ''}`}
                onClick={() => handleChangeRefreshInterval(val)}
                disabled={quotaRefreshLoading}
              >
                {label}
              </button>
            ))}
          </div>

          <button
            type="button"
            className={`auto-quota-toggle-btn ${quotaRefresh?.enabled ? 'enabled' : 'disabled'}`}
            onClick={handleToggleAutoRefresh}
            disabled={quotaRefreshLoading}
            aria-pressed={quotaRefresh?.enabled}
            title={quotaRefresh?.enabled ? t('providers.turnOffAutoRefresh') : t('providers.turnOnAutoRefresh')}
          >
            <span className="toggle-switch-track">
              <span className="toggle-switch-thumb" />
            </span>
            <span className="toggle-switch-text">{quotaRefresh?.enabled ? t('providers.toggleOn') : t('providers.toggleOff')}</span>
          </button>

          <button
            type="button"
            className="action-icon-btn primary manual-refresh-btn"
            onClick={handleManualRunRefresh}
            disabled={quotaRefreshLoading || quotaRefresh?.is_refreshing}
            title={t('providers.manualRefreshTitle')}
          >
            <IconRefresh size={13} className={quotaRefreshLoading || quotaRefresh?.is_refreshing ? 'spinner' : ''} />
            <span>{t('providers.manualRefreshBtn')}</span>
          </button>
        </div>

        {quotaRefresh?.last_error && (
          <div className="auto-quota-error-notice" style={{ width: '100%', marginTop: 4 }}>
            <IconAlertCircle size={14} />
            <span>{t('providers.lastRefreshNotice', { error: quotaRefresh.last_error })}</span>
          </div>
        )}
      </div>

      {/* Loading Skeleton */}
      {parentLoading && accounts.length === 0 && (
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
        {/* CARD 1: GOOGLE ANTIGRAVITY */}
        <div id="provider-google" className="visual-card">
            {/* Header */}
            <div className="visual-card-header">
              <div className="visual-brand-group">
                <div className="visual-logo-badge google">
                  <IconGoogle size={22} />
                </div>
                <div className="visual-brand-meta">
                  <div className="visual-card-title">
                    <span>Google Antigravity</span>
                  </div>
                  <div className="visual-chips-row">
                    <span className="model-family-chip">ag/*</span>
                    <span className="model-sub-chip">Gemini 2.5</span>
                    <span className="model-sub-chip">Claude 3.7</span>
                  </div>
                </div>
              </div>
              <span className={`badge ${googleStopState.badge}`}>{googleStopState.label}</span>
            </div>

            {/* Health Ring & Big Numbers */}
            <div className="visual-metrics-row">
              {renderHealthRing(googleQuotaAvg, t('providers.quotaAvgLabel'), t('providers.quotaAvailable'))}
              <div className="visual-stat-numbers">
                <div className="stat-num-box">
                  <span className="stat-num-val active">{googleActiveCount}</span>
                  <span className="stat-num-lbl">{t('providers.statActive')}</span>
                </div>
                <div className="stat-num-box">
                  <span className={`stat-num-val ${googleCooldownCount > 0 ? 'cooldown' : ''}`}>
                    {googleCooldownCount}
                  </span>
                  <span className="stat-num-lbl">{t('providers.statCooldown')}</span>
                </div>
                <div className="stat-num-box">
                  <span className="stat-num-val total">{accounts.length}</span>
                  <span className="stat-num-lbl">{t('providers.statTotal')}</span>
                </div>
              </div>
            </div>

            {/* Micro Quota Bars & Sparkline */}
            <div className="visual-quota-bars">
              <div className="micro-bar-row">
                <span className="micro-bar-label">{/* Gemini 5h: */}{t('providers.quotaGemini5h')}:</span>
                <div className="micro-bar-track">
                  <div
                    className="micro-bar-fill"
                    style={{
                      width: `${gemini5hAvg ?? 0}%`,
                      background:
                        (gemini5hAvg ?? 0) < 20
                          ? '#ef4444'
                          : (gemini5hAvg ?? 0) < 50
                          ? '#f59e0b'
                          : '#10b981',
                    }}
                  />
                </div>
                <span className="micro-bar-val">{gemini5hAvg !== null ? `${gemini5hAvg}%` : '-'}</span>
              </div>

              {geminiWeeklyAvg !== null && (
                <div className="micro-bar-row">
                  <span className="micro-bar-label">{/* Gemini Tuần: */}{t('providers.quotaGeminiWeekly')}:</span>
                  <div className="micro-bar-track">
                    <div
                      className="micro-bar-fill"
                      style={{
                        width: `${geminiWeeklyAvg}%`,
                        background:
                          geminiWeeklyAvg < 20
                            ? '#ef4444'
                            : geminiWeeklyAvg < 50
                            ? '#f59e0b'
                            : '#10b981',
                      }}
                    />
                  </div>
                  <span className="micro-bar-val">{`${geminiWeeklyAvg}%`}</span>
                </div>
              )}

              <div className="micro-bar-row">
                <span className="micro-bar-label">{t('providers.quotaClaude5h')}:</span>
                <div className="micro-bar-track">
                  <div
                    className="micro-bar-fill"
                    style={{
                      width: `${claude5hAvg ?? 0}%`,
                      background:
                        (claude5hAvg ?? 0) < 20
                          ? '#ef4444'
                          : (claude5hAvg ?? 0) < 50
                          ? '#f59e0b'
                          : '#10b981',
                    }}
                  />
                </div>
                <span className="micro-bar-val">{claude5hAvg !== null ? `${claude5hAvg}%` : '-'}</span>
              </div>

              {claudeWeeklyAvg !== null && (
                <div className="micro-bar-row">
                  <span className="micro-bar-label">{t('providers.quotaClaudeWeekly')}:</span>
                  <div className="micro-bar-track">
                    <div
                      className="micro-bar-fill"
                      style={{
                        width: `${claudeWeeklyAvg}%`,
                        background:
                          claudeWeeklyAvg < 20
                            ? '#ef4444'
                            : claudeWeeklyAvg < 50
                            ? '#f59e0b'
                            : '#10b981',
                      }}
                    />
                  </div>
                  <span className="micro-bar-val">{`${claudeWeeklyAvg}%`}</span>
                </div>
              )}

              {renderActivityPulse('#4285F4', t('providers.pulseGoogle'))}
            </div>

            {/* Consolidated Action Bar */}
            <div className="segment-action-bar">
              <div className="segment-actions-group">
                <button
                  className="action-icon-btn"
                  onClick={handleRefreshAllGoogleQuota}
                  disabled={actionLoading || accounts.length === 0}
                  title={t('providers.refreshGooglePoolQuota')}
                  aria-label={t('providers.refreshGooglePoolQuota')}
                >
                  <IconRefresh size={14} />
                  <span>{t('providers.quotaActionBtn')}</span>
                </button>
                <button
                  className="action-icon-btn"
                  onClick={handleStartGoogleOAuth}
                  disabled={actionLoading}
                  title={t('providers.loginGoogleOAuth')}
                  aria-label={t('providers.loginGoogleOAuth')}
                >
                  <IconShield size={14} />
                  <span>{t('providers.oauthGoogleBtn')}</span>
                </button>
                <button
                  className="action-icon-btn primary"
                  onClick={() => setShowGoogleAddModal(true)}
                  disabled={actionLoading}
                  title={t('providers.addGoogleManual')}
                  aria-label={t('providers.addGoogleManual')}
                >
                  <IconPlus size={14} />
                  <span>{t('providers.addTokenBtn')}</span>
                </button>
              </div>

              <button
                className="details-toggle-btn"
                style={{ width: 'auto' }}
                onClick={() => setShowGoogleDetails(!showGoogleDetails)}
                aria-expanded={showGoogleDetails}
                aria-controls="google-details-drawer"
              >
                <span>
                  {showGoogleDetails
                    ? t('providers.collapseAccounts')
                    : t('providers.accountDetailsGoogle', { count: accounts.length })}
                </span>
                <IconChevronDown
                  size={16}
                  className={`details-toggle-chevron ${showGoogleDetails ? 'open' : ''}`}
                />
              </button>
            </div>

            {/* Expandable Account Details Panel with Accessible CSS Transition */}
            <div
              id="google-details-drawer"
              className={`details-drawer-wrapper ${showGoogleDetails ? 'expanded' : 'collapsed'}`}
              aria-hidden={!showGoogleDetails}
            >
              <div className="details-drawer-content">
                {accounts.length === 0 ? (
                  <div className="empty-graphic-box">
                    <p style={{ margin: 0, fontSize: 13 }}>{t('providers.emptyGoogleAccounts')}</p>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={handleStartGoogleOAuth}
                    >
                      {t('providers.connectGoogleOAuth')}
                    </button>
                  </div>
                ) : (
                  <div className="table-container">
                    <table>
                      <thead>
                        <tr>
                          <th>{t('providers.thAccount')}</th>
                          <th>{t('providers.thStatus')}</th>
                          <th>{t('providers.thCooldown')}</th>
                          <th>{t('providers.thQuotaRemaining')}</th>
                          <th>{t('providers.thActions')}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {accounts.map((acc) => {
                          const isCooling = acc.cooldown_remaining > 0;
                          return (
                            <tr key={acc.id}>
                              <td>
                                <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                                  {acc.email}
                                </div>
                                <span className="badge badge-neutral font-mono" style={{ fontSize: 10, marginTop: 2 }}>
                                  id: {acc.id.slice(0, 8)}...
                                </span>
                              </td>
                              <td>
                                <span className={`badge ${acc.is_active ? 'badge-success' : 'badge-neutral'}`}>
                                  {acc.is_active ? t('providers.statusOn') : t('providers.statusOff')}
                                </span>
                              </td>
                              <td>
                                {isCooling ? (
                                  <span className="badge badge-warning font-mono">
                                    {Math.ceil(acc.cooldown_remaining)}s
                                  </span>
                                ) : (
                                  <span className="badge badge-success">{t('providers.statusReady')}</span>
                                )}
                              </td>
                              <td>
                                {acc.quota && (acc.quota.gemini_5h || acc.quota.claude_5h || acc.quota.gemini_weekly || acc.quota.claude_weekly) ? (
                                  <div style={{ fontSize: 11, display: 'flex', flexDirection: 'column', gap: 2 }}>
                                    {acc.quota.gemini_5h && (
                                      <span>
                                        {t('providers.quotaGemini5h')}: <strong>{acc.quota.gemini_5h.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.gemini_5h.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={t('providers.resetAt', { time: acc.quota.gemini_5h.reset_time })}>
                                            ({formatResetTime(acc.quota.gemini_5h.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    {acc.quota.gemini_weekly && (
                                      <span>
                                        {t('providers.quotaGeminiWeekly')}: <strong>{acc.quota.gemini_weekly.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.gemini_weekly.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={t('providers.resetAt', { time: acc.quota.gemini_weekly.reset_time })}>
                                            ({formatResetTime(acc.quota.gemini_weekly.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    {acc.quota.claude_5h && (
                                      <span>
                                        {t('providers.quotaClaude5h')}: <strong>{acc.quota.claude_5h.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.claude_5h.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={t('providers.resetAt', { time: acc.quota.claude_5h.reset_time })}>
                                            ({formatResetTime(acc.quota.claude_5h.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    {acc.quota.claude_weekly && (
                                      <span>
                                        {t('providers.quotaClaudeWeekly')}: <strong>{acc.quota.claude_weekly.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.claude_weekly.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={t('providers.resetAt', { time: acc.quota.claude_weekly.reset_time })}>
                                            ({formatResetTime(acc.quota.claude_weekly.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    <button
                                      className="btn btn-secondary btn-sm font-mono"
                                      style={{ padding: '1px 6px', fontSize: 10, alignSelf: 'flex-start', marginTop: 2 }}
                                      onClick={() =>
                                        setSelectedQuota({
                                          title: `Quota Google (${acc.email})`,
                                          quota: acc.quota,
                                          onRefreshQuota: () => handleRefreshGoogleQuota(acc),
                                        })
                                      }
                                    >
                                      JSON
                                    </button>
                                  </div>
                                ) : (
                                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{t('providers.quotaNotLoaded')}</span>
                                )}
                              </td>
                              <td>
                                <div className="table-actions">
                                  <button
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => handleTestGoogleAccount(acc)}
                                    disabled={actionLoading}
                                    title={t('providers.testToken')}
                                  >
                                    {t('providers.btnTest')}
                                  </button>
                                  {isCooling && (
                                    <button
                                      className="btn btn-secondary btn-sm"
                                      onClick={() => handleResetGoogleCooldown(acc)}
                                      disabled={actionLoading}
                                      title={t('providers.btnResetCooldown')}
                                    >
                                      {t('providers.btnResetCooldown')}
                                    </button>
                                  )}
                                  <button
                                    className="btn btn-danger btn-sm btn-icon-only"
                                    onClick={() => handleDeleteGoogleAccount(acc)}
                                    disabled={actionLoading}
                                    title={t('providers.btnDeleteAccount')}
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

        {/* CARD 2: OPENAI CODEX */}
        <div id="provider-codex" className="visual-card">
            {/* Header */}
            <div className="visual-card-header">
              <div className="visual-brand-group">
                <div className="visual-logo-badge codex">
                  <IconOpenAI size={22} />
                </div>
                <div className="visual-brand-meta">
                  <div className="visual-card-title">
                    <span>OpenAI Codex</span>
                  </div>
                  <div className="visual-chips-row">
                    <span className="model-family-chip">cx/*</span>
                    <span className="model-sub-chip">o3-mini</span>
                    <span className="model-sub-chip">gpt-4o</span>
                  </div>
                </div>
              </div>
              <span className={`badge ${codexStopState.badge}`}>{codexStopState.label}</span>
            </div>

            {/* Health Ring & Big Numbers */}
            <div className="visual-metrics-row">
              {renderHealthRing(codexPrimaryAvg, t('providers.primaryAvgLabel'), t('providers.codexPrimarySub'))}
              <div className="visual-stat-numbers">
                <div className="stat-num-box">
                  <span className="stat-num-val active">{codexActiveCount}</span>
                  <span className="stat-num-lbl">{t('providers.statActive')}</span>
                </div>
                <div className="stat-num-box">
                  <span className={`stat-num-val ${codexCooldownCount > 0 ? 'cooldown' : ''}`}>
                    {codexCooldownCount}
                  </span>
                  <span className="stat-num-lbl">{t('providers.statCooldown')}</span>
                </div>
                <div className="stat-num-box">
                  <span className="stat-num-val total">{codexTotalCount}</span>
                  <span className="stat-num-lbl">{t('providers.statTotal')}</span>
                </div>
              </div>
            </div>

            {/* Micro Quota Bars & Sparkline */}
            <div className="visual-quota-bars">
              <div className="micro-bar-row">
                <span className="micro-bar-label">{t('providers.quotaCodexPrimary')}:</span>
                <div className="micro-bar-track">
                  <div
                    className="micro-bar-fill"
                    style={{
                      width: `${codexPrimaryAvg ?? 0}%`,
                      background:
                        (codexPrimaryAvg ?? 0) <= 2
                          ? '#ef4444'
                          : (codexPrimaryAvg ?? 0) < 50
                          ? '#f59e0b'
                          : '#10a37f',
                    }}
                  />
                </div>
                <span className="micro-bar-val">{codexPrimaryAvg !== null ? `${codexPrimaryAvg}%` : '-'}</span>
              </div>

              <div className="micro-bar-row">
                <span className="micro-bar-label">{t('providers.quotaCodexWeekly')}:</span>
                <div className="micro-bar-track">
                  <div
                    className="micro-bar-fill"
                    style={{
                      width: `${codexWeeklyAvg ?? 0}%`,
                      background:
                        (codexWeeklyAvg ?? 0) <= 2
                          ? '#ef4444'
                          : (codexWeeklyAvg ?? 0) < 50
                          ? '#f59e0b'
                          : '#10a37f',
                    }}
                  />
                </div>
                <span className="micro-bar-val">{codexWeeklyAvg !== null ? `${codexWeeklyAvg}%` : '-'}</span>
              </div>

              {renderActivityPulse('#10a37f', t('providers.pulseCodex'))}
            </div>

            {/* Consolidated Action Bar */}
            <div className="segment-action-bar">
              <div className="segment-actions-group">
                <button
                  className="action-icon-btn"
                  onClick={handleRefreshAllCodexQuota}
                  disabled={actionLoading}
                  title={t('providers.refreshCodexPoolQuota')}
                  aria-label={t('providers.refreshCodexPoolQuota')}
                >
                  <IconRefresh size={14} />
                  <span>{t('providers.quotaActionBtn')}</span>
                </button>
                <button
                  className="action-icon-btn"
                  onClick={handleStartCodexOAuth}
                  disabled={actionLoading}
                  title={t('providers.initCodexOAuth')}
                  aria-label={t('providers.initCodexOAuth')}
                >
                  <IconShield size={14} />
                  <span>{t('providers.oauthPkceBtn')}</span>
                </button>
                <button
                  className="action-icon-btn primary"
                  onClick={() => setShowCodexAddModal(true)}
                  disabled={actionLoading}
                  title={t('providers.addAuthJson')}
                  aria-label={t('providers.addAuthJson')}
                >
                  <IconPlus size={14} />
                  <span>{t('providers.addAuthBtn')}</span>
                </button>
              </div>

              <button
                className="details-toggle-btn"
                style={{ width: 'auto' }}
                onClick={() => setShowCodexDetails(!showCodexDetails)}
                aria-expanded={showCodexDetails}
                aria-controls="codex-details-drawer"
              >
                <span>
                  {showCodexDetails
                    ? t('providers.collapseAccounts')
                    : t('providers.accountDetailsCodex', { count: codexTotalCount })}
                </span>
                <IconChevronDown
                  size={16}
                  className={`details-toggle-chevron ${showCodexDetails ? 'open' : ''}`}
                />
              </button>
            </div>

            {/* Expandable Codex Details Panel with Accessible CSS Transition */}
            <div
              id="codex-details-drawer"
              className={`details-drawer-wrapper ${showCodexDetails ? 'expanded' : 'collapsed'}`}
              aria-hidden={!showCodexDetails}
            >
              <div className="details-drawer-content">
                {codexAccounts.length === 0 ? (
                  <div className="empty-graphic-box">
                    <p style={{ margin: 0, fontSize: 13 }}>{t('providers.emptyCodexAccounts')}</p>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={handleStartCodexOAuth}
                    >
                      {t('providers.startCodexOAuthBtn')}
                    </button>
                  </div>
                ) : (
                  <div className="table-container">
                    <table>
                      <thead>
                        <tr>
                          <th>{t('providers.thAccount')}</th>
                          <th>{t('providers.thStatus')}</th>
                          <th>{t('providers.thCooldown')}</th>
                          <th>{t('providers.thLimit')}</th>
                          <th>{t('providers.thActions')}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {codexAccounts.map((acc) => {
                          const isActiveAcc = acc.is_active ?? acc.active ?? true;
                          const isCooling = (acc.cooldown_remaining ?? 0) > 0;
                          const q = getCodexQuotaNumbers(acc);
                          return (
                            <tr key={acc.id}>
                              <td>
                                <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                                  {acc.email || t('providers.accountUndefined')}
                                </div>
                                <span className="badge badge-neutral font-mono" style={{ fontSize: 10, marginTop: 2 }}>
                                  id: {acc.id.slice(0, 8)}...
                                </span>
                              </td>
                              <td>
                                <span className={`badge ${isActiveAcc ? 'badge-success' : 'badge-neutral'}`}>
                                  {isActiveAcc ? t('providers.statusOn') : t('providers.statusOff')}
                                </span>
                              </td>
                              <td>
                                {isCooling ? (
                                  <span className="badge badge-warning font-mono">
                                    {Math.ceil(acc.cooldown_remaining || 0)}s
                                  </span>
                                ) : (
                                  <span className="badge badge-success">{t('providers.statusReady')}</span>
                                )}
                              </td>
                              <td>
                                {q.primaryRemaining !== null ? (
                                  <div style={{ fontSize: 11, display: 'flex', flexDirection: 'column', gap: 3 }}>
                                    <div>
                                      <span>Primary (5h): <strong>{q.primaryRemaining}%</strong></span>
                                      {formatResetTime(q.primaryResetTime) && (
                                        <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={t('providers.resetAt', { time: q.primaryResetTime })}>
                                          ({formatResetTime(q.primaryResetTime)})
                                        </span>
                                      )}
                                    </div>
                                    {q.weeklyRemaining !== null && (
                                      <div>
                                        <span>Weekly: <strong>{q.weeklyRemaining}%</strong></span>
                                        {formatResetTime(q.weeklyResetTime) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={t('providers.resetAt', { time: q.weeklyResetTime })}>
                                            ({formatResetTime(q.weeklyResetTime)})
                                          </span>
                                        )}
                                      </div>
                                    )}
                                    <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 2 }}>
                                      <span
                                        className={`badge ${q.availableTickets > 0 ? 'badge-info' : 'badge-neutral'}`}
                                        style={{ fontSize: 10, padding: '1px 6px' }}
                                        title={t('providers.codexResetTicketsTooltip')}
                                      >
                                        🎟️ {t('providers.codexResetTickets', { count: q.availableTickets })}
                                      </span>
                                      {q.availableTickets > 0 && (
                                        <button
                                          className="btn btn-primary btn-sm font-mono"
                                          style={{ padding: '1px 6px', fontSize: 10 }}
                                          onClick={() => handleOpenResetCreditModal(acc)}
                                          title={t('providers.useCodexResetTicket')}
                                        >
                                          {t('providers.useTicket')}
                                        </button>
                                      )}
                                      <button
                                        className="btn btn-secondary btn-sm font-mono"
                                        style={{ padding: '1px 6px', fontSize: 10 }}
                                        onClick={() =>
                                          setSelectedQuota({
                                            title: `Quota Codex (${acc.email || acc.id})`,
                                            quota: acc.quota,
                                            onRefreshQuota: () => handleRefreshCodexAccountQuota(acc),
                                          })
                                        }
                                      >
                                        JSON
                                      </button>
                                    </div>
                                  </div>
                                ) : (
                                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{t('providers.quotaNotLoaded')}</span>
                                )}
                              </td>
                              <td>
                                <div className="table-actions">
                                  <button
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => handleToggleCodex(acc)}
                                    disabled={actionLoading}
                                  >
                                    {isActiveAcc ? t('providers.statusOff') : t('providers.statusOn')}
                                  </button>
                                  {isCooling && (
                                    <button
                                      className="btn btn-secondary btn-sm"
                                      onClick={() => handleResetCodex(acc)}
                                      disabled={actionLoading}
                                      title={t('providers.btnResetCooldown')}
                                    >
                                      {t('providers.btnResetCooldown')}
                                    </button>
                                  )}
                                  <button
                                    className="btn btn-danger btn-sm btn-icon-only"
                                    onClick={() => handleDeleteCodex(acc)}
                                    disabled={actionLoading}
                                    title={t('providers.btnDeleteAccount')}
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

        {/* CARD 3: OPENROUTER FIRST-CLASS PROVIDER */}
        <div id="provider-openrouter" className="visual-card">
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
              <div style={{ fontSize: 24, fontWeight: 700, color: (openrouterCredits && typeof openrouterCredits.total_credits === 'number' && (openrouterCredits.total_credits - (openrouterCredits.total_usage || 0)) <= 0.05) ? '#f59e0b' : '#10b981' }}>
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
                  <span className="badge badge-warning" style={{ fontSize: 10, padding: '1px 6px', background: 'rgba(245, 158, 11, 0.15)', color: '#f59e0b', borderColor: 'rgba(245, 158, 11, 0.3)' }}>
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
              style={{ width: 'auto' }}
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
                      <span className="badge badge-neutral" style={{ fontSize: 10, padding: '1px 5px', color: '#818cf8', borderColor: 'rgba(99, 102, 241, 0.3)' }}>
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
        <div id="provider-upstream" className="visual-card">
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
                style={{ width: 'auto' }}
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
                              <td>
                                <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>{p.name}</div>
                                <span className="badge badge-neutral font-mono" style={{ fontSize: 10, marginTop: 2 }}>
                                  {p.prefix}/*
                                </span>
                              </td>
                              <td>
                                <span className="badge badge-neutral">{p.type}</span>
                              </td>
                              <td className="font-mono text-break" style={{ fontSize: 12, maxWidth: 180 }}>
                                {p.base_url}
                              </td>
                              <td>
                                <span className="font-mono" style={{ fontSize: 11, color: 'var(--text-muted)' }}>
                                  {p.api_key ? '••••••••' : t('providers.emptyValue')}
                                </span>
                              </td>
                              <td>
                                <span className="badge badge-neutral" style={{ fontSize: 10 }}>
                                  {modelsArr.length > 0 ? t('providers.modelsCount', { count: modelsArr.length }) : t('providers.modelsDefault')}
                                </span>
                              </td>
                              <td>
                                <span className={`badge ${p.is_active ? 'badge-success' : 'badge-neutral'}`}>
                                  {p.is_active ? t('providers.statusOn') : t('providers.statusOff')}
                                </span>
                              </td>
                              <td>
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

      {/* MODAL: GOOGLE ADD ACCOUNT */}
      {showGoogleAddModal && (
        <div className="modal-backdrop" onClick={() => setShowGoogleAddModal(false)}>
          <div
            className="modal-card"
            style={{ maxWidth: 500 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title">{t('providers.modalGoogleAddTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowGoogleAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleAddGoogleAccount}>
              <div className="form-group">
                <label>{t('providers.labelGoogleEmail')}</label>
                <input
                  type="text"
                  placeholder="user@example.com"
                  value={googleEmail}
                  onChange={(e) => setGoogleEmail(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label>{t('providers.labelGoogleRefreshToken')}</label>
                <textarea
                  rows={4}
                  placeholder="1//04..."
                  value={googleRefreshToken}
                  onChange={(e) => setGoogleRefreshToken(e.target.value)}
                  required
                  style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  {t('providers.googleTokenHint')}
                </span>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowGoogleAddModal(false)}
                >
                  {t('providers.btnCancel')}
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : t('providers.btnAddToPool')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* MODAL: GOOGLE OAUTH POPUP / FALLBACK */}
      {showGoogleOAuthModal && (
        <div className="modal-backdrop" onClick={() => setShowGoogleOAuthModal(false)}>
          <div
            className="modal-card"
            style={{ maxWidth: 540 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title">{t('providers.modalGoogleOAuthTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowGoogleOAuthModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <div style={{ marginBottom: 16 }}>
              {googleOAuthTicket?.state && (
                <div style={{ marginBottom: 12 }}>
                  <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>{t('providers.sessionStateLabel')} </span>
                  <span className="badge badge-warning font-mono" style={{ fontSize: 11 }}>
                    {googleOAuthTicket.state.length > 28
                      ? googleOAuthTicket.state.slice(0, 28) + '...'
                      : googleOAuthTicket.state}
                  </span>
                </div>
              )}

              <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 8 }}>
                {t('providers.googleOAuthHelp')}
              </p>

              {(googleOAuthTicket?.authorize_url || googleOAuthUrl) && (
                <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
                  <a
                    href={googleOAuthTicket?.authorize_url || googleOAuthUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="btn btn-secondary btn-sm"
                    style={{ flex: 1, justifyContent: 'center' }}
                  >
                    {t('providers.openGoogleAuthPage')}
                  </a>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => {
                      const url = googleOAuthTicket?.authorize_url || googleOAuthUrl;
                      if (url) {
                        navigator.clipboard.writeText(url);
                        setActionMessage({
                          type: 'success',
                          text: t('providers.msgCopiedAuthLink'),
                        });
                      }
                    }}
                  >
                    {t('providers.copyLink')}
                  </button>
                </div>
              )}

              <div style={{ fontSize: 11.5, color: 'var(--text-muted)', marginBottom: 14 }}>
                💡 <b>{t('providers.tipNewAccount')}</b> {t('providers.tipNewAccountDesc')}
              </div>

              <form onSubmit={handleExchangeGoogleOAuth}>
                <div className="form-group">
                  <label>{t('providers.labelGoogleCode')}</label>
                  <textarea
                    rows={3}
                    placeholder={t('providers.placeholderGoogleCode')}
                    value={googleOAuthCode}
                    onChange={(e) => setGoogleOAuthCode(e.target.value)}
                    required
                    style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                  />
                  <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                    {t('providers.helpGoogleCode')}
                  </span>
                </div>

                <div className="modal-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowGoogleOAuthModal(false)}
                  >
                    {t('providers.btnClose')}
                  </button>
                  <button
                    type="submit"
                    className="btn btn-primary"
                    disabled={actionLoading || !googleOAuthCode.trim()}
                  >
                    {actionLoading ? <div className="spinner" /> : t('providers.btnCompleteAuth')}
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      )}

      {/* MODAL: CODEX ADD ACCOUNT */}
      {showCodexAddModal && (
        <div className="modal-backdrop" onClick={() => setShowCodexAddModal(false)}>
          <div
            className="modal-card"
            style={{ maxWidth: 500 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title">{t('providers.modalCodexAddTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowCodexAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleAddCodexAccount}>
              <div className="form-group">
                <label>{t('providers.labelCodexAuthPath')}</label>
                <input
                  type="text"
                  placeholder="/home/user/.codex/auth.json"
                  value={codexAuthPath}
                  onChange={(e) => setCodexAuthPath(e.target.value)}
                  required
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  {t('providers.helpCodexAuthPath')}
                </span>
              </div>

              <div className="form-group">
                <label>{t('providers.labelCodexEmail')}</label>
                <input
                  type="text"
                  placeholder="codex-user@example.com"
                  value={codexEmail}
                  onChange={(e) => setCodexEmail(e.target.value)}
                />
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowCodexAddModal(false)}
                >
                  {t('providers.btnCancel')}
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : t('providers.btnAddToPool')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* MODAL: CODEX OAUTH PKCE */}
      {showCodexOAuthModal && (
        <div className="modal-backdrop" onClick={() => setShowCodexOAuthModal(false)}>
          <div
            className="modal-card"
            style={{ maxWidth: 540 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title">{t('providers.modalCodexOAuthTitle')}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowCodexOAuthModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <div style={{ marginBottom: 16 }}>
              <div style={{ marginBottom: 12 }}>
                <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>{t('providers.sessionStatusLabel')} </span>
                <span className="badge badge-warning font-mono">
                  {codexOAuthStatus || t('providers.waitingAuth')}
                </span>
              </div>

              {codexOAuthTicket?.authorize_url && (
                <a
                  href={codexOAuthTicket.authorize_url}
                  target="_blank"
                  rel="noreferrer"
                  className="btn btn-secondary"
                  style={{ width: '100%', justifyContent: 'center', marginBottom: 16 }}
                >
                  {t('providers.openCodexLoginPage')}
                </a>
              )}

              <form onSubmit={handleExchangeCodexOAuth}>
                <div className="form-group">
                  <label>{t('providers.labelCodexCode')}</label>
                  <textarea
                    rows={3}
                    placeholder={t('providers.placeholderCodexCode')}
                    value={codexOAuthCode}
                    onChange={(e) => setCodexOAuthCode(e.target.value)}
                    required
                    style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                  />
                  <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                    {t('providers.helpCodexCode')}
                  </span>
                </div>

                <div className="modal-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowCodexOAuthModal(false)}
                  >
                    {t('providers.btnClose')}
                  </button>
                  <button
                    type="submit"
                    className="btn btn-primary"
                    disabled={actionLoading || !codexOAuthCode.trim()}
                  >
                    {actionLoading ? <div className="spinner" /> : t('providers.btnCompleteAuth')}
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      )}

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
                  style={{ color: 'var(--primary)', textDecoration: 'underline' }}
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
                  Đã chọn: <strong style={{ color: 'var(--primary)' }}>{selectedOpenRouterModels.length}</strong> model
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
                              <div className="font-mono" style={{ fontSize: 13, fontWeight: isSelected ? 600 : 500, color: isSelected ? '#818cf8' : 'var(--text-primary)' }}>
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

      {/* MODAL: QUOTA VIEWER */}
      {selectedQuota && (
        <div className="modal-backdrop" onClick={() => setSelectedQuota(null)}>
          <div
            className="modal-card"
            style={{ maxWidth: 560 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title">{selectedQuota.title}</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setSelectedQuota(null)}
              >
                <IconX size={16} />
              </button>
            </div>
            <pre className="code-viewer-box">
              {JSON.stringify(selectedQuota.quota, null, 2)}
            </pre>
            <div className="modal-actions">
              {selectedQuota.onRefreshQuota && (
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={selectedQuota.onRefreshQuota}
                  disabled={actionLoading}
                >
                  <IconRefresh size={14} />
                  <span>{t('providers.btnRefreshAccountQuota')}</span>
                </button>
              )}
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setSelectedQuota(null)}
              >
                {t('providers.btnClose')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* MODAL: CODEX RATE LIMIT RESET CREDIT */}
      {resetCreditModalAccount && (
        <div
          className="modal-backdrop"
          onClick={() => !consumingCredit && setResetCreditModalAccount(null)}
          role="dialog"
          aria-modal="true"
          aria-labelledby="codex-reset-modal-title"
        >
          <div
            className="modal-card"
            style={{ maxWidth: 520 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h3 className="modal-title" id="codex-reset-modal-title">
                {t('providers.modalResetCreditTitle')}
              </h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setResetCreditModalAccount(null)}
                disabled={consumingCredit}
                aria-label={t('providers.modalResetCreditBtnCancel')}
              >
                <IconX size={16} />
              </button>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12, fontSize: 13 }}>
              <p style={{ color: 'var(--text-secondary)', margin: 0 }}>
                {t('providers.modalResetCreditDesc')}
              </p>

              <div
                style={{
                  background: 'var(--bg-card)',
                  border: '1px solid var(--border-color)',
                  borderRadius: 6,
                  padding: 12,
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 6,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap' }}>
                  <span style={{ color: 'var(--text-muted)' }}>{t('providers.thAccount')}:</span>
                  <strong style={{ color: 'var(--text-primary)', wordBreak: 'break-all' }}>
                    {resetCreditModalAccount.email || resetCreditModalAccount.id}
                  </strong>
                </div>
                {(() => {
                  const q = getCodexQuotaNumbers(resetCreditModalAccount);
                  return (
                    <>
                      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap' }}>
                        <span style={{ color: 'var(--text-muted)' }}>Primary (5h):</span>
                        <span>
                          <strong>{q.primaryRemaining ?? '—'}%</strong> {t('providers.modalResetCreditRemaining')}
                          {formatResetTime(q.primaryResetTime) && ` (${formatResetTime(q.primaryResetTime)})`}
                        </span>
                      </div>
                      {q.weeklyRemaining !== null && (
                        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap' }}>
                          <span style={{ color: 'var(--text-muted)' }}>Weekly:</span>
                          <span>
                            <strong>{q.weeklyRemaining}%</strong> {t('providers.modalResetCreditRemaining')}
                            {formatResetTime(q.weeklyResetTime) && ` (${formatResetTime(q.weeklyResetTime)})`}
                          </span>
                        </div>
                      )}
                      <div style={{ display: 'flex', justifyContent: 'space-between', borderTop: '1px solid var(--border-color)', paddingTop: 6, marginTop: 4 }}>
                        <span style={{ color: 'var(--text-muted)' }}>{t('providers.thQuotaLimit')}:</span>
                        <span className="badge badge-info font-mono">
                          🎟️ {t('providers.modalResetCreditAvailable', { count: q.availableTickets })}
                        </span>
                      </div>
                    </>
                  );
                })()}
              </div>

              {resetCreditsLoading ? (
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--text-muted)' }}>
                  <div className="spinner" />
                  <span>{t('providers.modalResetCreditLoading')}</span>
                </div>
              ) : resetCreditsList.length > 0 ? (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                  <span style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                    {t('providers.modalResetCreditListTitle')}
                  </span>
                  <div style={{ maxHeight: 160, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 6 }}>
                    {resetCreditsList.map((c) => (
                      <label
                        key={c.id}
                        style={{
                          display: 'flex',
                          alignItems: 'flex-start',
                          gap: 8,
                          padding: 8,
                          borderRadius: 6,
                          border: selectedCreditId === c.id ? '1px solid var(--primary)' : '1px solid var(--border-color)',
                          background: selectedCreditId === c.id ? 'var(--primary-subtle, rgba(59, 130, 246, 0.08))' : 'var(--bg-card)',
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="radio"
                          name="selected_codex_credit"
                          checked={selectedCreditId === c.id}
                          onChange={() => setSelectedCreditId(c.id)}
                          style={{ marginTop: 2 }}
                        />
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 2, fontSize: 12 }}>
                          <span style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                            {c.title || 'Full reset (Weekly + 5 hr)'}
                          </span>
                          <span style={{ color: 'var(--text-muted)' }}>
                            {c.description || 'Rate limit reset'}
                          </span>
                          {c.expires_at && (
                            <span style={{ fontSize: 11, color: 'var(--text-warning, #f59e0b)' }}>
                              {t('providers.modalResetCreditExpires', { date: new Date(c.expires_at).toLocaleDateString() })}
                            </span>
                          )}
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
              ) : (
                <div style={{ color: 'var(--text-muted)', fontSize: 12 }}>
                  {t('providers.noResetTicketsAvailable')}
                </div>
              )}

              <div
                style={{
                  background: 'rgba(239, 68, 68, 0.08)',
                  border: '1px solid rgba(239, 68, 68, 0.25)',
                  borderRadius: 6,
                  padding: 10,
                  color: 'var(--error, #ef4444)',
                  fontSize: 12,
                }}
              >
                ⚠️ {t('providers.modalResetCreditWarning')}
              </div>
            </div>

            <div className="modal-actions" style={{ marginTop: 16 }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setResetCreditModalAccount(null)}
                disabled={consumingCredit}
              >
                {t('providers.modalResetCreditBtnCancel')}
              </button>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={handleConfirmConsumeResetCredit}
                disabled={consumingCredit || resetCreditsLoading || !selectedCreditId || resetCreditsList.length === 0}
              >
                {consumingCredit ? <div className="spinner" /> : t('providers.modalResetCreditBtnConfirm')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
