import React, { useState } from 'react';
import { useI18n } from '../i18n';
import {
  IconCopy,
  IconCheck,
  IconShield,
  IconZap,
  IconAlertCircle,
  IconTerminal,
  IconCode,
  IconBookOpen,
} from '../icons';

export const IntegrationTab: React.FC = () => {
  const { t } = useI18n();
  const [apiKey, setApiKey] = useState('ag-proxy-key');
  const [selectedModel, setSelectedModel] = useState<string>('ag/gemini-3.8-flash-high');
  const [activeTab, setActiveTab] = useState<'hermes' | 'codex' | 'python' | 'curl' | 'node'>('hermes');
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const baseUrl = 'https://router.namhv.vip/v1';

  const copyToClipboard = async (text: string, id: string) => {
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        const textArea = document.createElement('textarea');
        textArea.value = text;
        textArea.style.position = 'fixed';
        textArea.style.opacity = '0';
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();
        document.execCommand('copy');
        document.body.removeChild(textArea);
      }
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch (err) {
      console.error('Failed to copy: ', err);
    }
  };

  const activeKey = apiKey.trim() || 'ag-proxy-key';

  // Snippets
  const hermesConfigSnippet = `# ~/.hermes/config.yaml
# Cấu hình kết nối Hermes Agent với ezRouter
llm:
  provider: openai
  base_url: ${baseUrl}
  api_key: ${activeKey}
  model: ${selectedModel}
  temperature: 0.7
  max_tokens: 4096
`;

  const hermesCliEnvSnippet = `# Thiết lập biến môi trường cho Hermes CLI
export OPENAI_BASE_URL="${baseUrl}"
export OPENAI_API_KEY="${activeKey}"

# Chạy Hermes với model mong muốn
hermes chat --model ${selectedModel}
`;

  const codexConfigSnippet = `# ~/.codex/config.toml
# Cấu hình endpoint cho OpenAI Codex CLI
model = "${selectedModel}"
base_url = "${baseUrl}"
api_key = "${activeKey}"

[options]
temperature = 0.2
stream = true
`;

  const codexCliSnippet = `# Thiết lập môi trường và thực thi trực tiếp với Codex CLI
export OPENAI_BASE_URL="${baseUrl}"
export OPENAI_API_KEY="${activeKey}"

codex -m ${selectedModel} "Viết hàm xử lý exponential backoff retry bằng Rust"
`;

  const pythonSdkSnippet = `import os
from openai import OpenAI

# Khởi tạo OpenAI client trỏ trực tiếp tới ezRouter
client = OpenAI(
    base_url="${baseUrl}",
    api_key=os.environ.get("ROUTER_API_KEY", "${activeKey}"),
)

print("=== Gửi yêu cầu Streaming tới model: ${selectedModel} ===")
stream = client.chat.completions.create(
    model="${selectedModel}",
    messages=[
        {"role": "system", "content": "Bạn là trợ lý AI chuyên sâu, phản hồi chuẩn xác bằng tiếng Việt."},
        {"role": "user", "content": "Giải thích ngắn gọn cơ chế hoạt động của một LLM Router."},
    ],
    temperature=0.7,
    stream=True,  # Nhận từng token qua Server-Sent Events (SSE)
)

for chunk in stream:
    delta = chunk.choices[0].delta.content or ""
    print(delta, end="", flush=True)

print("\\n")
`;

  const curlStandardSnippet = `curl -X POST "${baseUrl}/chat/completions" \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer ${activeKey}" \\
  -d '{
    "model": "${selectedModel}",
    "messages": [
      {"role": "system", "content": "Bạn là trợ lý AI hữu ích."},
      {"role": "user", "content": "Xin chào! Hãy giới thiệu 3 tính năng mạnh nhất của bạn."}
    ],
    "temperature": 0.7
  }'
`;

  const curlStreamSnippet = `curl -N -X POST "${baseUrl}/chat/completions" \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer ${activeKey}" \\
  -d '{
    "model": "${selectedModel}",
    "messages": [
      {"role": "user", "content": "Viết đoạn code Python tính số Fibonacci đệ quy có memoization."}
    ],
    "stream": true
  }'
`;

  const curlModelsSnippet = `curl -s -X GET "${baseUrl}/models" \\
  -H "Authorization: Bearer ${activeKey}"
`;

  const nodeSdkSnippet = `import OpenAI from 'openai';

// Khởi tạo SDK trỏ tới ezRouter
const client = new OpenAI({
  baseURL: '${baseUrl}',
  apiKey: process.env.ROUTER_API_KEY || '${activeKey}',
});

async function runDemo() {
  console.log('--- Bắt đầu nhận phản hồi trực tiếp (Streaming) ---');
  const stream = await client.chat.completions.create({
    model: '${selectedModel}',
    messages: [
      { role: 'system', content: 'Bạn là chuyên gia kiến trúc phần mềm cao cấp.' },
      { role: 'user', content: 'Tóm tắt 3 lợi ích cốt lõi khi tích hợp LLM Router nội bộ.' },
    ],
    stream: true,
  });

  for await (const chunk of stream) {
    const text = chunk.choices[0]?.delta?.content || '';
    process.stdout.write(text);
  }
  console.log('\\n--- Hoàn tất ---');
}

runDemo().catch(console.error);
`;

  const nodeFetchSnippet = `// Tương thích Node.js 18+, Bun, Deno (Không cần cài thêm thư viện ngoài)
async function callRouter() {
  const response = await fetch('${baseUrl}/chat/completions', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': 'Bearer ${activeKey}',
    },
    body: JSON.stringify({
      model: '${selectedModel}',
      messages: [
        { role: 'user', content: 'Xin chào từ Node.js native fetch!' }
      ],
      temperature: 0.7,
    }),
  });

  if (!response.ok) {
    const err = await response.text();
    throw new Error(\`Lỗi HTTP \${response.status}: \${err}\`);
  }

  const result = await response.json();
  console.log('Kết quả:', result.choices[0].message.content);
}

callRouter();
`;

  const toolsSnippet = `{
  "model": "${selectedModel}",
  "messages": [
    { "role": "user", "content": "Thời tiết Hà Nội hôm nay thế nào?" }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "lay_thong_tin_thoi_tiet",
        "description": "Lấy dữ liệu thời tiết hiện tại theo tỉnh thành",
        "parameters": {
          "type": "object",
          "properties": {
            "dia_diem": {
              "type": "string",
              "description": "Tên thành phố hoặc tỉnh, ví dụ: Hà Nội, TP.HCM"
            }
          },
          "required": ["dia_diem"]
        }
      }
    }
  ],
  "tool_choice": "auto"
}`;

  return (
    <div className="integration-guide">
      {/* Header */}
      <div className="section-header">
        <div>
          <h2 className="section-title">{t('integration.title')}</h2>
          <span style={{ fontSize: 13, color: 'var(--text-muted)' }}>
            {t('integration.subtitle')}
          </span>
        </div>
      </div>

      {/* Security Warning Callout */}
      <div className="security-alert-box">
        <div className="security-alert-header">
          <IconShield size={22} style={{ color: '#f59e0b', flexShrink: 0 }} />
          <div>
            <h3 style={{ fontSize: 15, fontWeight: 700, color: '#fbbf24', margin: 0 }}>
              {t('integration.securityTitle')}
            </h3>
            <p style={{ fontSize: 13, color: '#fef3c7', marginTop: 4, lineHeight: 1.5 }}>
              {t('integration.securityDesc')}
            </p>
          </div>
        </div>
        <ul className="security-alert-list">
          <li>
            <strong>{t('integration.securityNoEmbedKey')}</strong> {t('integration.securityNoEmbedKeyDesc')}
          </li>
          <li>
            <strong>{t('integration.securityNoCommit')}</strong> {t('integration.securityNoCommitDesc')}
          </li>
          <li>
            <strong>{t('integration.securityUseEnv')}</strong> {t('integration.securityUseEnvDesc')}
          </li>
          <li>
            <strong>{t('integration.securityBackendProxy')}</strong> {t('integration.securityBackendProxyDesc')}
          </li>
        </ul>
      </div>

      {/* Quick Info Grid */}
      <div className="integration-info-grid">
        <div className="card integration-info-card">
          <div className="kpi-label">
            <span>{t('integration.standardBaseUrl')}</span>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => copyToClipboard(baseUrl, 'base-url')}
              title={t('integration.copyBaseUrlTitle')}
            >
              {copiedId === 'base-url' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
              <span>{copiedId === 'base-url' ? t('actions.copied') : t('actions.copy')}</span>
            </button>
          </div>
          <div className="integration-code-highlight" style={{ fontSize: 14 }}>
            {baseUrl}
          </div>
          <div className="kpi-sub" style={{ marginTop: 8 }}>
            {t('integration.baseUrlDesc')}
          </div>
        </div>

        <div className="card integration-info-card">
          <div className="kpi-label">
            <span>{t('integration.apiKeyPlaceholderLabel')}</span>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => copyToClipboard('ag-proxy-key', 'api-key-placeholder')}
              title={t('integration.copyApiKeyTitle')}
            >
              {copiedId === 'api-key-placeholder' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
              <span>{copiedId === 'api-key-placeholder' ? t('actions.copied') : t('actions.copy')}</span>
            </button>
          </div>
          <div className="integration-code-highlight" style={{ fontSize: 14 }}>
            ag-proxy-key
          </div>
          <div className="kpi-sub" style={{ marginTop: 8 }}>
            {t('integration.apiKeyDesc')}
          </div>
        </div>
      </div>

      {/* Interactive Customizer Bar */}
      <div className="card" style={{ padding: 18 }}>
        <h4 style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 12 }}>
          {t('integration.customizerTitle')}
        </h4>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 14 }}>
          <div>
            <label style={{ display: 'block', fontSize: 12, color: 'var(--text-muted)', marginBottom: 6, fontWeight: 500 }}>
              {t('integration.yourApiKeyLabel')}
            </label>
            <input
              type="text"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="ag-proxy-key"
              style={{ width: '100%' }}
            />
          </div>

          <div>
            <label style={{ display: 'block', fontSize: 12, color: 'var(--text-muted)', marginBottom: 6, fontWeight: 500 }}>
              {t('integration.modelSelectionLabel')}
            </label>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              style={{ width: '100%' }}
            >
              <option value="ag/gemini-3.8-flash-high">ag/gemini-3.8-flash-high ({t('integration.modelOptGeminiFlash')})</option>
              <option value="ag/gemini-pro-agent">ag/gemini-pro-agent ({t('integration.modelOptGeminiProAgent')})</option>
              <option value="ag/gemini-3.1-pro-low">ag/gemini-3.1-pro-low ({t('integration.modelOptGeminiProLow')})</option>
              <option value="cx/gpt-6.1-sol">cx/gpt-6.1-sol ({t('integration.modelOptCodex61Sol')})</option>
              <option value="cx/gpt-6-astra">cx/gpt-6-astra ({t('integration.modelOptCodexAstra')})</option>
              <option value="cx/gpt-5.6-sol">cx/gpt-5.6-sol ({t('integration.modelOptCodexSol')})</option>
            </select>
          </div>
        </div>
      </div>

      {/* Model Examples Reference Card */}
      <div className="card">
        <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconBookOpen size={18} style={{ color: 'var(--accent-primary)' }} />
          <span>{t('integration.modelCatalogTitle')}</span>
        </h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: 14 }}>
          <div className="model-spec-card">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="badge badge-success">Google Antigravity</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard('ag/gemini-3.8-flash-high', 'm-ag')}
                title={t('integration.copyModelTitle')}
              >
                {copiedId === 'm-ag' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'm-ag' ? t('actions.copied') : t('actions.copy')}</span>
              </button>
            </div>
            <div className="model-spec-name">ag/gemini-3.8-flash-high</div>
            <p className="model-spec-desc">
              {t('integration.geminiFlashDesc')}
            </p>
          </div>

          <div className="model-spec-card">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="badge badge-primary">OpenAI Codex Flagship</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard('cx/gpt-6.1-sol', 'm-cx61')}
                title={t('integration.copyModelTitle')}
              >
                {copiedId === 'm-cx61' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'm-cx61' ? t('actions.copied') : t('actions.copy')}</span>
              </button>
            </div>
            <div className="model-spec-name">cx/gpt-6.1-sol</div>
            <p className="model-spec-desc">
              {t('integration.codex61SolDesc')}
            </p>
          </div>

          <div className="model-spec-card">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="badge badge-primary">OpenAI Codex</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard('cx/gpt-5.6-sol', 'm-cx')}
                title={t('integration.copyModelTitle')}
              >
                {copiedId === 'm-cx' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'm-cx' ? t('actions.copied') : t('actions.copy')}</span>
              </button>
            </div>
            <div className="model-spec-name">cx/gpt-5.6-sol</div>
            <p className="model-spec-desc">
              {t('integration.codexSolDesc')}
            </p>
          </div>
        </div>
      </div>

      {/* Code Examples Navigation Tabs */}
      <div className="integration-tabs-container">
        <div className="integration-tabs-header">
          <button
            type="button"
            className={`integration-tab-btn ${activeTab === 'hermes' ? 'active' : ''}`}
            onClick={() => setActiveTab('hermes')}
          >
            <IconZap size={16} />
            <span>Hermes Agent</span>
          </button>
          <button
            type="button"
            className={`integration-tab-btn ${activeTab === 'codex' ? 'active' : ''}`}
            onClick={() => setActiveTab('codex')}
          >
            <IconTerminal size={16} />
            <span>Codex CLI</span>
          </button>
          <button
            type="button"
            className={`integration-tab-btn ${activeTab === 'python' ? 'active' : ''}`}
            onClick={() => setActiveTab('python')}
          >
            <IconCode size={16} />
            <span>OpenAI SDK Python</span>
          </button>
          <button
            type="button"
            className={`integration-tab-btn ${activeTab === 'curl' ? 'active' : ''}`}
            onClick={() => setActiveTab('curl')}
          >
            <IconTerminal size={16} />
            <span>cURL / Bash</span>
          </button>
          <button
            type="button"
            className={`integration-tab-btn ${activeTab === 'node' ? 'active' : ''}`}
            onClick={() => setActiveTab('node')}
          >
            <IconCode size={16} />
            <span>Node.js / TS</span>
          </button>
        </div>

        {/* Tab Content: Hermes */}
        {activeTab === 'hermes' && (
          <div className="integration-tab-pane">
            <h4 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 8 }}>
              {t('integration.hermesTitle')}
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              {t('integration.hermesDesc')}
            </p>

            {/* Config YAML */}
            <div className="code-card">
              <div className="code-card-header">
                <span className="code-file-tag">~/.hermes/config.yaml</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(hermesConfigSnippet, 'hermes-yaml')}
                >
                  {copiedId === 'hermes-yaml' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'hermes-yaml' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{hermesConfigSnippet}</code>
              </pre>
            </div>

            {/* CLI Environment */}
            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">{t('integration.terminalEnvVars')}</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(hermesCliEnvSnippet, 'hermes-env')}
                >
                  {copiedId === 'hermes-env' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'hermes-env' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{hermesCliEnvSnippet}</code>
              </pre>
            </div>
          </div>
        )}

        {/* Tab Content: Codex CLI */}
        {activeTab === 'codex' && (
          <div className="integration-tab-pane">
            <h4 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 8 }}>
              {t('integration.codexTitle')}
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              {t('integration.codexDesc')}
            </p>

            <div className="code-card">
              <div className="code-card-header">
                <span className="code-file-tag">~/.codex/config.toml</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(codexConfigSnippet, 'codex-toml')}
                >
                  {copiedId === 'codex-toml' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'codex-toml' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{codexConfigSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">{t('integration.codexCliTag')}</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(codexCliSnippet, 'codex-cli')}
                >
                  {copiedId === 'codex-cli' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'codex-cli' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{codexCliSnippet}</code>
              </pre>
            </div>
          </div>
        )}

        {/* Tab Content: Python SDK */}
        {activeTab === 'python' && (
          <div className="integration-tab-pane">
            <h4 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 8 }}>
              {t('integration.pythonTitle')}
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              {t('integration.pythonDesc')}
            </p>

            <div className="code-card">
              <div className="code-card-header">
                <span className="code-file-tag">example_openai.py</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(pythonSdkSnippet, 'py-sdk')}
                >
                  {copiedId === 'py-sdk' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'py-sdk' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{pythonSdkSnippet}</code>
              </pre>
            </div>
          </div>
        )}

        {/* Tab Content: cURL */}
        {activeTab === 'curl' && (
          <div className="integration-tab-pane">
            <h4 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 8 }}>
              {t('integration.curlTitle')}
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              {t('integration.curlDesc')}
            </p>

            <div className="code-card">
              <div className="code-card-header">
                <span className="code-file-tag">{t('integration.curlStandardTag')}</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(curlStandardSnippet, 'curl-std')}
                >
                  {copiedId === 'curl-std' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'curl-std' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{curlStandardSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">{t('integration.curlStreamTag')}</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(curlStreamSnippet, 'curl-stream')}
                >
                  {copiedId === 'curl-stream' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'curl-stream' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{curlStreamSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">{t('integration.curlModelsTag')}</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(curlModelsSnippet, 'curl-models')}
                >
                  {copiedId === 'curl-models' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'curl-models' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{curlModelsSnippet}</code>
              </pre>
            </div>
          </div>
        )}

        {/* Tab Content: Node.js */}
        {activeTab === 'node' && (
          <div className="integration-tab-pane">
            <h4 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 8 }}>
              {t('integration.nodeTitle')}
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              {t('integration.nodeDesc')}
            </p>

            <div className="code-card">
              <div className="code-card-header">
                <span className="code-file-tag">OpenAI Node.js SDK (npm i openai)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(nodeSdkSnippet, 'node-sdk')}
                >
                  {copiedId === 'node-sdk' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'node-sdk' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{nodeSdkSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">Native fetch (Node.js 18+, Bun, Deno)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(nodeFetchSnippet, 'node-fetch')}
                >
                  {copiedId === 'node-fetch' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'node-fetch' ? t('actions.copied') : t('actions.copy')}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{nodeFetchSnippet}</code>
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* Streaming Architecture & Notes */}
      <div className="card">
        <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconZap size={18} style={{ color: '#38bdf8' }} />
          <span>{t('integration.streamingTitle')}</span>
        </h3>
        <div style={{ fontSize: 13.5, color: 'var(--text-secondary)', lineHeight: 1.6 }}>
          <p style={{ marginBottom: 10 }}>
            {t('integration.streamingIntro')}
          </p>
          <ul style={{ paddingLeft: 20, marginBottom: 10, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <li>
              <strong>{t('integration.streamingLowLatency')}</strong> {t('integration.streamingLowLatencyDesc')}
            </li>
            <li>
              <strong>{t('integration.streamingChunkFormat')}</strong> {t('integration.streamingChunkFormatDesc')}
            </li>
            <li>
              <strong>{t('integration.streamingDoneSignal')}</strong> {t('integration.streamingDoneSignalDesc')}
            </li>
            <li>
              <strong>{t('integration.streamingProxyNote')}</strong> {t('integration.streamingProxyNoteDesc')}
            </li>
          </ul>
        </div>
      </div>

      {/* Tools & Function Calling Notes */}
      <div className="card">
        <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconCode size={18} style={{ color: '#a78bfa' }} />
          <span>{t('integration.toolsTitle')}</span>
        </h3>
        <div style={{ fontSize: 13.5, color: 'var(--text-secondary)', lineHeight: 1.6 }}>
          <p style={{ marginBottom: 10 }}>
            {t('integration.toolsIntro')}
          </p>
          <ul style={{ paddingLeft: 20, marginBottom: 12, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <li>
              <strong>{t('integration.toolsProtocol')}</strong> {t('integration.toolsProtocolDesc')}
            </li>
            <li>
              <strong>{t('integration.toolsLoop')}</strong> {t('integration.toolsLoopDesc')}
            </li>
          </ul>

          <div className="code-card">
            <div className="code-card-header">
              <span className="code-file-tag">{t('integration.toolsPayloadTag')}</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard(toolsSnippet, 'tools-json')}
              >
                {copiedId === 'tools-json' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'tools-json' ? t('actions.copied') : t('actions.copy')}</span>
              </button>
            </div>
            <pre className="code-card-pre">
              <code>{toolsSnippet}</code>
            </pre>
          </div>
        </div>
      </div>

      {/* Error Troubleshooting Guide */}
      <div className="card">
        <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 14, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconAlertCircle size={18} style={{ color: '#ef4444' }} />
          <span>{t('integration.troubleshootTitle')}</span>
        </h3>

        <div className="troubleshoot-grid">
          {/* 401 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-401">HTTP 401 Unauthorized</div>
            <div className="troubleshoot-title">{t('integration.err401Title')}</div>
            <div className="troubleshoot-desc">
              <strong>{t('integration.causeLabel')}</strong> {t('integration.err401Cause')}
            </div>
            <div className="troubleshoot-fix">
              <strong>{t('integration.fixLabel')}</strong> {t('integration.err401Fix')}
            </div>
          </div>

          {/* 404 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-404">HTTP 404 Not Found / Model Not Found</div>
            <div className="troubleshoot-title">{t('integration.err404Title')}</div>
            <div className="troubleshoot-desc">
              <strong>{t('integration.causeLabel')}</strong> {t('integration.err404Cause')}
            </div>
            <div className="troubleshoot-fix">
              <strong>{t('integration.fixLabel')}</strong> {t('integration.err404Fix')}
            </div>
          </div>

          {/* 429 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-429">HTTP 429 Rate Limit / Cooldown</div>
            <div className="troubleshoot-title">{t('integration.err429Title')}</div>
            <div className="troubleshoot-desc">
              <strong>{t('integration.causeLabel')}</strong> {t('integration.err429Cause')}
            </div>
            <div className="troubleshoot-fix">
              <strong>{t('integration.fixLabel')}</strong> {t('integration.err429Fix')}
            </div>
          </div>

          {/* 500 / 502 / 503 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-500">HTTP 500 / 502 / 503 Upstream Error</div>
            <div className="troubleshoot-title">{t('integration.err500Title')}</div>
            <div className="troubleshoot-desc">
              <strong>{t('integration.causeLabel')}</strong> {t('integration.err500Cause')}
            </div>
            <div className="troubleshoot-fix">
              <strong>{t('integration.fixLabel')}</strong> {t('integration.err500Fix')}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
