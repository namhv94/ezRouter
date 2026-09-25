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
          ? 'Đã bật tự động làm mới Quota định kỳ.'
          : 'Đã tắt tự động làm mới Quota.',
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Lỗi cập nhật cấu hình tự động làm mới quota.',
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
        text: `Đã đổi chu kỳ làm mới Quota thành ${intervalSecs / 60} phút.`,
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Lỗi đổi chu kỳ làm mới quota.',
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
        text: `Làm mới quota hoàn tất (${res.last_summary?.duration_ms || 0}ms): Google +${res.last_summary?.google_refreshed || 0}, Codex +${res.last_summary?.codex_refreshed || 0}`,
      });
      onRefresh();
      loadCodexData();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Lỗi làm mới quota.',
      });
    } finally {
      setQuotaRefreshLoading(false);
    }
  };

  const formatTimestamp = (ts: number | null) => {
    if (!ts) return 'Chưa thực hiện';
    const date = new Date(ts * 1000);
    return date.toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  };

  const formatCountdown = (nextTs: number | null) => {
    if (!nextTs || !quotaRefresh?.enabled) return '-';
    const diff = Math.round(nextTs - Date.now() / 1000);
    if (diff <= 0) return 'Đang đến hạn...';
    if (diff < 60) return `trong ${diff}s`;
    const m = Math.floor(diff / 60);
    const s = diff % 60;
    return `trong ${m}m ${s}s`;
  };

  useEffect(() => {
    loadCodexData();
    loadQuotaRefreshStatus();
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
            ? `Đăng nhập Google thành công cho tài khoản mới ${addedEmail}!`
            : `Tài khoản Google ${addedEmail} đã có sẵn trong pool — đã làm mới token (không tạo trùng)!`,
        });
        setShowGoogleOAuthModal(false);
        setGoogleOAuthCode('');
        setGoogleOAuthTicket(null);
        onRefresh();
      }
    };
    window.addEventListener('message', handleWindowMessage);
    return () => window.removeEventListener('message', handleWindowMessage);
  }, [onRefresh]);

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
              text: `Tài khoản Codex ${res.email || ''} đã xác thực thành công!`,
            });
            setShowCodexOAuthModal(false);
            setCodexOAuthTicket(null);
            loadCodexData();
          } else if (res.status === 'error') {
            stopped = true;
            clearInterval(interval);
            setActionMessage({
              type: 'error',
              text: `Lỗi OAuth Codex: ${res.error || res.message || 'Thất bại'}`,
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
  }, [showCodexOAuthModal, codexOAuthTicket]);

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
            text: 'Đã mở cửa sổ đăng nhập Google OAuth. Hoàn tất trên cửa sổ đó hoặc dán URL callback bên dưới.',
          });
        }
      }
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Không thể khởi tạo đăng nhập Google.',
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
          ? `Đã kết nối tài khoản Google mới ${res.email} thành công!`
          : `Tài khoản Google ${res.email} đã có sẵn trong pool — đã làm mới token (không tạo trùng)!`,
      });
      setShowGoogleOAuthModal(false);
      setGoogleOAuthCode('');
      setGoogleOAuthTicket(null);
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Đổi mã xác thực Google OAuth thất bại.',
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleAddGoogleAccount = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!googleRefreshToken.trim()) {
      setActionMessage({ type: 'error', text: 'Vui lòng cung cấp Refresh Token.' });
      return;
    }
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.createAccount({
        email: googleEmail.trim() || undefined,
        refresh_token: googleRefreshToken.trim(),
      });
      setActionMessage({ type: 'success', text: 'Đã thêm tài khoản Google Antigravity mới.' });
      setShowGoogleAddModal(false);
      setGoogleEmail('');
      setGoogleRefreshToken('');
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể thêm tài khoản.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleResetGoogleCooldown = async (acc: AccountResponse) => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.resetAccount(acc.id);
      setActionMessage({ type: 'success', text: `Đã reset cooldown cho "${acc.email}".` });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi khi reset cooldown.' });
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
        text: `Đã cập nhật hạn mức quota cho "${acc.email}".`,
      });
      setSelectedQuota((prev) => ({
        title: `Hạn Mức Quota Google (${acc.email})`,
        quota: res.quota,
        onRefreshQuota: prev?.onRefreshQuota || (() => handleRefreshGoogleQuota(acc)),
      }));
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi làm mới hạn mức quota.' });
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
        text: 'Đã làm mới quota cho tất cả tài khoản Google trong pool.',
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi làm mới quota toàn pool.' });
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
        text: `Kiểm tra token "${acc.email}": Thành công (${JSON.stringify(res)})`,
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Kiểm tra "${acc.email}" thất bại: ${err?.message || 'Lỗi xác thực'}`,
      });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDeleteGoogleAccount = async (acc: AccountResponse) => {
    if (!window.confirm(`Xác nhận xóa tài khoản Google "${acc.email}"?`)) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteAccount(acc.id);
      setActionMessage({ type: 'success', text: `Đã xóa tài khoản Google "${acc.email}".` });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể xóa tài khoản.' });
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
        text: err?.message || 'Không thể khởi tạo luồng OAuth Codex.',
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
        text: 'Xác thực OAuth Codex hoàn tất và đã thêm vào pool.',
      });
      setShowCodexOAuthModal(false);
      setCodexOAuthTicket(null);
      setCodexOAuthCode('');
      loadCodexData();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Đổi mã xác thực OAuth Codex thất bại.',
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
        text: 'Vui lòng cung cấp đường dẫn tệp auth (auth_path).',
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
      setActionMessage({ type: 'success', text: 'Đã thêm tài khoản Codex thành công.' });
      setShowCodexAddModal(false);
      setCodexAuthPath('');
      setCodexEmail('');
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi thêm tài khoản Codex.' });
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
        text: `Đã ${isCurrentlyActive ? 'tắt' : 'bật'} tài khoản Codex "${acc.email || acc.id}".`,
      });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: err?.message || 'Lỗi thay đổi trạng thái tài khoản Codex.',
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
        text: `Đã reset cooldown tài khoản Codex "${acc.email || acc.id}".`,
      });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi reset cooldown Codex.' });
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
        text: `Đã cập nhật hạn mức quota cho "${acc.email || acc.id}".`,
      });
      setSelectedQuota((prev) => ({
        title: `Hạn Mức Quota Codex (${acc.email || acc.id})`,
        quota: res.quota,
        onRefreshQuota: prev?.onRefreshQuota || (() => handleRefreshCodexAccountQuota(acc)),
      }));
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi làm mới hạn mức quota Codex.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleRefreshAllCodexQuota = async () => {
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.refreshCodexQuota();
      setActionMessage({
        type: 'success',
        text: 'Đã làm mới quota toàn bộ pool OpenAI Codex.',
      });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi làm mới quota Codex pool.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDeleteCodex = async (acc: CodexAccountRecord) => {
    if (!window.confirm(`Xác nhận xóa tài khoản Codex "${acc.email || acc.id}"?`)) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteCodexAccount(acc.id);
      setActionMessage({ type: 'success', text: 'Đã xóa tài khoản Codex khỏi hệ thống.' });
      loadCodexData();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể xóa tài khoản Codex.' });
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
        setActionMessage({ type: 'warning', text: 'Upstream không trả về model nào.' });
      } else {
        setModelsInput(JSON.stringify(models, null, 2));
        setActionMessage({ type: 'success', text: `Đã tự tải ${models.length} model từ upstream.` });
      }
      setModelsLoadedFor(url);
    } catch (err: any) {
      setActionMessage({
        type: 'warning',
        text: `Không tự tải được models: ${err?.message || 'Lỗi kết nối upstream'}. Bạn vẫn có thể nhập thủ công.`,
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
        text: 'Vui lòng điền đầy đủ Tên, Tiền tố và Base URL.',
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
          text: `Đã cập nhật nhà cung cấp "${name}" thành công.`,
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
          text: `Đã thêm nhà cung cấp "${name}" thành công.`,
        });
      }
      setShowAddProviderModal(false);
      resetUpstreamForm();
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Lỗi khi lưu nhà cung cấp.' });
    } finally {
      setActionLoading(false);
    }
  };

  const handleDeleteUpstream = async (p: ProviderResponse) => {
    if (!window.confirm(`Xác nhận xóa nhà cung cấp "${p.name}" (${p.prefix})?`)) return;
    setActionLoading(true);
    setActionMessage(null);
    try {
      await api.deleteProvider(p.id);
      setActionMessage({ type: 'success', text: `Đã xóa nhà cung cấp "${p.name}".` });
      onRefresh();
    } catch (err: any) {
      setActionMessage({ type: 'error', text: err?.message || 'Không thể xóa nhà cung cấp.' });
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
        text: `Kiểm tra "${p.name}": ${JSON.stringify(res)}`,
      });
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Kiểm tra "${p.name}" thất bại: ${err?.message || 'Lỗi upstream'}`,
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
        text: `Đồng bộ "${p.name}": ${JSON.stringify(res)}`,
      });
      onRefresh();
    } catch (err: any) {
      setActionMessage({
        type: 'error',
        text: `Đồng bộ "${p.name}" thất bại: ${err?.message || 'Lỗi đồng bộ'}`,
      });
    } finally {
      setActionLoading(false);
    }
  };

  const formatResetTime = (isoString?: string) => {
    if (!isoString) return null;
    try {
      const d = new Date(isoString);
      if (isNaN(d.getTime())) return null;
      const diffSec = Math.round((d.getTime() - Date.now()) / 1000);
      if (diffSec <= 0) return 'sắp reset';
      if (diffSec < 60) return `${diffSec}s`;
      if (diffSec < 3600) return `${Math.round(diffSec / 60)}m`;
      if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ${Math.round((diffSec % 3600) / 60)}m`;
      const days = Math.floor(diffSec / 86400);
      const hours = Math.round((diffSec % 86400) / 3600);
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
        weeklyUsed: null,
        weeklyRemaining: null,
        isStopped: false,
      };
    }
    const pUsed =
      acc.quota.primary_window?.used_percent ??
      (acc.quota.primary_percent !== undefined ? 100 - acc.quota.primary_percent : null);
    const pRem = pUsed !== null ? Math.max(0, Math.min(100, Math.round(100 - pUsed))) : null;
    const wUsed =
      acc.quota.weekly_window?.used_percent ??
      acc.quota.secondary_window?.used_percent ??
      (acc.quota.secondary_percent !== undefined ? 100 - acc.quota.secondary_percent : null);
    const wRem = wUsed !== null ? Math.max(0, Math.min(100, Math.round(100 - wUsed))) : null;
    const isStopped = pUsed !== null && pUsed >= 98.0;
    return { primaryUsed: pUsed, primaryRemaining: pRem, weeklyUsed: wUsed, weeklyRemaining: wRem, isStopped };
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
    if (accounts.length === 0) return { label: 'Chưa có tài khoản', badge: 'badge-neutral', status: 'neutral' };
    const activeAccounts = accounts.filter((a) => a.is_active);
    if (activeAccounts.length === 0) return { label: 'Đang tắt', badge: 'badge-neutral', status: 'neutral' };
    if (googleActiveCount === 0 && googleCooldownCount > 0) return { label: 'Đang Cooldown', badge: 'badge-warning', status: 'cooldown' };
    if (claude5hAccs.length > 0 && googleClaudeStoppedCount >= activeAccounts.length) {
      return { label: 'Dừng Quota Claude', badge: 'badge-error', status: 'stopped' };
    }
    if (gemini5hAccs.length > 0 && googleGeminiStoppedCount >= activeAccounts.length) {
      return { label: 'Dừng Quota Gemini', badge: 'badge-error', status: 'stopped' };
    }
    if (googleClaudeStoppedCount > 0) {
      return { label: `${googleClaudeStoppedCount} TK hết Claude`, badge: 'badge-warning', status: 'cooldown' };
    }
    if (googleGeminiStoppedCount > 0) {
      return { label: `${googleGeminiStoppedCount} TK hết Gemini`, badge: 'badge-warning', status: 'cooldown' };
    }
    return { label: 'Sẵn sàng hoạt động', badge: 'badge-success', status: 'active' };
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
    if (codexTotalCount === 0) return { label: 'Chưa có tài khoản', badge: 'badge-neutral', status: 'neutral' };
    const activeAccounts = codexAccounts.filter((a) => (a.is_active ?? a.active ?? true));
    if (activeAccounts.length === 0 && codexAccounts.length > 0) return { label: 'Đang tắt', badge: 'badge-neutral', status: 'neutral' };
    if (codexActiveCount === 0 && codexCooldownCount > 0) return { label: 'Đang Cooldown', badge: 'badge-warning', status: 'cooldown' };
    if (codexPrimaryAccs.length > 0 && codexStoppedCount >= (activeAccounts.length || 1)) {
      return { label: 'Dừng Quota (>=98%)', badge: 'badge-error', status: 'stopped' };
    }
    if (codexStoppedCount > 0) {
      return { label: `${codexStoppedCount} TK dừng quota`, badge: 'badge-warning', status: 'cooldown' };
    }
    return { label: 'Sẵn sàng hoạt động', badge: 'badge-success', status: 'active' };
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
            {percent !== null ? (subLabel || 'Hạn mức khả dụng') : 'Chưa có Quota'}
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
        title={`${label} (Activity Indicator - không biểu thị lịch sử thực tế)`}
        aria-label={`${label} (Activity Indicator)`}
      >
        <div className="activity-indicator-badge">
          <span className="activity-pulse-dot" style={{ backgroundColor: strokeColor }} />
          <span>{label}</span>
          <span className="activity-indicator-sub">(Activity Indicator)</span>
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
            <span>Nhà Cung Cấp & Pools</span>
          </h2>
          <span className="providers-header-sub">
            Hạ tầng upstream Google Antigravity & OpenAI Codex
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
            title="Làm mới trạng thái toàn bộ nhà cung cấp"
          >
            <IconRefresh size={15} />
            <span>Làm Mới</span>
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
              ? 'Hệ thống định tuyến sẵn sàng'
              : totalCooling > 0
              ? 'Đang chờ cooldown'
              : 'Chưa có tài khoản hoạt động'}
          </span>
          <span style={{ color: 'var(--text-muted)', fontWeight: 400, fontSize: 11.5, marginLeft: 4 }}>
            ({googleActiveCount + codexActiveCount}/{accounts.length + codexTotalCount} sẵn sàng)
          </span>
        </div>

        <div className="status-strip-chips">
          <button
            type="button"
            className="status-segment-chip"
            onClick={() => document.getElementById('provider-google')?.scrollIntoView({ behavior: 'smooth' })}
            title="Cuộn tới Google Antigravity"
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
            title="Cuộn tới OpenAI Codex"
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
            onClick={() => document.getElementById('provider-upstream')?.scrollIntoView({ behavior: 'smooth' })}
            title="Cuộn tới Upstream Tùy Biến"
          >
            <IconServer size={14} />
            <span>
              Upstream: <strong>{providers.filter((p) => p.is_active).length}/{providers.length}</strong>
            </span>
            <span className="status-dot-mini active" />
          </button>
        </div>

        <div className="status-legend-items">
          <span className="status-legend-item">
            <span className="status-dot-mini active" />
            <span>Hoạt động</span>
          </span>
          <span className="status-legend-item">
            <span className="status-dot-mini cooldown" />
            <span>Cooldown</span>
          </span>
          <span className="status-legend-item">
            <span className="status-dot-mini stopped" />
            <span>Dừng Quota</span>
          </span>
          <span className="status-legend-item">
            <span className="status-dot-mini neutral" />
            <span>Tắt</span>
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
            <span className="auto-quota-strip-title">Tự Động Làm Mới Quota</span>
            <span className="auto-quota-strip-meta">
              {quotaRefresh?.is_refreshing ? (
                <span className="highlight-refreshing">Đang làm mới...</span>
              ) : quotaRefresh?.enabled ? (
                <>
                  <span>Lần cuối: <strong>{formatTimestamp(quotaRefresh?.last_refresh ?? null)}</strong></span>
                  <span style={{ margin: '0 4px', color: 'var(--text-muted)' }}>•</span>
                  <span>Kế tiếp: <strong className="highlight">{formatCountdown(quotaRefresh?.next_refresh ?? null)}</strong></span>
                </>
              ) : (
                <span style={{ color: 'var(--text-muted)' }}>Đang tắt</span>
              )}
            </span>
          </div>
        </div>

        <div className="auto-quota-strip-right">
          <div className="auto-quota-interval-group" role="radiogroup" aria-label="Chu kỳ làm mới">
            {[
              { label: '1m', val: 60, title: '1 phút' },
              { label: '5m', val: 300, title: '5 phút (Mặc định)' },
              { label: '15m', val: 900, title: '15 phút' },
              { label: '30m', val: 1800, title: '30 phút' },
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
            title={quotaRefresh?.enabled ? 'Nhấn để tắt tự động làm mới' : 'Nhấn để bật tự động làm mới'}
          >
            <span className="toggle-switch-track">
              <span className="toggle-switch-thumb" />
            </span>
            <span className="toggle-switch-text">{quotaRefresh?.enabled ? 'BẬT' : 'TẮT'}</span>
          </button>

          <button
            type="button"
            className="action-icon-btn primary manual-refresh-btn"
            onClick={handleManualRunRefresh}
            disabled={quotaRefreshLoading || quotaRefresh?.is_refreshing}
            title="Kích hoạt làm mới Quota an toàn ngay bây giờ"
          >
            <IconRefresh size={13} className={quotaRefreshLoading || quotaRefresh?.is_refreshing ? 'spinner' : ''} />
            <span>Làm mới</span>
          </button>
        </div>

        {quotaRefresh?.last_error && (
          <div className="auto-quota-error-notice" style={{ width: '100%', marginTop: 4 }}>
            <IconAlertCircle size={14} />
            <span>Lưu ý lần làm mới trước: {quotaRefresh.last_error}</span>
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
              {renderHealthRing(googleQuotaAvg, 'Hạn mức TB', 'Quota khả dụng')}
              <div className="visual-stat-numbers">
                <div className="stat-num-box">
                  <span className="stat-num-val active">{googleActiveCount}</span>
                  <span className="stat-num-lbl">Hoạt động</span>
                </div>
                <div className="stat-num-box">
                  <span className={`stat-num-val ${googleCooldownCount > 0 ? 'cooldown' : ''}`}>
                    {googleCooldownCount}
                  </span>
                  <span className="stat-num-lbl">Cooldown</span>
                </div>
                <div className="stat-num-box">
                  <span className="stat-num-val total">{accounts.length}</span>
                  <span className="stat-num-lbl">Tổng số</span>
                </div>
              </div>
            </div>

            {/* Micro Quota Bars & Sparkline */}
            <div className="visual-quota-bars">
              <div className="micro-bar-row">
                <span className="micro-bar-label">Gemini 5h:</span>
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
                  <span className="micro-bar-label">Gemini Tuần:</span>
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
                <span className="micro-bar-label">Claude 5h:</span>
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
                  <span className="micro-bar-label">Claude Tuần:</span>
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

              {renderActivityPulse('#4285F4', 'Nhịp hoạt động Google')}
            </div>

            {/* Consolidated Action Bar */}
            <div className="segment-action-bar">
              <div className="segment-actions-group">
                <button
                  className="action-icon-btn"
                  onClick={handleRefreshAllGoogleQuota}
                  disabled={actionLoading || accounts.length === 0}
                  title="Làm mới Quota toàn pool Google"
                  aria-label="Làm mới Quota toàn pool Google"
                >
                  <IconRefresh size={14} />
                  <span>Quota</span>
                </button>
                <button
                  className="action-icon-btn"
                  onClick={handleStartGoogleOAuth}
                  disabled={actionLoading}
                  title="Đăng nhập Google qua OAuth 1-Click"
                  aria-label="Đăng nhập Google qua OAuth 1-Click"
                >
                  <IconShield size={14} />
                  <span>OAuth Google</span>
                </button>
                <button
                  className="action-icon-btn primary"
                  onClick={() => setShowGoogleAddModal(true)}
                  disabled={actionLoading}
                  title="Thêm tài khoản thủ công qua Refresh Token"
                  aria-label="Thêm tài khoản thủ công qua Refresh Token"
                >
                  <IconPlus size={14} />
                  <span>Thêm Token</span>
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
                    ? 'Thu gọn danh sách tài khoản'
                    : `Chi tiết tài khoản (${accounts.length} trong pool)`}
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
                    <p style={{ margin: 0, fontSize: 13 }}>Chưa có tài khoản Google Antigravity nào.</p>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={handleStartGoogleOAuth}
                    >
                      Kết nối Google OAuth
                    </button>
                  </div>
                ) : (
                  <div className="table-container">
                    <table>
                      <thead>
                        <tr>
                          <th>Tài Khoản</th>
                          <th>Trạng Thái</th>
                          <th>Cooldown</th>
                          <th>Quota Còn Lại</th>
                          <th>Tác Vụ</th>
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
                                  {acc.is_active ? 'Bật' : 'Tắt'}
                                </span>
                              </td>
                              <td>
                                {isCooling ? (
                                  <span className="badge badge-warning font-mono">
                                    {Math.ceil(acc.cooldown_remaining)}s
                                  </span>
                                ) : (
                                  <span className="badge badge-success">Sẵn sàng</span>
                                )}
                              </td>
                              <td>
                                {acc.quota && (acc.quota.gemini_5h || acc.quota.claude_5h || acc.quota.gemini_weekly || acc.quota.claude_weekly) ? (
                                  <div style={{ fontSize: 11, display: 'flex', flexDirection: 'column', gap: 2 }}>
                                    {acc.quota.gemini_5h && (
                                      <span>
                                        Gemini 5h: <strong>{acc.quota.gemini_5h.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.gemini_5h.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={`Reset lúc: ${acc.quota.gemini_5h.reset_time}`}>
                                            ({formatResetTime(acc.quota.gemini_5h.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    {acc.quota.gemini_weekly && (
                                      <span>
                                        Gemini Tuần: <strong>{acc.quota.gemini_weekly.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.gemini_weekly.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={`Reset lúc: ${acc.quota.gemini_weekly.reset_time}`}>
                                            ({formatResetTime(acc.quota.gemini_weekly.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    {acc.quota.claude_5h && (
                                      <span>
                                        Claude 5h: <strong>{acc.quota.claude_5h.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.claude_5h.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={`Reset lúc: ${acc.quota.claude_5h.reset_time}`}>
                                            ({formatResetTime(acc.quota.claude_5h.reset_time)})
                                          </span>
                                        )}
                                      </span>
                                    )}
                                    {acc.quota.claude_weekly && (
                                      <span>
                                        Claude Tuần: <strong>{acc.quota.claude_weekly.remaining_percent}%</strong>
                                        {formatResetTime(acc.quota.claude_weekly.reset_time) && (
                                          <span style={{ color: 'var(--text-muted)', fontSize: 10, marginLeft: 4 }} title={`Reset lúc: ${acc.quota.claude_weekly.reset_time}`}>
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
                                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Chưa nạp</span>
                                )}
                              </td>
                              <td>
                                <div className="table-actions">
                                  <button
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => handleTestGoogleAccount(acc)}
                                    disabled={actionLoading}
                                    title="Kiểm tra token"
                                  >
                                    Test
                                  </button>
                                  {isCooling && (
                                    <button
                                      className="btn btn-secondary btn-sm"
                                      onClick={() => handleResetGoogleCooldown(acc)}
                                      disabled={actionLoading}
                                      title="Reset Cooldown"
                                    >
                                      Reset
                                    </button>
                                  )}
                                  <button
                                    className="btn btn-danger btn-sm btn-icon-only"
                                    onClick={() => handleDeleteGoogleAccount(acc)}
                                    disabled={actionLoading}
                                    title="Xóa tài khoản"
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
              {renderHealthRing(codexPrimaryAvg, 'Primary TB', 'Cửa sổ 5h khả dụng')}
              <div className="visual-stat-numbers">
                <div className="stat-num-box">
                  <span className="stat-num-val active">{codexActiveCount}</span>
                  <span className="stat-num-lbl">Hoạt động</span>
                </div>
                <div className="stat-num-box">
                  <span className={`stat-num-val ${codexCooldownCount > 0 ? 'cooldown' : ''}`}>
                    {codexCooldownCount}
                  </span>
                  <span className="stat-num-lbl">Cooldown</span>
                </div>
                <div className="stat-num-box">
                  <span className="stat-num-val total">{codexTotalCount}</span>
                  <span className="stat-num-lbl">Tổng số</span>
                </div>
              </div>
            </div>

            {/* Micro Quota Bars & Sparkline */}
            <div className="visual-quota-bars">
              <div className="micro-bar-row">
                <span className="micro-bar-label">Primary (5h):</span>
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
                <span className="micro-bar-label">Weekly:</span>
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

              {renderActivityPulse('#10a37f', 'Nhịp hoạt động Codex')}
            </div>

            {/* Consolidated Action Bar */}
            <div className="segment-action-bar">
              <div className="segment-actions-group">
                <button
                  className="action-icon-btn"
                  onClick={handleRefreshAllCodexQuota}
                  disabled={actionLoading}
                  title="Làm mới Quota toàn pool Codex"
                  aria-label="Làm mới Quota toàn pool Codex"
                >
                  <IconRefresh size={14} />
                  <span>Quota</span>
                </button>
                <button
                  className="action-icon-btn"
                  onClick={handleStartCodexOAuth}
                  disabled={actionLoading}
                  title="Khởi tạo luồng xác thực OAuth PKCE"
                  aria-label="Khởi tạo luồng xác thực OAuth PKCE"
                >
                  <IconShield size={14} />
                  <span>OAuth PKCE</span>
                </button>
                <button
                  className="action-icon-btn primary"
                  onClick={() => setShowCodexAddModal(true)}
                  disabled={actionLoading}
                  title="Thêm tệp auth.json"
                  aria-label="Thêm tệp auth.json"
                >
                  <IconPlus size={14} />
                  <span>Thêm Auth</span>
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
                    ? 'Thu gọn danh sách tài khoản'
                    : `Chi tiết tài khoản (${codexTotalCount} trong pool)`}
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
                    <p style={{ margin: 0, fontSize: 13 }}>Chưa có tài khoản OpenAI Codex nào.</p>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={handleStartCodexOAuth}
                    >
                      Bắt đầu OAuth PKCE
                    </button>
                  </div>
                ) : (
                  <div className="table-container">
                    <table>
                      <thead>
                        <tr>
                          <th>Tài Khoản</th>
                          <th>Trạng Thái</th>
                          <th>Cooldown</th>
                          <th>Hạn Mức</th>
                          <th>Tác Vụ</th>
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
                                  {acc.email || '(Chưa xác định)'}
                                </div>
                                <span className="badge badge-neutral font-mono" style={{ fontSize: 10, marginTop: 2 }}>
                                  id: {acc.id.slice(0, 8)}...
                                </span>
                              </td>
                              <td>
                                <span className={`badge ${isActiveAcc ? 'badge-success' : 'badge-neutral'}`}>
                                  {isActiveAcc ? 'Bật' : 'Tắt'}
                                </span>
                              </td>
                              <td>
                                {isCooling ? (
                                  <span className="badge badge-warning font-mono">
                                    {Math.ceil(acc.cooldown_remaining || 0)}s
                                  </span>
                                ) : (
                                  <span className="badge badge-success">Sẵn sàng</span>
                                )}
                              </td>
                              <td>
                                {q.primaryRemaining !== null ? (
                                  <div style={{ fontSize: 11, display: 'flex', flexDirection: 'column', gap: 2 }}>
                                    <span>Primary: <strong>{q.primaryRemaining}%</strong></span>
                                    {q.weeklyRemaining !== null && (
                                      <span>Weekly: <strong>{q.weeklyRemaining}%</strong></span>
                                    )}
                                    <button
                                      className="btn btn-secondary btn-sm font-mono"
                                      style={{ padding: '1px 6px', fontSize: 10, alignSelf: 'flex-start', marginTop: 2 }}
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
                                ) : (
                                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Chưa nạp</span>
                                )}
                              </td>
                              <td>
                                <div className="table-actions">
                                  <button
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => handleToggleCodex(acc)}
                                    disabled={actionLoading}
                                  >
                                    {isActiveAcc ? 'Tắt' : 'Bật'}
                                  </button>
                                  {isCooling && (
                                    <button
                                      className="btn btn-secondary btn-sm"
                                      onClick={() => handleResetCodex(acc)}
                                      disabled={actionLoading}
                                      title="Reset Cooldown"
                                    >
                                      Reset
                                    </button>
                                  )}
                                  <button
                                    className="btn btn-danger btn-sm btn-icon-only"
                                    onClick={() => handleDeleteCodex(acc)}
                                    disabled={actionLoading}
                                    title="Xóa tài khoản"
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

        {/* CARD 3: UPSTREAM CUSTOM PROVIDERS */}
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
              <span className="badge badge-neutral font-mono">{providers.length} nguồn</span>
            </div>

            {/* Health Ring & Big Numbers */}
            <div className="visual-metrics-row">
              {renderHealthRing(
                providers.length > 0
                  ? Math.round((providers.filter((p) => p.is_active).length / providers.length) * 100)
                  : 0,
                'Tỷ lệ hoạt động',
                'Nguồn sẵn sàng'
              )}
              <div className="visual-stat-numbers">
                <div className="stat-num-box">
                  <span className="stat-num-val active">
                    {providers.filter((p) => p.is_active).length}
                  </span>
                  <span className="stat-num-lbl">Hoạt động</span>
                </div>
                <div className="stat-num-box">
                  <span className="stat-num-val total">{providers.length}</span>
                  <span className="stat-num-lbl">Cấu hình</span>
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
                  title="Thêm nhà cung cấp upstream mới"
                  aria-label="Thêm nhà cung cấp upstream mới"
                >
                  <IconPlus size={14} />
                  <span>Thêm Upstream</span>
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
                    ? 'Thu gọn danh sách upstream'
                    : `Chi tiết upstream (${providers.length} nguồn)`}
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
                {providers.length === 0 ? (
                  <div className="empty-graphic-box">
                    <p style={{ margin: 0, fontSize: 13 }}>Chưa có nhà cung cấp upstream nào được cấu hình.</p>
                    <button className="btn btn-secondary btn-sm" onClick={openAddUpstreamModal}>
                      Thêm Upstream đầu tiên
                    </button>
                  </div>
                ) : (
                  <div className="table-container">
                    <table>
                      <thead>
                        <tr>
                          <th>Tên & Prefix</th>
                          <th>Loại</th>
                          <th>Base URL</th>
                          <th>API Key</th>
                          <th>Models</th>
                          <th>Trạng Thái</th>
                          <th>Tác Vụ</th>
                        </tr>
                      </thead>
                      <tbody>
                        {providers.map((p) => {
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
                                  {p.api_key ? '••••••••' : '(Trống)'}
                                </span>
                              </td>
                              <td>
                                <span className="badge badge-neutral" style={{ fontSize: 10 }}>
                                  {modelsArr.length > 0 ? `${modelsArr.length} models` : 'Mặc định'}
                                </span>
                              </td>
                              <td>
                                <span className={`badge ${p.is_active ? 'badge-success' : 'badge-neutral'}`}>
                                  {p.is_active ? 'Bật' : 'Tắt'}
                                </span>
                              </td>
                              <td>
                                <div className="table-actions">
                                  <button
                                    className="btn btn-secondary btn-sm btn-icon-only"
                                    onClick={() => openEditUpstreamModal(p)}
                                    title="Sửa cấu hình"
                                  >
                                    <IconEdit size={14} />
                                  </button>
                                  <button
                                    className="btn btn-secondary btn-sm"
                                    onClick={() => handleTestUpstream(p)}
                                    disabled={actionLoading}
                                    title="Kiểm tra kết nối"
                                  >
                                    Test
                                  </button>
                                  <button
                                    className="btn btn-secondary btn-sm btn-icon-only"
                                    onClick={() => handleSyncUpstream(p)}
                                    disabled={actionLoading}
                                    title="Đồng bộ models"
                                  >
                                    <IconRefresh size={14} />
                                  </button>
                                  <button
                                    className="btn btn-danger btn-sm btn-icon-only"
                                    onClick={() => handleDeleteUpstream(p)}
                                    disabled={actionLoading}
                                    title="Xóa nhà cung cấp"
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
              <h3 className="modal-title">Thêm Tài Khoản Google Antigravity</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowGoogleAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleAddGoogleAccount}>
              <div className="form-group">
                <label>Email Google (tùy chọn hoặc định danh)</label>
                <input
                  type="text"
                  placeholder="user@example.com"
                  value={googleEmail}
                  onChange={(e) => setGoogleEmail(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label>OAuth Refresh Token (Bắt buộc)</label>
                <textarea
                  rows={4}
                  placeholder="1//04..."
                  value={googleRefreshToken}
                  onChange={(e) => setGoogleRefreshToken(e.target.value)}
                  required
                  style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  Token được mã hóa bảo mật và không bao giờ xuất hiện trong phản hồi API.
                </span>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowGoogleAddModal(false)}
                >
                  Hủy
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : 'Thêm Vào Pool'}
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
              <h3 className="modal-title">Đăng Nhập Google OAuth (Antigravity)</h3>
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
                  <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Mã phiên (State): </span>
                  <span className="badge badge-warning font-mono" style={{ fontSize: 11 }}>
                    {googleOAuthTicket.state.length > 28
                      ? googleOAuthTicket.state.slice(0, 28) + '...'
                      : googleOAuthTicket.state}
                  </span>
                </div>
              )}

              <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 8 }}>
                Hoàn tất trên cửa sổ Google vừa mở, hoặc mở trực tiếp qua liên kết bên dưới:
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
                    Mở Trang Ủy Quyền Google
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
                          text: 'Đã sao chép link ủy quyền! Dán vào Cửa Sổ Ẩn Danh (Incognito) nếu muốn chọn tài khoản khác.',
                        });
                      }
                    }}
                  >
                    Sao Chép Link
                  </button>
                </div>
              )}

              <div style={{ fontSize: 11.5, color: 'var(--text-muted)', marginBottom: 14 }}>
                💡 <b>Mẹo thêm tài khoản mới:</b> Nếu Google tự động chọn tài khoản đang đăng nhập, hãy bấm <b>Sao Chép Link</b> và mở trong <b>Cửa sổ ẩn danh (Incognito)</b> để chọn hoặc đăng nhập tài khoản Google khác.
              </div>

              <form onSubmit={handleExchangeGoogleOAuth}>
                <div className="form-group">
                  <label>Mã Authorization Code hoặc Toàn Bộ URL Callback:</label>
                  <textarea
                    rows={3}
                    placeholder="Dán mã code hoặc toàn bộ URL callback (http://localhost:20229/auth/callback?code=...)"
                    value={googleOAuthCode}
                    onChange={(e) => setGoogleOAuthCode(e.target.value)}
                    required
                    style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                  />
                  <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                    Dán URL chuyển hướng sau khi đăng nhập Google hoặc mã authorization code để hoàn tất liên kết tài khoản.
                  </span>
                </div>

                <div className="modal-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowGoogleOAuthModal(false)}
                  >
                    Đóng
                  </button>
                  <button
                    type="submit"
                    className="btn btn-primary"
                    disabled={actionLoading || !googleOAuthCode.trim()}
                  >
                    {actionLoading ? <div className="spinner" /> : 'Hoàn Tất Ủy Quyền'}
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
              <h3 className="modal-title">Thêm Tài Khoản OpenAI Codex</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowCodexAddModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleAddCodexAccount}>
              <div className="form-group">
                <label>Đường dẫn tệp auth.json (Bắt buộc)</label>
                <input
                  type="text"
                  placeholder="/home/user/.codex/auth.json"
                  value={codexAuthPath}
                  onChange={(e) => setCodexAuthPath(e.target.value)}
                  required
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  Hệ thống sao chép tệp bảo mật vào kho riêng của proxy.
                </span>
              </div>

              <div className="form-group">
                <label>Email / Nhãn định danh (tùy chọn)</label>
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
                  Hủy
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : 'Thêm Vào Pool'}
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
              <h3 className="modal-title">Xác Thực OpenAI Codex OAuth (PKCE)</h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowCodexOAuthModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <div style={{ marginBottom: 16 }}>
              <div style={{ marginBottom: 12 }}>
                <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Trạng thái phiên: </span>
                <span className="badge badge-warning font-mono">
                  {codexOAuthStatus || 'Đang chờ xác thực...'}
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
                  Mở Trang Đăng Nhập OpenAI Codex
                </a>
              )}

              <form onSubmit={handleExchangeCodexOAuth}>
                <div className="form-group">
                  <label>Mã Authorization Code hoặc URL Callback</label>
                  <textarea
                    rows={3}
                    placeholder="Dán mã code hoặc toàn bộ URL localhost:1455/auth/callback?code=..."
                    value={codexOAuthCode}
                    onChange={(e) => setCodexOAuthCode(e.target.value)}
                    required
                    style={{ resize: 'vertical', fontFamily: 'var(--font-mono)' }}
                  />
                  <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                    Dán URL chuyển hướng sau khi đăng nhập OpenAI để hoàn tất lưu tài khoản.
                  </span>
                </div>

                <div className="modal-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setShowCodexOAuthModal(false)}
                  >
                    Đóng
                  </button>
                  <button
                    type="submit"
                    className="btn btn-primary"
                    disabled={actionLoading || !codexOAuthCode.trim()}
                  >
                    {actionLoading ? <div className="spinner" /> : 'Hoàn Tất Ủy Quyền'}
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
                {editingProvider ? 'Cập Nhật Nhà Cung Cấp' : 'Thêm Nhà Cung Cấp Upstream'}
              </h3>
              <button
                className="btn btn-secondary btn-sm btn-icon-only"
                onClick={() => setShowAddProviderModal(false)}
              >
                <IconX size={16} />
              </button>
            </div>

            <form onSubmit={handleSaveUpstream}>
              <div className="form-group">
                <label>Tên Nhà Cung Cấp</label>
                <input
                  type="text"
                  placeholder="Ví dụ: DeepSeek Official"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                />
              </div>

              <div className="form-group">
                <label>Tiền Tố Routing (Prefix)</label>
                <input
                  type="text"
                  placeholder="Ví dụ: deepseek"
                  value={prefix}
                  onChange={(e) => setPrefix(e.target.value)}
                  disabled={!!editingProvider}
                  required
                />
                <span style={{ fontSize: 11.5, color: 'var(--text-muted)' }}>
                  Định tuyến model: <code>{prefix || 'prefix'}/*</code>
                </span>
              </div>

              <div className="form-group">
                <label>Loại Giao Thức (Provider Type)</label>
                <select
                  value={providerType}
                  onChange={(e) => setProviderType(e.target.value)}
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
                <label>Base URL Upstream</label>
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
                <label>Khóa API Upstream</label>
                <input
                  type="text"
                  placeholder="sk-..."
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
                  <label style={{ marginBottom: 0 }}>Danh Sách Models (tự tải hoặc nhập tay)</label>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => { void handleFetchProviderModels(true); }}
                    disabled={modelsLoading || !baseUrl.trim()}
                  >
                    {modelsLoading ? <div className="spinner" /> : <IconRefresh size={14} />}
                    <span>{modelsLoading ? 'Đang tải...' : 'Tải models'}</span>
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
                  Kích hoạt nhà cung cấp này
                </label>
              </div>

              <div className="modal-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowAddProviderModal(false)}
                >
                  Hủy
                </button>
                <button type="submit" className="btn btn-primary" disabled={actionLoading}>
                  {actionLoading ? <div className="spinner" /> : 'Lưu Nhà Cung Cấp'}
                </button>
              </div>
            </form>
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
                  <span>Làm Mới Quota Tài Khoản</span>
                </button>
              )}
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setSelectedQuota(null)}
              >
                Đóng
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
