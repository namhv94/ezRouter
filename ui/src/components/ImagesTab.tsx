import React, { useState, useEffect, useRef } from 'react';
import { useI18n } from '../i18n';
import { getStoredAdminKey } from '../api';
import {
  IconImage,
  IconCheck,
  IconCopy,
  IconTrash,
  IconAlertCircle,
  IconX,
  IconDownload,
  IconMaximize,
  IconCode,
} from '../icons';

const POPULAR_IMAGE_MODELS = [
  {
    id: 'openrouter/black-forest-labs/flux-1-schnell',
    name: 'FLUX.1 Schnell (Nhanh & Sắc nét)',
    desc: '4-step distilled latent model, siêu tốc, chất lượng cao',
  },
  {
    id: 'openrouter/black-forest-labs/flux-1-dev',
    name: 'FLUX.1 Dev (Chi tiết cao)',
    desc: '12B parameter guidance-distilled model cho độ chân thực tối đa',
  },
  {
    id: 'openrouter/stabilityai/stable-diffusion-3.5-large',
    name: 'Stable Diffusion 3.5 Large',
    desc: '8B parameter model từ Stability AI, hiểu prompt typography tốt',
  },
  {
    id: 'openrouter/google/imagen-3',
    name: 'Google Imagen 3',
    desc: 'Photorealistic generation từ Google DeepMind',
  },
];

interface GeneratedImageItem {
  url?: string;
  b64_json?: string;
  revised_prompt?: string;
}

interface ImageGenerationResponse {
  created?: number;
  data?: GeneratedImageItem[];
  error?: {
    message?: string;
    type?: string;
    code?: string | number;
  };
}

interface ImagesTabProps {
  adminKey?: string;
}

export const ImagesTab: React.FC<ImagesTabProps> = ({ adminKey }) => {
  const { t } = useI18n();

  // Model & Provider state
  const [selectedModel, setSelectedModel] = useState<string>(
    POPULAR_IMAGE_MODELS[0].id
  );
  const [customModel, setCustomModel] = useState<string>('');
  const [isCustomModel, setIsCustomModel] = useState<boolean>(false);
  const [providerModels, setProviderModels] = useState<string[]>([]);

  // Auth Key state
  const defaultKey = adminKey || getStoredAdminKey();
  const [authKey, setAuthKey] = useState<string>(defaultKey);
  const [showAuthKey, setShowAuthKey] = useState<boolean>(false);

  // Generation parameters
  const [prompt, setPrompt] = useState<string>('');
  const [imageSize, setImageSize] = useState<string>('1024x1024');
  const [imageCount, setImageCount] = useState<number>(1);
  const [responseFormat, setResponseFormat] = useState<string>('url');

  // Execution state
  const [loading, setLoading] = useState<boolean>(false);
  const [elapsedMs, setElapsedMs] = useState<number>(0);
  const timerRef = useRef<number | null>(null);

  // Results & Error
  const [results, setResults] = useState<GeneratedImageItem[]>([]);
  const [rawResponse, setRawResponse] = useState<string>('');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [errorStatus, setErrorStatus] = useState<number | null>(null);

  // UI helpers
  const [copiedUrlIndex, setCopiedUrlIndex] = useState<number | null>(null);
  const [copiedJson, setCopiedJson] = useState<boolean>(false);
  const [copiedCurl, setCopiedCurl] = useState<boolean>(false);
  const [lightboxUrl, setLightboxUrl] = useState<string | null>(null);
  const [showCurlModal, setShowCurlModal] = useState<boolean>(false);
  const [showRawJson, setShowRawJson] = useState<boolean>(false);

  const activeModel = isCustomModel
    ? customModel.trim() || POPULAR_IMAGE_MODELS[0].id
    : selectedModel;

  // Load configured providers on mount to discover image models
  useEffect(() => {
    let isMounted = true;
    const fetchProviders = async () => {
      try {
        const headers: Record<string, string> = {
          'Content-Type': 'application/json',
        };
        const key = authKey || defaultKey;
        if (key) {
          headers['Authorization'] = `Bearer ${key}`;
        }
        const res = await fetch('/admin/providers', { headers });
        if (res.ok) {
          const list = await res.json();
          if (Array.isArray(list) && isMounted) {
            const found: string[] = [];
            for (const p of list) {
              if (p && p.is_active && Array.isArray(p.models)) {
                for (const m of p.models) {
                  const mId = typeof m === 'string' ? m : m?.id;
                  if (mId && typeof mId === 'string') {
                    const lower = mId.toLowerCase();
                    if (
                      lower.includes('flux') ||
                      lower.includes('diffusion') ||
                      lower.includes('image') ||
                      lower.includes('dall-e') ||
                      lower.includes('midjourney')
                    ) {
                      const qualified = p.prefix ? `${p.prefix}/${mId}` : mId;
                      if (!found.includes(qualified)) {
                        found.push(qualified);
                      }
                    }
                  }
                }
              }
            }
            if (found.length > 0) {
              setProviderModels(found);
            }
          }
        }
      } catch {
        // Fallback silently if unauthenticated or endpoint unavailable
      }
    };
    fetchProviders();
    return () => {
      isMounted = false;
    };
  }, [authKey, defaultKey]);

  // Clean timer on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  const handleGenerate = async () => {
    if (!prompt.trim() || loading) return;

    setLoading(true);
    setErrorMessage(null);
    setErrorStatus(null);
    setResults([]);
    setRawResponse('');
    setElapsedMs(0);

    const startTime = Date.now();
    timerRef.current = window.setInterval(() => {
      setElapsedMs(Date.now() - startTime);
    }, 50);

    try {
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };
      const key = authKey.trim();
      if (key) {
        headers['Authorization'] = `Bearer ${key}`;
      }

      const payload = {
        model: activeModel,
        prompt: prompt.trim(),
        n: imageCount,
        size: imageSize,
        response_format: responseFormat,
      };

      const res = await fetch('/v1/images/generations', {
        method: 'POST',
        headers,
        body: JSON.stringify(payload),
      });

      const totalDuration = Date.now() - startTime;
      setElapsedMs(totalDuration);

      const data: ImageGenerationResponse = await res.json();
      setRawResponse(JSON.stringify(data, null, 2));

      if (!res.ok) {
        setErrorStatus(res.status);
        const msg =
          data?.error?.message ||
          (typeof data === 'string' ? data : JSON.stringify(data));
        setErrorMessage(msg || `HTTP ${res.status}`);
      } else if (Array.isArray(data.data) && data.data.length > 0) {
        setResults(data.data);
      } else {
        setErrorMessage(t('images.noImagesReturned'));
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMessage(msg);
    } finally {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
      setLoading(false);
    }
  };

  const handleCopyUrl = (url: string, index: number) => {
    navigator.clipboard.writeText(url);
    setCopiedUrlIndex(index);
    setTimeout(() => setCopiedUrlIndex(null), 2000);
  };

  const handleCopyJson = () => {
    if (!rawResponse) return;
    navigator.clipboard.writeText(rawResponse);
    setCopiedJson(true);
    setTimeout(() => setCopiedJson(false), 2000);
  };

  const handleCopyCurl = () => {
    const curl = `curl -X POST "http://localhost:20229/v1/images/generations" \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer ${authKey.trim() || 'YOUR_API_KEY'}" \\
  -d '{
    "model": "${activeModel}",
    "prompt": ${JSON.stringify(prompt.trim() || 'A futuristic cyberpunk street in Hanoi at night')},
    "n": ${imageCount},
    "size": "${imageSize}",
    "response_format": "${responseFormat}"
  }'`;
    navigator.clipboard.writeText(curl);
    setCopiedCurl(true);
    setTimeout(() => setCopiedCurl(false), 2000);
  };

  const handleDownload = (item: GeneratedImageItem, index: number) => {
    const src = item.url || (item.b64_json ? `data:image/png;base64,${item.b64_json}` : '');
    if (!src) return;

    if (src.startsWith('data:')) {
      const a = document.createElement('a');
      a.href = src;
      a.download = `ezrouter-gen-${index + 1}-${Date.now()}.png`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
    } else {
      // For external URL, open in new tab or download
      window.open(src, '_blank');
    }
  };

  return (
    <div className="tab-pane active" style={{ display: 'block' }}>
      {/* Header */}
      <div className="tab-header" style={{ marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <div
              style={{
                width: 40,
                height: 40,
                borderRadius: 'var(--radius-md)',
                backgroundColor: 'rgba(52, 211, 153, 0.1)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--accent-primary)',
              }}
            >
              <IconImage size={22} />
            </div>
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>{t('images.title')}</h2>
                <span className="badge badge-success" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>
                  {t('images.badge')}
                </span>
              </div>
              <p style={{ margin: '4px 0 0', fontSize: 13, color: 'var(--text-secondary)' }}>
                {t('images.subtitle')}
              </p>
            </div>
          </div>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => setShowCurlModal(true)}
            style={{ display: 'flex', alignItems: 'center', gap: 6, minHeight: 44 }}
          >
            <IconCode size={16} />
            <span>{t('images.curlExample')}</span>
          </button>
        </div>
      </div>

      {/* Main Grid: Controls Left, Results Right */}
      <div className="images-studio-grid">
        {/* Left Column: Form & Configuration */}
        <div className="card" style={{ padding: 20 }}>
          <h3 style={{ margin: '0 0 16px', fontSize: 15, fontWeight: 600, color: 'var(--text-primary)' }}>
            {t('images.configTitle')}
          </h3>

          {/* Model Selection */}
          <div className="form-group" style={{ marginBottom: 16 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
              <label htmlFor="image-model-select" style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-secondary)' }}>
                {t('images.modelLabel')}
              </label>
              <button
                type="button"
                className="btn-link"
                onClick={() => setIsCustomModel(!isCustomModel)}
                style={{ fontSize: 12, color: 'var(--accent-primary)', background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
              >
                {isCustomModel ? t('images.selectFromList') : t('images.customModelInput')}
              </button>
            </div>

            {isCustomModel ? (
              <input
                id="image-model-custom"
                type="text"
                className="form-control"
                placeholder={t('images.customModelPlaceholder')}
                value={customModel}
                onChange={(e) => setCustomModel(e.target.value)}
                style={{ width: '100%', boxSizing: 'border-box' }}
              />
            ) : (
              <select
                id="image-model-select"
                className="form-control"
                value={selectedModel}
                onChange={(e) => setSelectedModel(e.target.value)}
                style={{ width: '100%', boxSizing: 'border-box' }}
              >
                <optgroup label={t('images.groupPopular')}>
                  {POPULAR_IMAGE_MODELS.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name} ({m.id})
                    </option>
                  ))}
                </optgroup>
                {providerModels.length > 0 && (
                  <optgroup label={t('images.groupProviders')}>
                    {providerModels.map((mId) => (
                      <option key={mId} value={mId}>
                        {mId}
                      </option>
                    ))}
                  </optgroup>
                )}
              </select>
            )}
          </div>

          {/* Prompt Input */}
          <div className="form-group" style={{ marginBottom: 16 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
              <label htmlFor="image-prompt" style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-secondary)' }}>
                {t('images.promptLabel')}
              </label>
              {prompt && (
                <button
                  type="button"
                  className="btn-link"
                  onClick={() => setPrompt('')}
                  style={{
                    fontSize: 12,
                    color: 'var(--text-muted)',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                  }}
                >
                  <IconTrash size={12} />
                  <span>{t('images.clearPrompt')}</span>
                </button>
              )}
            </div>
            <textarea
              id="image-prompt"
              className="form-control"
              rows={4}
              placeholder={t('images.promptPlaceholder')}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              style={{ width: '100%', boxSizing: 'border-box', resize: 'vertical' }}
            />

            {/* Quick Sample Prompts */}
            <div style={{ marginTop: 10 }}>
              <span style={{ fontSize: 11, color: 'var(--text-muted)', display: 'block', marginBottom: 6 }}>
                {t('images.samplePromptsLabel')}:
              </span>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => setPrompt(t('images.sample1Text'))}
                >
                  ✨ {t('images.sample1')}
                </button>
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => setPrompt(t('images.sample2Text'))}
                >
                  🎨 {t('images.sample2')}
                </button>
                <button
                  type="button"
                  className="preset-btn"
                  onClick={() => setPrompt(t('images.sample3Text'))}
                >
                  💎 {t('images.sample3')}
                </button>
              </div>
            </div>
          </div>

          {/* Dimensions & Parameters Row */}
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))', gap: 12, marginBottom: 16 }}>
            {/* Dimensions */}
            <div>
              <label htmlFor="image-size-select" style={{ fontSize: 12, fontWeight: 500, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>
                {t('images.sizeLabel')}
              </label>
              <select
                id="image-size-select"
                className="form-control"
                value={imageSize}
                onChange={(e) => setImageSize(e.target.value)}
                style={{ width: '100%', boxSizing: 'border-box' }}
              >
                <option value="1024x1024">1024 x 1024 (1:1)</option>
                <option value="1024x1792">1024 x 1792 (9:16)</option>
                <option value="1792x1024">1792 x 1024 (16:9)</option>
                <option value="512x512">512 x 512 (Nhỏ)</option>
              </select>
            </div>

            {/* Count (n) */}
            <div>
              <label htmlFor="image-count-select" style={{ fontSize: 12, fontWeight: 500, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>
                {t('images.countLabel')}
              </label>
              <select
                id="image-count-select"
                className="form-control"
                value={imageCount}
                onChange={(e) => setImageCount(Number(e.target.value))}
                style={{ width: '100%', boxSizing: 'border-box' }}
              >
                <option value={1}>1 ảnh</option>
                <option value={2}>2 ảnh</option>
                <option value={4}>4 ảnh</option>
              </select>
            </div>

            {/* Format */}
            <div>
              <label htmlFor="image-format-select" style={{ fontSize: 12, fontWeight: 500, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>
                {t('images.formatLabel')}
              </label>
              <select
                id="image-format-select"
                className="form-control"
                value={responseFormat}
                onChange={(e) => setResponseFormat(e.target.value)}
                style={{ width: '100%', boxSizing: 'border-box' }}
              >
                <option value="url">URL</option>
                <option value="b64_json">Base64 (b64_json)</option>
              </select>
            </div>
          </div>

          {/* Auth Key Input */}
          <div className="form-group" style={{ marginBottom: 20 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
              <label htmlFor="image-auth-key" style={{ fontSize: 12, fontWeight: 500, color: 'var(--text-secondary)' }}>
                {t('images.authKeyLabel')}
              </label>
              <button
                type="button"
                className="btn-link"
                onClick={() => setShowAuthKey(!showAuthKey)}
                style={{ fontSize: 11, color: 'var(--text-muted)', background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
              >
                {showAuthKey ? t('images.hideKey') : t('images.showKey')}
              </button>
            </div>
            <input
              id="image-auth-key"
              type={showAuthKey ? 'text' : 'password'}
              className="form-control"
              placeholder={t('images.authKeyPlaceholder')}
              value={authKey}
              onChange={(e) => setAuthKey(e.target.value)}
              style={{ width: '100%', boxSizing: 'border-box', fontFamily: 'var(--font-mono)', fontSize: 13 }}
            />
            <span style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 4, display: 'block' }}>
              {t('images.authKeyHelp')}
            </span>
          </div>

          {/* Generate Button */}
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleGenerate}
            disabled={!prompt.trim() || loading}
            style={{
              width: '100%',
              minHeight: 44,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 8,
              fontSize: 14,
              fontWeight: 600,
            }}
          >
            {loading ? (
              <>
                <div className="spinner" style={{ width: 16, height: 16, borderWidth: 2 }} />
                <span>
                  {t('images.generating')} ({(elapsedMs / 1000).toFixed(1)}s)
                </span>
              </>
            ) : (
              <>
                <IconImage size={18} />
                <span>{t('images.generateBtn')}</span>
              </>
            )}
          </button>
        </div>

        {/* Right Column: Output / Gallery / Status */}
        <div className="card" style={{ padding: 20 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16, flexWrap: 'wrap', gap: 8 }}>
            <h3 style={{ margin: 0, fontSize: 15, fontWeight: 600, color: 'var(--text-primary)' }}>
              {t('images.resultTitle')}
            </h3>
            {elapsedMs > 0 && !loading && (
              <span className="badge badge-success" style={{ fontSize: 12 }}>
                {t('images.duration')}: {(elapsedMs / 1000).toFixed(2)}s
              </span>
            )}
          </div>

          {/* Error Message Banner */}
          {errorMessage && (
            <div
              className="alert alert-danger"
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: 10,
                marginBottom: 16,
                padding: 12,
                borderRadius: 'var(--radius-md)',
              }}
            >
              <IconAlertCircle size={20} style={{ color: 'var(--color-error)', flexShrink: 0, marginTop: 2 }} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <strong style={{ display: 'block', fontSize: 13, marginBottom: 2 }}>
                  {t('images.errorTitle')} {errorStatus ? `(${errorStatus})` : ''}
                </strong>
                <span style={{ fontSize: 12, wordBreak: 'break-word', fontFamily: 'var(--font-mono)' }}>
                  {errorMessage}
                </span>
              </div>
            </div>
          )}

          {/* Loading Skeleton */}
          {loading && (
            <div
              style={{
                minHeight: 340,
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: 'var(--bg-app)',
                borderRadius: 'var(--radius-md)',
                border: '1px dashed var(--border-subtle)',
                padding: 24,
                textAlign: 'center',
              }}
            >
              <div className="spinner" style={{ width: 40, height: 40, borderWidth: 3, marginBottom: 16 }} />
              <h4 style={{ margin: '0 0 6px', fontSize: 15 }}>{t('images.generating')}</h4>
              <p style={{ margin: 0, fontSize: 13, color: 'var(--text-muted)' }}>
                {(elapsedMs / 1000).toFixed(1)}s • Dispatching request via upstream provider...
              </p>
            </div>
          )}

          {/* Empty State */}
          {!loading && results.length === 0 && !errorMessage && (
            <div
              style={{
                minHeight: 340,
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                backgroundColor: 'var(--bg-app)',
                borderRadius: 'var(--radius-md)',
                border: '1px dashed var(--border-subtle)',
                padding: 32,
                textAlign: 'center',
              }}
            >
              <div
                style={{
                  width: 56,
                  height: 56,
                  borderRadius: '50%',
                  backgroundColor: 'rgba(255, 255, 255, 0.04)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--text-muted)',
                  marginBottom: 16,
                }}
              >
                <IconImage size={28} />
              </div>
              <h4 style={{ margin: '0 0 6px', fontSize: 15, color: 'var(--text-primary)' }}>
                {t('images.readyTitle')}
              </h4>
              <p style={{ margin: 0, fontSize: 13, color: 'var(--text-secondary)', maxWidth: 360, lineHeight: 1.5 }}>
                {t('images.readyDesc')}
              </p>
            </div>
          )}

          {/* Results Gallery */}
          {!loading && results.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: results.length > 1 ? 'repeat(auto-fit, minmax(240px, 1fr))' : '1fr',
                  gap: 16,
                }}
              >
                {results.map((item, index) => {
                  const imageSrc =
                    item.url ||
                    (item.b64_json ? `data:image/png;base64,${item.b64_json}` : '');
                  return (
                    <div
                      key={index}
                      className="image-card-item"
                      style={{
                        backgroundColor: 'var(--bg-app)',
                        borderRadius: 'var(--radius-md)',
                        border: '1px solid var(--border-subtle)',
                        overflow: 'hidden',
                        display: 'flex',
                        flexDirection: 'column',
                      }}
                    >
                      {/* Image Render Area */}
                      <div
                        style={{
                          position: 'relative',
                          width: '100%',
                          backgroundColor: '#000000',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          minHeight: 240,
                          cursor: 'pointer',
                        }}
                        onClick={() => imageSrc && setLightboxUrl(imageSrc)}
                      >
                        {imageSrc ? (
                          <img
                            src={imageSrc}
                            alt={item.revised_prompt || prompt}
                            style={{
                              width: '100%',
                              height: 'auto',
                              display: 'block',
                              objectFit: 'contain',
                              maxHeight: 400,
                            }}
                            loading="lazy"
                          />
                        ) : (
                          <div style={{ padding: 20, color: 'var(--text-muted)', fontSize: 12 }}>
                            {t('images.noImagesReturned')}
                          </div>
                        )}
                        <div
                          className="image-overlay-actions"
                          style={{
                            position: 'absolute',
                            top: 8,
                            right: 8,
                            display: 'flex',
                            gap: 6,
                          }}
                        >
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm btn-icon-only"
                            title={t('images.viewFull')}
                            onClick={(e) => {
                              e.stopPropagation();
                              if (imageSrc) setLightboxUrl(imageSrc);
                            }}
                            style={{
                              backgroundColor: 'rgba(0,0,0,0.65)',
                              color: '#ffffff',
                              border: 'none',
                              backdropFilter: 'blur(4px)',
                            }}
                          >
                            <IconMaximize size={16} />
                          </button>
                        </div>
                      </div>

                      {/* Card Footer Actions */}
                      <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
                        {item.revised_prompt && (
                          <p
                            style={{
                              margin: 0,
                              fontSize: 11,
                              color: 'var(--text-muted)',
                              fontStyle: 'italic',
                              lineHeight: 1.4,
                            }}
                          >
                            <strong>{t('images.revisedPrompt')}:</strong> {item.revised_prompt}
                          </p>
                        )}
                        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                          {item.url && (
                            <button
                              type="button"
                              className="btn btn-secondary btn-sm"
                              onClick={() => handleCopyUrl(item.url!, index)}
                              style={{ flex: 1, minHeight: 44, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
                            >
                              {copiedUrlIndex === index ? (
                                <>
                                  <IconCheck size={14} style={{ color: 'var(--color-success)' }} />
                                  <span>{t('images.copied')}</span>
                                </>
                              ) : (
                                <>
                                  <IconCopy size={14} />
                                  <span>{t('images.copyUrl')}</span>
                                </>
                              )}
                            </button>
                          )}
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm"
                            onClick={() => handleDownload(item, index)}
                            style={{ flex: 1, minHeight: 44, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
                          >
                            <IconDownload size={14} />
                            <span>{t('images.download')}</span>
                          </button>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Raw JSON Response Accordion */}
              {rawResponse && (
                <div style={{ marginTop: 12 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                    <button
                      type="button"
                      className="btn-link"
                      onClick={() => setShowRawJson(!showRawJson)}
                      style={{
                        fontSize: 12,
                        color: 'var(--text-secondary)',
                        background: 'none',
                        border: 'none',
                        cursor: 'pointer',
                        padding: 0,
                        display: 'flex',
                        alignItems: 'center',
                        gap: 6,
                      }}
                    >
                      <IconCode size={14} />
                      <span>{showRawJson ? '▲ ' : '▼ '} {t('images.rawJson')}</span>
                    </button>
                    {showRawJson && (
                      <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={handleCopyJson}
                        style={{ fontSize: 11, padding: '4px 8px', minHeight: 32 }}
                      >
                        {copiedJson ? t('images.copied') : t('images.copyJson')}
                      </button>
                    )}
                  </div>
                  {showRawJson && (
                    <pre
                      style={{
                        margin: 0,
                        padding: 12,
                        backgroundColor: 'var(--bg-app)',
                        borderRadius: 'var(--radius-md)',
                        border: '1px solid var(--border-subtle)',
                        fontFamily: 'var(--font-mono)',
                        fontSize: 12,
                        maxHeight: 250,
                        overflowY: 'auto',
                        whiteSpace: 'pre-wrap',
                        wordBreak: 'break-all',
                      }}
                    >
                      {rawResponse}
                    </pre>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Lightbox Modal */}
      {lightboxUrl && (
        <div
          className="modal-backdrop"
          onClick={() => setLightboxUrl(null)}
          style={{
            position: 'fixed',
            inset: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.85)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 9999,
            padding: 20,
            backdropFilter: 'blur(6px)',
          }}
        >
          <div
            style={{
              position: 'relative',
              maxWidth: '90vw',
              maxHeight: '90vh',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              className="btn btn-secondary btn-icon-only"
              onClick={() => setLightboxUrl(null)}
              style={{
                position: 'absolute',
                top: -48,
                right: 0,
                backgroundColor: 'rgba(255, 255, 255, 0.15)',
                color: '#ffffff',
                border: 'none',
                minHeight: 44,
                minWidth: 44,
              }}
            >
              <IconX size={20} />
            </button>
            <img
              src={lightboxUrl}
              alt="Fullscreen preview"
              style={{
                maxWidth: '100%',
                maxHeight: '85vh',
                objectFit: 'contain',
                borderRadius: 'var(--radius-md)',
                boxShadow: '0 20px 40px rgba(0, 0, 0, 0.6)',
              }}
            />
          </div>
        </div>
      )}

      {/* cURL Snippet Modal */}
      {showCurlModal && (
        <div
          className="modal-backdrop"
          onClick={() => setShowCurlModal(false)}
          style={{
            position: 'fixed',
            inset: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.75)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 9999,
            padding: 20,
          }}
        >
          <div
            className="modal-card"
            style={{
              backgroundColor: 'var(--bg-card)',
              border: '1px solid var(--border-subtle)',
              borderRadius: 'var(--radius-lg)',
              maxWidth: 600,
              width: '100%',
              padding: 24,
              position: 'relative',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h3 style={{ margin: 0, fontSize: 16, fontWeight: 600 }}>{t('images.curlExample')}</h3>
              <button
                type="button"
                className="btn-icon-only"
                onClick={() => setShowCurlModal(false)}
                style={{ background: 'none', border: 'none', color: 'var(--text-muted)', cursor: 'pointer', minHeight: 44, minWidth: 44 }}
              >
                <IconX size={18} />
              </button>
            </div>
            <pre
              style={{
                backgroundColor: 'var(--bg-app)',
                padding: 16,
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border-subtle)',
                fontFamily: 'var(--font-mono)',
                fontSize: 12,
                overflowX: 'auto',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                lineHeight: 1.5,
              }}
            >
{`curl -X POST "http://localhost:20229/v1/images/generations" \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer ${authKey.trim() || 'YOUR_API_KEY'}" \\
  -d '{
    "model": "${activeModel}",
    "prompt": ${JSON.stringify(prompt.trim() || 'A futuristic cyberpunk street in Hanoi at night')},
    "n": ${imageCount},
    "size": "${imageSize}",
    "response_format": "${responseFormat}"
  }'`}
            </pre>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, marginTop: 16 }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setShowCurlModal(false)}
                style={{ minHeight: 44 }}
              >
                {t('images.closeModal')}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleCopyCurl}
                style={{ minHeight: 44, display: 'flex', alignItems: 'center', gap: 6 }}
              >
                {copiedCurl ? <IconCheck size={16} /> : <IconCopy size={16} />}
                <span>{copiedCurl ? t('images.copied') : t('images.copyJson')}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
