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
  quota_count?: number;
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
  quota_exhausted?: number;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  avg_duration_ms: number;
}

export interface TokenPeriodStats {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  requests: number;
  error_count: number;
}

export interface TokenPeriodsSummary {
  today: TokenPeriodStats;
  yesterday: TokenPeriodStats;
  last_3_days: TokenPeriodStats;
  last_7_days: TokenPeriodStats;
  last_30_days: TokenPeriodStats;
  all_time: TokenPeriodStats;
}

export interface DailyTokenStat {
  date: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  requests: number;
  error_count: number;
  avg_duration_ms: number;
}

export interface TokenAnalyticsSummary {
  total_tokens: number;
  prompt_tokens: number;
  completion_tokens: number;
  total_requests: number;
  error_count: number;
  peak_day: string | null;
  peak_tokens: number;
  avg_tokens_per_day: number;
}

export interface TokenAnalyticsResponse {
  period: string;
  periods: TokenPeriodsSummary;
  daily: DailyTokenStat[];
  summary: TokenAnalyticsSummary;
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
  ttft_ms?: number;
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
