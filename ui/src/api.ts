import {
  ActiveRequestsResponse,
  AdminStats,
  ApiKey,
  ComboRecord,
  CreateApiKeyRequest,
  CreateComboRequest,
  CreateProviderRequest,
  HealthInfo,
  ModelListResponse,
  ModelRequestSummary,
  ProviderResponse,
  RequestsResponse,
  TokenAnalyticsResponse,
  UpdateProviderRequest,
  type TokenSaverSettings,
  SystemLogsResponse,
} from './types';

const STORAGE_KEY = 'ag_proxy_admin_key';

export function getStoredAdminKey(): string {
  return localStorage.getItem(STORAGE_KEY) || '';
}

export function setStoredAdminKey(key: string): void {
  localStorage.setItem(STORAGE_KEY, key.trim());
}

export function clearStoredAdminKey(): void {
  localStorage.removeItem(STORAGE_KEY);
}

class ApiError extends Error {
  status: number;
  data: any;

  constructor(message: string, status: number, data?: any) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.data = data;
  }
}

async function request<T>(
  path: string,
  options: RequestInit = {},
  customKey?: string
): Promise<T> {
  const token = customKey !== undefined ? customKey : getStoredAdminKey();
  const headers = new Headers(options.headers || {});

  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }
  if (!headers.has('Content-Type') && options.body && typeof options.body === 'string') {
    headers.set('Content-Type', 'application/json');
  }

  const res = await fetch(path, {
    ...options,
    headers,
  });

  if (!res.ok) {
    let errMsg = `Lỗi máy chủ (${res.status})`;
    let errData: any = null;
    try {
      errData = await res.json();
      if (errData?.error?.message) {
        errMsg = errData.error.message;
      } else if (errData?.error) {
        errMsg = typeof errData.error === 'string' ? errData.error : JSON.stringify(errData.error);
      } else if (errData?.message) {
        errMsg = errData.message;
      }
    } catch {
      try {
        const text = await res.text();
        if (text) errMsg = text;
      } catch {
        // use default errMsg
      }
    }
    throw new ApiError(errMsg, res.status, errData);
  }

  // Check content-type for json
  const contentType = res.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    return (await res.json()) as T;
  }
  return (await res.text()) as unknown as T;
}

export const api = {
  // Health
  getHealth: () => request<HealthInfo>('/health'),

  // Stats & Requests
  getStats: (key?: string) => request<AdminStats>('/admin/stats', {}, key),
  getRequestSummary: () => request<ModelRequestSummary[]>('/admin/request-summary'),
  getTokenAnalytics: (period?: string) =>
    request<TokenAnalyticsResponse>(
      `/admin/token-analytics${period ? `?period=${encodeURIComponent(period)}` : ''}`
    ),
  getRequests: (params: { limit?: number; offset?: number; model?: string; status?: string }) => {
    const q = new URLSearchParams();
    if (params.limit !== undefined) q.set('limit', String(params.limit));
    if (params.offset !== undefined) q.set('offset', String(params.offset));
    if (params.model) q.set('model', params.model);
    if (params.status) q.set('status', params.status);
    return request<RequestsResponse>(`/admin/requests?${q.toString()}`);
  },
  getActiveRequests: () => request<ActiveRequestsResponse>('/admin/active-requests'),
  getActiveRequestsStream: (
    onMessage: (data: ActiveRequestsResponse) => void,
    onError?: (err: any) => void
  ) => {
    const token = getStoredAdminKey();
    const url = `/admin/active-requests/stream${token ? `?token=${encodeURIComponent(token)}` : ''}`;
    const es = new EventSource(url);
    es.onmessage = (event) => {
      try {
        const parsed: ActiveRequestsResponse = JSON.parse(event.data);
        onMessage(parsed);
      } catch (e) {
        if (onError) onError(e);
      }
    };
    es.onerror = (err) => {
      if (onError) onError(err);
    };
    return () => es.close();
  },

  // Providers
  getProviders: () => request<ProviderResponse[]>('/admin/providers'),
  createProvider: (data: CreateProviderRequest) =>
    request<ProviderResponse>('/admin/providers', {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  updateProvider: (id: string, data: UpdateProviderRequest) =>
    request<ProviderResponse>(`/admin/providers/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),
  deleteProvider: (id: string) =>
    request<{ status: string; id: string }>(`/admin/providers/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    }),
  testProvider: (id: string) =>
    request<any>(`/admin/providers/${encodeURIComponent(id)}/test`, {
      method: 'POST',
    }),
  fetchProviderModels: (data: {
    provider_id?: string;
    type?: string;
    base_url?: string;
    api_key?: string;
  }) =>
    request<{ status: string; models: string[] }>('/admin/providers/fetch-models', {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  syncModels: (id: string) =>
    request<any>(`/admin/providers/${encodeURIComponent(id)}/sync-models`, {
      method: 'POST',
    }),

  // Combos
  getCombos: () => request<ComboRecord[]>('/admin/combos'),
  createCombo: (data: CreateComboRequest) =>
    request<ComboRecord>('/admin/combos', {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  updateCombo: (id: string, data: Partial<CreateComboRequest>) =>
    request<ComboRecord>(`/admin/combos/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),
  deleteCombo: (id: string) =>
    request<{ status: string; id: string }>(`/admin/combos/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    }),

  // API Keys
  getApiKeys: () => request<ApiKey[]>('/admin/api-keys'),
  createApiKey: (data: CreateApiKeyRequest) =>
    request<ApiKey>('/admin/api-keys', {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  updateApiKey: (id: string, isActive: boolean) =>
    request<ApiKey>(`/admin/api-keys/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      body: JSON.stringify({ is_active: isActive }),
    }),
  deleteApiKey: (id: string) =>
    request<{ status: string; id: string }>(`/admin/api-keys/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    }),

  // Models
  getModels: () => request<ModelListResponse>('/v1/models'),

  // Token Saver Settings
  getSettings: (customKey?: string) =>
    request<TokenSaverSettings>('/admin/settings', {}, customKey),
  updateSettings: (settings: Partial<TokenSaverSettings>, customKey?: string) =>
    request<TokenSaverSettings & { ok: boolean }>(
      '/admin/settings',
      {
        method: 'POST',
        body: JSON.stringify(settings),
      },
      customKey
    ),

  // System Logs
  getSystemLogs: (params?: { limit?: number; level?: string; search?: string }) => {
    const q = new URLSearchParams();
    if (params?.limit) q.set('limit', String(params.limit));
    if (params?.level && params.level !== 'all') q.set('level', params.level);
    if (params?.search) q.set('search', params.search);
    const qs = q.toString();
    return request<SystemLogsResponse>(`/admin/system-logs${qs ? `?${qs}` : ''}`);
  },
  clearSystemLogs: () =>
    request<{ success: boolean }>('/admin/system-logs', { method: 'DELETE' }),
};
