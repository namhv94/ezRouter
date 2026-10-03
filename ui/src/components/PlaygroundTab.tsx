import React, { useState, useEffect, useRef } from 'react';
import { ModelEntry } from '../types';
import { api, getStoredAdminKey } from '../api';
import { useI18n } from '../i18n';
import {
  IconPlayground,
  IconZap,
  IconRefresh,
  IconCheck,
  IconX,
  IconAlertCircle,
  IconCopy,
  IconTrash,
} from '../icons';

// Pre-seeded list of known Antigravity & OpenAI Codex models
const DEFAULT_AG_MODELS = [
  'ag/gemini-3.8-flash-high',
  'ag/gemini-3.8-flash-low',
  'ag/gemini-3.7-flash-tiered',
  'ag/gemini-3.7-flash-low',
  'ag/gemini-pro-agent',
  'ag/gemini-3.1-pro-low',
  'ag/claude-sonnet-4-6',
  'ag/claude-opus-4-6-thinking',
];

const DEFAULT_CX_MODELS = [
  'cx/gpt-6.1-sol',
  'cx/gpt-6-sol',
  'cx/gpt-6-luna',
  'cx/gpt-6-astra',
  'cx/gpt-5.6-sol',
  'cx/gpt-5.6-terra',
  'cx/gpt-5.6-luna',
  'cx/gpt-reserve',
  'cx/gpt-5.5',
];

interface PlaygroundTabProps {
  adminKey?: string;
}

export const PlaygroundTab: React.FC<PlaygroundTabProps> = ({ adminKey }) => {
  const { t, locale } = useI18n();

  // Model state
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>(DEFAULT_AG_MODELS[0]);
  const [customModel, setCustomModel] = useState<string>('');
  const [isCustomModel, setIsCustomModel] = useState<boolean>(false);
  const [loadingModels, setLoadingModels] = useState<boolean>(false);

  // Auth key state
  const defaultKey = adminKey || getStoredAdminKey();
  const [authKey, setAuthKey] = useState<string>(defaultKey);
  const [showAuthKey, setShowAuthKey] = useState<boolean>(false);

  // Mode & prompts
  const [isStream, setIsStream] = useState<boolean>(true);
  const [systemPrompt, setSystemPrompt] = useState<string>(
    t('playground.defaultSystemPrompt')
  );
  const [userPrompt, setUserPrompt] = useState<string>(
    t('playground.defaultUserPrompt')
  );

  // Request execution state
  const [loading, setLoading] = useState<boolean>(false);
  const [streaming, setStreaming] = useState<boolean>(false);
  const [responseText, setResponseText] = useState<string>('');
  const [rawResponse, setRawResponse] = useState<any>(null);
  const [responseStatus, setResponseStatus] = useState<number | null>(null);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [usage, setUsage] = useState<{
    prompt_tokens?: number;
    completion_tokens?: number;
    total_tokens?: number;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [modelUsed, setModelUsed] = useState<string>('');

  // UI helpers
  const [viewMode, setViewMode] = useState<'formatted' | 'raw'>('formatted');
  const [copied, setCopied] = useState<boolean>(false);

  const abortControllerRef = useRef<AbortController | null>(null);

  // Load models on mount
  useEffect(() => {
    let isMounted = true;
    const fetchModels = async () => {
      setLoadingModels(true);
      try {
        const res = await api.getModels();
        if (isMounted && res?.data && Array.isArray(res.data)) {
          const fetchedIds = res.data.map((m: ModelEntry) => m.id);
          const allModels = Array.from(
            new Set([...DEFAULT_AG_MODELS, ...DEFAULT_CX_MODELS, ...fetchedIds])
          );
          setModels(allModels);
        }
      } catch {
        // Fallback to default models
        if (isMounted) {
          setModels([...DEFAULT_AG_MODELS, ...DEFAULT_CX_MODELS]);
        }
      } finally {
        if (isMounted) setLoadingModels(false);
      }
    };

    fetchModels();
    return () => {
      isMounted = false;
    };
  }, []);

  // Update auth key if adminKey prop changes
  useEffect(() => {
    if (adminKey && !authKey) {
      setAuthKey(adminKey);
    }
  }, [adminKey, authKey]);

  // Clean up abort controller on unmount
  useEffect(() => {
    return () => {
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
      }
    };
  }, []);

  const activeModel = isCustomModel ? customModel.trim() : selectedModel;

  // Preset prompt handlers
  const applyPreset = (text: string) => {
    setUserPrompt(text);
  };

  const handleClearOutput = () => {
    setResponseText('');
    setRawResponse(null);
    setResponseStatus(null);
    setDurationMs(null);
    setUsage(null);
    setError(null);
    setModelUsed('');
  };

  const handleCopy = () => {
    const textToCopy =
      viewMode === 'raw' && rawResponse
        ? JSON.stringify(rawResponse, null, 2)
        : responseText;
    if (!textToCopy) return;

    navigator.clipboard.writeText(textToCopy).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const handleCancelRequest = () => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
    }
    setLoading(false);
    setStreaming(false);
  };

  const handleSend = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeModel) {
      setError(t('playground.errSelectModel'));
      return;
    }
    if (!userPrompt.trim()) {
      setError(t('playground.errEnterPrompt'));
      return;
    }

    // Reset previous run
    setError(null);
    setResponseText('');
    setRawResponse(null);
    setResponseStatus(null);
    setDurationMs(null);
    setUsage(null);
    setModelUsed(activeModel);
    setLoading(true);
    setStreaming(isStream);

    const controller = new AbortController();
    abortControllerRef.current = controller;

    const messages = [];
    if (systemPrompt.trim()) {
      messages.push({ role: 'system' as const, content: systemPrompt.trim() });
    }
    messages.push({ role: 'user' as const, content: userPrompt.trim() });

    const token = authKey.trim() || defaultKey;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

    const payload = {
      model: activeModel,
      messages,
      stream: isStream,
    };

    const startTime = performance.now();

    try {
      const res = await fetch('/v1/chat/completions', {
        method: 'POST',
        headers,
        body: JSON.stringify(payload),
        signal: controller.signal,
      });

      setResponseStatus(res.status);

      if (!res.ok) {
        let errMsg = t('playground.errServerStatus', { status: res.status });
        let errJson: any = null;
        try {
          errJson = await res.json();
          if (errJson?.error?.message) {
            errMsg = errJson.error.message;
          } else if (errJson?.detail) {
            errMsg = typeof errJson.detail === 'string' ? errJson.detail : JSON.stringify(errJson.detail);
          } else if (errJson?.error) {
            errMsg = typeof errJson.error === 'string' ? errJson.error : JSON.stringify(errJson.error);
          } else if (errJson?.message) {
            errMsg = errJson.message;
          }
        } catch {
          try {
            const rawTxt = await res.text();
            if (rawTxt) errMsg = rawTxt;
          } catch {
            // keep default errMsg
          }
        }

        setError(errMsg);
        setRawResponse(errJson || { status: res.status, message: errMsg });
        setDurationMs(Math.round(performance.now() - startTime));
        setLoading(false);
        setStreaming(false);
        return;
      }

      // Handle non-streaming response
      if (!isStream) {
        const data = await res.json();
        const duration = Math.round(performance.now() - startTime);
        setDurationMs(duration);
        setRawResponse(data);
        const text = data?.choices?.[0]?.message?.content || '';
        setResponseText(text);
        if (data?.usage) {
          setUsage(data.usage);
        }
        setLoading(false);
        setStreaming(false);
        return;
      }

      // Handle streaming response (SSE)
      if (!res.body) {
        throw new Error(t('playground.errNoStream'));
      }

      const reader = res.body.getReader();
      const decoder = new TextDecoder('utf-8');
      let fullContent = '';
      let buffer = '';
      const rawChunks: any[] = [];
      let streamUsage: any = null;

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed || trimmed.startsWith(':')) continue;
          if (trimmed === 'data: [DONE]') continue;
          if (trimmed.startsWith('data:')) {
            const jsonText = trimmed.slice(5).trim();
            if (!jsonText) continue;
            try {
              const parsed = JSON.parse(jsonText);
              rawChunks.push(parsed);
              const delta = parsed?.choices?.[0]?.delta?.content || '';
              if (delta) {
                fullContent += delta;
                setResponseText(fullContent);
              }
              if (parsed?.usage) {
                streamUsage = parsed.usage;
                setUsage(streamUsage);
              }
            } catch {
              // Incomplete chunk, keep going
            }
          }
        }
      }

      const duration = Math.round(performance.now() - startTime);
      setDurationMs(duration);
      setRawResponse(rawChunks);
      setLoading(false);
      setStreaming(false);
    } catch (err: any) {
      if (err.name === 'AbortError') {
        setError(t('playground.errAborted'));
      } else {
        setError(err?.message || t('playground.errSend'));
      }
      setDurationMs(Math.round(performance.now() - startTime));
      setLoading(false);
      setStreaming(false);
    } finally {
      abortControllerRef.current = null;
    }
  };

  // Group models for select
  const allKnownModels = models.length > 0 ? models : [...DEFAULT_AG_MODELS, ...DEFAULT_CX_MODELS];
  const agModels = allKnownModels.filter((m) => m.startsWith('ag/'));
  const cxModels = allKnownModels.filter((m) => m.startsWith('cx/'));
  const otherModels = allKnownModels.filter(
    (m) => !m.startsWith('ag/') && !m.startsWith('cx/')
  );

  return (
    <div className="tab-content" style={{ width: '100%', maxWidth: '100%', minWidth: 0 }}>
      {/* Top Banner / Section Header */}
      <div className="section-header">
        <div>
          <h2 className="section-title" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <IconPlayground size={22} style={{ color: 'var(--accent-primary)' }} />
            {t('playground.title')}
          </h2>
          <p className="section-desc">
            {locale === 'vi' ? (
              <>
                Kiểm thử chat completions trực tiếp tới các mô hình Antigravity (<code>ag/*</code>),
                OpenAI Codex (<code>cx/*</code>) và Upstream qua cổng <code>/v1/chat/completions</code>.
              </>
            ) : (
              <>
                Test chat completions directly against Antigravity (<code>ag/*</code>),
                OpenAI Codex (<code>cx/*</code>), and Upstream models via <code>/v1/chat/completions</code>.
              </>
            )}
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={() => {
            setLoadingModels(true);
            api
              .getModels()
              .then((res) => {
                if (res?.data) {
                  const ids = res.data.map((m: ModelEntry) => m.id);
                  setModels(Array.from(new Set([...DEFAULT_AG_MODELS, ...DEFAULT_CX_MODELS, ...ids])));
                }
              })
              .catch(() => {})
              .finally(() => setLoadingModels(false));
          }}
          disabled={loadingModels}
          title={t('playground.refreshModelsTitle')}
        >
          <IconRefresh size={14} className={loadingModels ? 'spinner' : ''} />
          <span>{t('playground.refreshModels')}</span>
        </button>
      </div>

      {/* Main 2-Column Responsive Layout */}
      <div className="playground-grid">
        {/* Left Column: Request Configuration & Inputs */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: 10 }}>
            <h3 style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)' }}>
              {t('playground.configTitle')}
            </h3>
            <span className="badge badge-neutral" style={{ fontSize: 11 }}>
              POST /v1/chat/completions
            </span>
          </div>

          <form onSubmit={handleSend} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            {/* Model Selection */}
            <div className="form-group" style={{ marginBottom: 0 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                <label htmlFor="model-select">{t('playground.modelLabel')}</label>
                <button
                  type="button"
                  style={{
                    background: 'none',
                    border: 'none',
                    color: 'var(--accent-primary)',
                    fontSize: 12,
                    cursor: 'pointer',
                    padding: 0,
                  }}
                  onClick={() => setIsCustomModel(!isCustomModel)}
                >
                  {isCustomModel ? t('playground.selectFromList') : t('playground.customModelInput')}
                </button>
              </div>

              {!isCustomModel ? (
                <select
                  id="model-select"
                  value={selectedModel}
                  onChange={(e) => setSelectedModel(e.target.value)}
                  disabled={loading}
                >
                  <optgroup label={t('playground.groupAg')}>
                    {agModels.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </optgroup>
                  <optgroup label={t('playground.groupCx')}>
                    {cxModels.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </optgroup>
                  {otherModels.length > 0 && (
                    <optgroup label={t('playground.groupOther')}>
                      {otherModels.map((m) => (
                        <option key={m} value={m}>
                          {m}
                        </option>
                      ))}
                    </optgroup>
                  )}
                </select>
              ) : (
                <input
                  type="text"
                  placeholder={t('playground.customModelPlaceholder')}
                  value={customModel}
                  onChange={(e) => setCustomModel(e.target.value)}
                  disabled={loading}
                  autoFocus
                />
              )}
            </div>

            {/* Auth Key Input */}
            <div className="form-group" style={{ marginBottom: 0 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                <label htmlFor="auth-key-input">{t('playground.authKeyLabel')}</label>
                <button
                  type="button"
                  style={{
                    background: 'none',
                    border: 'none',
                    color: 'var(--text-secondary)',
                    fontSize: 12,
                    cursor: 'pointer',
                    padding: 0,
                  }}
                  onClick={() => setShowAuthKey(!showAuthKey)}
                >
                  {showAuthKey ? t('playground.hideKey') : t('playground.showKey')}
                </button>
              </div>
              <input
                id="auth-key-input"
                type={showAuthKey ? 'text' : 'password'}
                placeholder={t('playground.authKeyPlaceholder')}
                value={authKey}
                onChange={(e) => setAuthKey(e.target.value)}
                disabled={loading}
              />
              <span style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>
                {t('playground.authKeyHelp')}
              </span>
            </div>

            {/* Stream Mode Toggle */}
            <div className="form-group" style={{ marginBottom: 0 }}>
              <label>{t('playground.responseModeLabel')}</label>
              <div className="stream-toggle-group">
                <button
                  type="button"
                  className={`stream-toggle-btn ${isStream ? 'active' : ''}`}
                  onClick={() => setIsStream(true)}
                  disabled={loading}
                >
                  {t('playground.streamSse')}
                </button>
                <button
                  type="button"
                  className={`stream-toggle-btn ${!isStream ? 'active' : ''}`}
                  onClick={() => setIsStream(false)}
                  disabled={loading}
                >
                  {t('playground.nonStreamJson')}
                </button>
              </div>
            </div>

            {/* Preset prompts */}
            <div>
              <label style={{ display: 'block', marginBottom: 4 }}>{t('playground.quickPresetsLabel')}</label>
              <div className="playground-presets">
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => applyPreset(t('playground.presetIntroText'))}
                  disabled={loading}
                >
                  {t('playground.presetIntro')}
                </button>
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => applyPreset(t('playground.presetQuickTestText'))}
                  disabled={loading}
                >
                  {t('playground.presetQuickTest')}
                </button>
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => applyPreset(t('playground.presetPythonText'))}
                  disabled={loading}
                >
                  {t('playground.presetPython')}
                </button>
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => applyPreset(t('playground.presetTranslateText'))}
                  disabled={loading}
                >
                  {t('playground.presetTranslate')}
                </button>
              </div>
            </div>

            {/* Collapsible System Prompt */}
            <details style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
              <summary style={{ cursor: 'pointer', padding: '4px 0', fontWeight: 500, color: 'var(--accent-primary)' }}>
                {t('playground.systemPromptToggle')}
              </summary>
              <div style={{ marginTop: 8 }}>
                <textarea
                  rows={2}
                  value={systemPrompt}
                  onChange={(e) => setSystemPrompt(e.target.value)}
                  placeholder={t('playground.systemPromptPlaceholder')}
                  disabled={loading}
                />
              </div>
            </details>

            {/* User Prompt Textarea */}
            <div className="form-group" style={{ marginBottom: 0 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                <label htmlFor="user-prompt">{t('playground.userPromptLabel')}</label>
                {userPrompt && (
                  <button
                    type="button"
                    style={{
                      background: 'none',
                      border: 'none',
                      color: 'var(--text-muted)',
                      fontSize: 12,
                      cursor: 'pointer',
                      padding: 0,
                    }}
                    onClick={() => setUserPrompt('')}
                    disabled={loading}
                  >
                    {t('playground.clearPrompt')}
                  </button>
                )}
              </div>
              <textarea
                id="user-prompt"
                rows={5}
                value={userPrompt}
                onChange={(e) => setUserPrompt(e.target.value)}
                placeholder={t('playground.userPromptPlaceholder')}
                disabled={loading}
              />
            </div>

            {/* Action Buttons */}
            <div style={{ display: 'flex', gap: 10, marginTop: 4 }}>
              <button
                type="submit"
                className="btn btn-primary"
                style={{ flex: 1 }}
                disabled={loading || !userPrompt.trim() || (!selectedModel && !customModel.trim())}
              >
                {loading ? (
                  <>
                    <div className="spinner" style={{ width: 16, height: 16, borderWidth: 2 }} />
                    <span>{streaming ? t('playground.receivingStream') : t('playground.sending')}</span>
                  </>
                ) : (
                  <>
                    <IconZap size={16} />
                    <span>{t('playground.sendRequest')}</span>
                  </>
                )}
              </button>

              {loading && (
                <button
                  type="button"
                  className="btn btn-danger"
                  onClick={handleCancelRequest}
                >
                  <IconX size={16} />
                  <span>{t('playground.cancel')}</span>
                </button>
              )}
            </div>
          </form>
        </div>

        {/* Right Column: Output & Response Panel */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* Header Bar of Output Card */}
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--border-subtle)', paddingBottom: 10, flexWrap: 'wrap', gap: 8 }}>
            <h3 style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)' }}>
              {t('playground.responseTitle')}
            </h3>

            {/* Actions for output */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              {responseText && (
                <>
                  <div style={{ display: 'inline-flex', borderRadius: 4, background: 'var(--bg-input)', border: '1px solid var(--border-subtle)', overflow: 'hidden' }}>
                    <button
                      type="button"
                      style={{
                        padding: '4px 10px',
                        fontSize: 12,
                        border: 'none',
                        background: viewMode === 'formatted' ? 'var(--accent-primary)' : 'transparent',
                        color: viewMode === 'formatted' ? '#fff' : 'var(--text-secondary)',
                        cursor: 'pointer',
                        minHeight: 32,
                      }}
                      onClick={() => setViewMode('formatted')}
                    >
                      {t('playground.viewFormatted')}
                    </button>
                    <button
                      type="button"
                      style={{
                        padding: '4px 10px',
                        fontSize: 12,
                        border: 'none',
                        background: viewMode === 'raw' ? 'var(--accent-primary)' : 'transparent',
                        color: viewMode === 'raw' ? '#fff' : 'var(--text-secondary)',
                        cursor: 'pointer',
                        minHeight: 32,
                      }}
                      onClick={() => setViewMode('raw')}
                    >
                      {t('playground.viewRaw')}
                    </button>
                  </div>

                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleCopy}
                    title={t('playground.copyResponseTitle')}
                    style={{ minHeight: 32, padding: '4px 8px' }}
                  >
                    {copied ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                    <span>{copied ? t('playground.copied') : t('playground.copy')}</span>
                  </button>

                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleClearOutput}
                    title={t('playground.clearOutputTitle')}
                    style={{ minHeight: 32, padding: '4px 8px' }}
                  >
                    <IconTrash size={14} />
                  </button>
                </>
              )}
            </div>
          </div>

          {/* Metadata Badges */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap', minHeight: 26 }}>
            {loading ? (
              <span className="badge badge-warning" style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                <span className="spinner" style={{ width: 10, height: 10, borderWidth: 1.5 }} />
                {streaming ? t('playground.statusStreaming') : t('playground.statusProcessing')}
              </span>
            ) : responseStatus !== null ? (
              <span className={`badge ${responseStatus >= 200 && responseStatus < 300 ? 'badge-success' : 'badge-error'}`}>
                {responseStatus} {responseStatus === 200 ? 'OK' : 'Error'}
              </span>
            ) : (
              <span className="badge badge-neutral">{t('playground.statusNoData')}</span>
            )}

            {modelUsed && (
              <span className="badge badge-neutral" style={{ fontFamily: 'var(--font-mono)' }}>
                {modelUsed}
              </span>
            )}

            {durationMs !== null && (
              <span className="badge badge-neutral">
                {durationMs} ms
              </span>
            )}

            {usage && (
              <span className="badge badge-neutral" title={t('playground.tokensDetail', { prompt: usage.prompt_tokens || 0, completion: usage.completion_tokens || 0 })}>
                {t('playground.tokensCount', { count: usage.total_tokens || (usage.prompt_tokens || 0) + (usage.completion_tokens || 0) })}
              </span>
            )}

            {responseStatus !== null && (
              <span className="badge badge-neutral">
                {isStream ? 'Stream SSE' : 'Non-stream'}
              </span>
            )}
          </div>

          {/* Error Banner */}
          {error && (
            <div className="alert alert-error" style={{ marginBottom: 0 }}>
              <IconAlertCircle size={18} style={{ flexShrink: 0 }} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontWeight: 600 }}>{t('playground.errorTitle')}</div>
                <div style={{ marginTop: 2, wordBreak: 'break-word' }}>{error}</div>
                {error.includes('Upstream base URL is not configured') && (
                  <div style={{ marginTop: 6, fontSize: 12, opacity: 0.9 }}>
                    {t('playground.errUpstreamHint')}{' '}
                    <strong>{t('playground.errUpstreamHintProviderTab')}</strong>.
                  </div>
                )}
                {error.includes('Invalid API key') && (
                  <div style={{ marginTop: 6, fontSize: 12, opacity: 0.9 }}>
                    {t('playground.errInvalidKeyHint')}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Response Output Container */}
          {loading && !responseText && !error ? (
            <div className="state-container" style={{ minHeight: 280, backgroundColor: 'var(--bg-app)', borderRadius: 'var(--radius-md)', border: '1px solid var(--border-subtle)' }}>
              <div className="spinner" style={{ width: 28, height: 28 }} />
              <p style={{ marginTop: 12, fontSize: 13 }}>
                {t('playground.connectingModel', { model: activeModel })}
              </p>
              <p style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                {t('playground.waitingResponse')}
              </p>
            </div>
          ) : !responseText && !error ? (
            <div className="state-container" style={{ minHeight: 280, backgroundColor: 'var(--bg-app)', borderRadius: 'var(--radius-md)', border: '1px solid var(--border-subtle)' }}>
              <IconPlayground size={36} style={{ color: 'var(--text-muted)', opacity: 0.6 }} />
              <p style={{ fontWeight: 500, color: 'var(--text-primary)', marginTop: 8 }}>
                {t('playground.readyTitle')}
              </p>
              <p style={{ fontSize: 12, maxWidth: 360 }}>
                {locale === 'vi' ? (
                  <>
                    Chọn mô hình <code>ag/*</code> hoặc <code>cx/*</code>, nhập câu hỏi và bấm nút <strong>Gửi Yêu Cầu</strong> để kiểm thử phản hồi thời gian thực.
                  </>
                ) : (
                  <>
                    Select an <code>ag/*</code> or <code>cx/*</code> model, enter your question, and click <strong>Send Request</strong> to test real-time responses.
                  </>
                )}
              </p>
            </div>
          ) : viewMode === 'formatted' && responseText ? (
            <div className="playground-output-box">
              {responseText}
              {streaming && <span className="streaming-cursor" />}
            </div>
          ) : (
            <pre
              className="playground-output-box"
              style={{
                fontSize: 12.5,
                color: '#a5b4fc',
              }}
            >
              {rawResponse ? JSON.stringify(rawResponse, null, 2) : responseText}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
};

// Also export as ChatTestTab for backwards-compatibility or alternate references
export const ChatTestTab = PlaygroundTab;
