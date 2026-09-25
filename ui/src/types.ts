// TypeScript definitions matching ag-proxy-rust backend API contract

export interface HealthInfo {
  status: string;
  service: string;
  mode: string;
  port: number;
  data_dir: string;
}

export interface AdminStats {
  total_requests: number;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  avg_duration_ms: number;
  error_count: number;
  total_accounts: number;
  active_accounts: number;
  cooldown_accounts: number;
  active_rate: number;
  error_rate: number;
  rpm: number;
}

export interface ModelRequestSummary {
  model: string;
  requests: number;
  ok: number;
  errors: number;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  avg_duration_ms: number;
}

export interface RequestLogItem {
  id: number;
  timestamp: number;
  model: string;
  account_id: string | null;
  status: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  duration_ms: number;
  error: string | null;
}

export interface RequestsResponse {
  items: RequestLogItem[];
  total: number;
  limit: number;
  offset: number;
}

export interface ActiveRequestItem {
  id: string;
  client: string;
  requested_model: string;
  model: string;
  provider: string;
  account: string;
  status: string;
  stream: boolean;
  started_at: number;
  elapsed_ms: number;
  completed_at?: number;
}

export interface ActiveRequestsResponse {
  items: ActiveRequestItem[];
  server_time: number;
}

export interface ProviderResponse {
  id: string;
  name: string;
  prefix: string;
  type: string;
  base_url: string;
  api_key: string;
  models: any;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateProviderRequest {
  name: string;
  prefix: string;
  type: string;
  base_url: string;
  api_key: string;
  models?: any;
  is_active?: boolean;
}

export interface UpdateProviderRequest {
  name?: string;
  prefix?: string;
  type?: string;
  base_url?: string;
  api_key?: string;
  models?: any;
  is_active?: boolean;
}

export interface ComboRecord {
  id: string;
  name: string;
  models: string[] | any;
  strategy: string;
  created_at: string;
  updated_at: string;
}

export interface CreateComboRequest {
  id?: string;
  name: string;
  models: string[];
  strategy?: string;
}

export interface AccountQuotaWindow {
  used_percent?: number;
  limit_window_seconds?: number;
  reset_after_seconds?: number;
  reset_at?: number;
}

export interface AccountQuota {
  primary_window?: AccountQuotaWindow;
  weekly_window?: AccountQuotaWindow;
  [key: string]: any;
}

export interface AccountResponse {
  id: string;
  email: string;
  is_active: boolean;
  cooldown_remaining: number;
  last_used_ago: number | null;
  total_requests: number;
  error_count: number;
  last_error: string | null;
  token_valid: boolean;
  recent_rpm: number;
  quota: any;
}

export interface ListAccountsResponse {
  accounts: AccountResponse[];
  total: number;
}

export interface CreateAccountRequest {
  email?: string;
  refresh_token?: string;
}

export interface CodexAccountRecord {
  id: string;
  email: string | null;
  auth_path?: string;
  account_id_suffix?: string;
  is_active?: boolean;
  active?: boolean;
  cooldown_remaining?: number;
  total_requests?: number;
  recent_rpm?: number;
  quota?: any;
  created_at?: string;
  updated_at?: string;
  last_error?: string | null;
}

export interface CodexAccountsResponse {
  accounts: CodexAccountRecord[];
  total?: number;
  rotation_enabled?: boolean;
}

export interface CodexStatusResponse {
  status: string;
  total_accounts: number;
  active_accounts: number;
  cooldown_accounts: number;
  rpm: number;
  accounts?: CodexAccountRecord[];
}

export interface QuotaRefreshSummary {
  google_total: number;
  google_refreshed: number;
  google_skipped_cooldown: number;
  google_skipped_inactive: number;
  codex_total: number;
  codex_refreshed: number;
  codex_skipped_cooldown: number;
  codex_skipped_inactive: number;
  errors: number;
  duration_ms: number;
}

export interface QuotaRefreshStatus {
  ok: boolean;
  enabled: boolean;
  interval_secs: number;
  last_refresh: number | null;
  next_refresh: number | null;
  last_error: string | null;
  is_refreshing: boolean;
  last_summary?: QuotaRefreshSummary;
}

export interface UpdateQuotaRefreshPayload {
  enabled?: boolean;
  interval_secs?: number;
}

export interface ApiKey {
  id: string;
  name: string;
  key: string;
  is_active: boolean;
  total_requests: number;
  created_at: string;
}

export interface CreateApiKeyRequest {
  name: string;
  key?: string;
}

export interface ModelEntry {
  id: string;
  object: string;
  created: number;
  owned_by: string;
}

export interface ModelListResponse {
  object: string;
  data: ModelEntry[];
}

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export interface ChatCompletionRequest {
  model: string;
  messages: ChatMessage[];
  stream?: boolean;
  temperature?: number;
  max_tokens?: number;
}

export type TokenSaverLevel = 'off' | 'lite' | 'full' | 'ultra';

export interface TokenSaverSettings {
  token_saver_enabled: boolean;
  rtk_enabled: boolean;
  caveman_level: TokenSaverLevel;
  ponytail_level: TokenSaverLevel;
}

export interface SystemLogEntry {
  id: number;
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

export interface SystemLogsResponse {
  logs: SystemLogEntry[];
  total: number;
}
