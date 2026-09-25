import React, { useState } from 'react';
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
  const [apiKey, setApiKey] = useState('ag-proxy-key');
  const [selectedModel, setSelectedModel] = useState<string>('ag/gemini-3.8-flash-high');
  const [activeTab, setActiveTab] = useState<'hermes' | 'codex' | 'python' | 'curl' | 'node'>('hermes');
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const baseUrl = typeof window !== 'undefined' && window.location.origin
    ? `${window.location.origin}/v1`
    : 'http://localhost:20229/v1';

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
          <h2 className="section-title">Hướng Dẫn Tích Hợp API (API Integration)</h2>
          <span style={{ fontSize: 13, color: 'var(--text-muted)' }}>
            Chuẩn giao thức OpenAI-compatible v1 endpoint phục vụ Hermes, Codex CLI, Python, cURL, Node.js
          </span>
        </div>
      </div>

      {/* Security Warning Callout */}
      <div className="security-alert-box">
        <div className="security-alert-header">
          <IconShield size={22} style={{ color: '#f59e0b', flexShrink: 0 }} />
          <div>
            <h3 style={{ fontSize: 15, fontWeight: 700, color: '#fbbf24', margin: 0 }}>
              Cảnh Báo An Toàn: Bảo Vệ Khóa API (Never Expose Secrets)
            </h3>
            <p style={{ fontSize: 13, color: '#fef3c7', marginTop: 4, lineHeight: 1.5 }}>
              Khóa API cấp quyền truy cập trực tiếp vào các mô hình và tài nguyên của hệ thống. Hãy tuân thủ nghiêm ngặt các quy tắc an toàn sau:
            </p>
          </div>
        </div>
        <ul className="security-alert-list">
          <li>
            <strong>Tuyệt đối KHÔNG nhúng trực tiếp API Key vào mã nguồn client-side:</strong> Tránh lưu key trong mã JavaScript frontend (React, Vue, HTML tĩnh) hoặc bundle ứng dụng di động công khai. Kẻ xấu có thể trích xuất khóa qua DevTools hoặc dịch ngược gói cài đặt.
          </li>
          <li>
            <strong>Không commit khóa lên kho mã nguồn:</strong> Đảm bảo các file cấu hình chứa key (như <code>.env</code>, <code>config.yaml</code>, <code>config.toml</code>) đã được thêm vào <code>.gitignore</code> trước khi đẩy lên GitHub, GitLab hay các kho lưu trữ công cộng.
          </li>
          <li>
            <strong>Sử dụng Biến Môi Trường (Environment Variables):</strong> Luôn nạp key qua biến môi trường của hệ điều hành hoặc các hệ thống Secret Management bảo mật (Vault, AWS Secrets Manager, GitHub Secrets).
          </li>
          <li>
            <strong>Kiến trúc Backend Proxy:</strong> Với các ứng dụng web phục vụ người dùng cuối, mọi lệnh gọi API phải đi qua backend server nội bộ của bạn thay vì cho phép trình duyệt người dùng gửi trực tiếp.
          </li>
        </ul>
      </div>

      {/* Quick Info Grid */}
      <div className="integration-info-grid">
        <div className="card integration-info-card">
          <div className="kpi-label">
            <span>Base URL Chuẩn</span>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => copyToClipboard(baseUrl, 'base-url')}
              title="Sao chép Base URL"
            >
              {copiedId === 'base-url' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
              <span>{copiedId === 'base-url' ? 'Đã sao chép' : 'Sao chép'}</span>
            </button>
          </div>
          <div className="integration-code-highlight" style={{ fontSize: 14 }}>
            {baseUrl}
          </div>
          <div className="kpi-sub" style={{ marginTop: 8 }}>
            Tương thích trực tiếp với tham số <code>base_url</code> / <code>baseURL</code> của mọi thư viện OpenAI v1.
          </div>
        </div>

        <div className="card integration-info-card">
          <div className="kpi-label">
            <span>API Key Placeholder</span>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => copyToClipboard('ag-proxy-key', 'api-key-placeholder')}
              title="Sao chép API Key mẫu"
            >
              {copiedId === 'api-key-placeholder' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
              <span>{copiedId === 'api-key-placeholder' ? 'Đã sao chép' : 'Sao chép'}</span>
            </button>
          </div>
          <div className="integration-code-highlight" style={{ fontSize: 14 }}>
            ag-proxy-key
          </div>
          <div className="kpi-sub" style={{ marginTop: 8 }}>
            Gửi qua header: <code>Authorization: Bearer &lt;khóa-api&gt;</code>. Quản lý tại tab "Quản Lý Khóa API".
          </div>
        </div>
      </div>

      {/* Interactive Customizer Bar */}
      <div className="card" style={{ padding: 18 }}>
        <h4 style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 12 }}>
          Tùy Chỉnh Mẫu Code Nhanh (Xem Trước Trực Tiếp)
        </h4>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 14 }}>
          <div>
            <label style={{ display: 'block', fontSize: 12, color: 'var(--text-muted)', marginBottom: 6, fontWeight: 500 }}>
              Khóa API Của Bạn (Tùy chọn điền để cập nhật ví dụ bên dưới):
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
              Mô Hình Mẫu (Model Selection):
            </label>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              style={{ width: '100%' }}
            >
              <option value="ag/gemini-3.8-flash-high">ag/gemini-3.8-flash-high (Google Antigravity - Tốc độ cao & Ngữ cảnh rộng)</option>
              <option value="ag/gemini-pro-agent">ag/gemini-pro-agent (Google Antigravity - Gemini Pro Agent 9router)</option>
              <option value="ag/gemini-3.1-pro-low">ag/gemini-3.1-pro-low (Google Antigravity - Gemini Pro Low Quota)</option>
              <option value="cx/gpt-5.6-sol">cx/gpt-5.6-sol (OpenAI Codex - Chuyên sâu Logic Lập Trình)</option>
            </select>
          </div>
        </div>
      </div>

      {/* Model Examples Reference Card */}
      <div className="card">
        <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconBookOpen size={18} style={{ color: 'var(--accent-primary)' }} />
          <span>Danh Mục Mô Hình Ví Dụ Chuẩn (Supported Model Namespaces)</span>
        </h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: 14 }}>
          <div className="model-spec-card">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="badge badge-success">Google Antigravity</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard('ag/gemini-3.8-flash-high', 'm-ag')}
                title="Sao chép tên model"
              >
                {copiedId === 'm-ag' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'm-ag' ? 'Đã sao chép' : 'Sao chép'}</span>
              </button>
            </div>
            <div className="model-spec-name">ag/gemini-3.8-flash-high</div>
            <p className="model-spec-desc">
              Phù hợp tác vụ trò chuyện thời gian thực, tổng hợp dữ liệu, cửa sổ ngữ cảnh cực lớn, hỗ trợ streaming tốc độ cao và Function Calling (Tools).
            </p>
          </div>

          <div className="model-spec-card">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span className="badge badge-primary">OpenAI Codex</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard('cx/gpt-5.6-sol', 'm-cx')}
                title="Sao chép tên model"
              >
                {copiedId === 'm-cx' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'm-cx' ? 'Đã sao chép' : 'Sao chép'}</span>
              </button>
            </div>
            <div className="model-spec-name">cx/gpt-5.6-sol</div>
            <p className="model-spec-desc">
              Tối ưu cho kỹ thuật lập trình nâng cao, suy luận logic phức tạp, debug mã nguồn, kiến trúc phần mềm và thực thi agent tự động.
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
              Tích Hợp Với Hermes Agent (CLI & Configuration)
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              Hermes Agent tương thích tự nhiên với các router OpenAI-compatible. Bạn có thể định cấu hình qua file <code>~/.hermes/config.yaml</code> hoặc qua các biến môi trường trực tiếp.
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
                  <span>{copiedId === 'hermes-yaml' ? 'Đã sao chép' : 'Sao chép'}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{hermesConfigSnippet}</code>
              </pre>
            </div>

            {/* CLI Environment */}
            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">Terminal (Environment Variables)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(hermesCliEnvSnippet, 'hermes-env')}
                >
                  {copiedId === 'hermes-env' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'hermes-env' ? 'Đã sao chép' : 'Sao chép'}</span>
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
              Tích Hợp Với OpenAI Codex CLI
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              Thiết lập file cấu hình <code>~/.codex/config.toml</code> để điều hướng mọi lệnh gọi từ Codex CLI trực tiếp qua router:
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
                  <span>{copiedId === 'codex-toml' ? 'Đã sao chép' : 'Sao chép'}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{codexConfigSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">Thực Thi Dòng Lệnh (Bash / Zsh)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(codexCliSnippet, 'codex-cli')}
                >
                  {copiedId === 'codex-cli' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'codex-cli' ? 'Đã sao chép' : 'Sao chép'}</span>
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
              Tích Hợp Bằng Thư Viện Chính Thức: OpenAI Python SDK
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              Cài đặt thư viện bằng lệnh: <code>pip install openai</code>. Ví dụ dưới đây minh họa khởi tạo client với Base URL và đọc dữ liệu theo luồng (streaming):
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
                  <span>{copiedId === 'py-sdk' ? 'Đã sao chép' : 'Sao chép'}</span>
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
              Kiểm Tra Nhanh Bằng cURL (Terminal / Shell)
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              Các mẫu câu lệnh cURL chuẩn cho cả chế độ đồng bộ (non-stream) và chế độ luồng (streaming) qua cờ <code>-N</code> (no-buffer):
            </p>

            <div className="code-card">
              <div className="code-card-header">
                <span className="code-file-tag">cURL: Chuẩn Non-Streaming (Nhận JSON hoàn chỉnh)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(curlStandardSnippet, 'curl-std')}
                >
                  {copiedId === 'curl-std' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'curl-std' ? 'Đã sao chép' : 'Sao chép'}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{curlStandardSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">cURL: Chế Độ Streaming SSE (Server-Sent Events với cờ -N)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(curlStreamSnippet, 'curl-stream')}
                >
                  {copiedId === 'curl-stream' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'curl-stream' ? 'Đã sao chép' : 'Sao chép'}</span>
                </button>
              </div>
              <pre className="code-card-pre">
                <code>{curlStreamSnippet}</code>
              </pre>
            </div>

            <div className="code-card" style={{ marginTop: 14 }}>
              <div className="code-card-header">
                <span className="code-file-tag">cURL: Kiểm Tra Danh Sách Mô Hình (GET /v1/models)</span>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => copyToClipboard(curlModelsSnippet, 'curl-models')}
                >
                  {copiedId === 'curl-models' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                  <span>{copiedId === 'curl-models' ? 'Đã sao chép' : 'Sao chép'}</span>
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
              Tích Hợp Trong Hệ Sinh Thái Node.js & TypeScript
            </h4>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              Hỗ trợ cả thư viện <code>openai</code> chính thức lẫn hàm <code>fetch</code> có sẵn trong Node.js phiên bản hiện đại:
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
                  <span>{copiedId === 'node-sdk' ? 'Đã sao chép' : 'Sao chép'}</span>
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
                  <span>{copiedId === 'node-fetch' ? 'Đã sao chép' : 'Sao chép'}</span>
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
          <span>Lưu Ý Về Cơ Chế Streaming (Server-Sent Events)</span>
        </h3>
        <div style={{ fontSize: 13.5, color: 'var(--text-secondary)', lineHeight: 1.6 }}>
          <p style={{ marginBottom: 10 }}>
            Khi đặt thuộc tính <code>"stream": true</code> trong payload yêu cầu, kết nối HTTP sẽ được giữ mở với định dạng <code>Content-Type: text/event-stream</code>.
          </p>
          <ul style={{ paddingLeft: 20, marginBottom: 10, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <li>
              <strong>Độ trễ thấp tối ưu (Low TTFT):</strong> ezRouter được thiết kế với cơ chế zero-buffering forwarding. Từng gói tin SSE từ mô hình nền tảng (Google Antigravity hoặc OpenAI Codex) được đẩy tức thời tới client ngay khi sinh ra mà không cần gom cụm vào bộ nhớ đệm.
            </li>
            <li>
              <strong>Định dạng chunk tiêu chuẩn:</strong> Mỗi chunk văn bản mang cấu trúc chuẩn <code>{'data: {"choices":[{"delta":{"content":"..."}}]}'}</code> với trường <code>choices[0].delta.content</code>.
            </li>
            <li>
              <strong>Tín hiệu hoàn tất:</strong> Khi mô hình sinh xong toàn bộ phản hồi, server sẽ gửi dòng kết thúc <code>data: [DONE]</code> trước khi ngắt kết nối HTTP.
            </li>
            <li>
              <strong>Lưu ý cấu hình mạng trung gian:</strong> Nếu bạn đặt thêm reverse proxy (như Nginx) ở giữa client và ezRouter, hãy cấu hình header <code>X-Accel-Buffering: no</code> và tắt <code>proxy_buffering off;</code> để tránh việc proxy tự ý gom buffer làm giật cục luồng stream.
            </li>
          </ul>
        </div>
      </div>

      {/* Tools & Function Calling Notes */}
      <div className="card">
        <h3 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
          <IconCode size={18} style={{ color: '#a78bfa' }} />
          <span>Lưu Ý Về Tools & Gọi Hàm (Function Calling)</span>
        </h3>
        <div style={{ fontSize: 13.5, color: 'var(--text-secondary)', lineHeight: 1.6 }}>
          <p style={{ marginBottom: 10 }}>
            ezRouter hỗ trợ đầy đủ tính năng Function Calling chuẩn OpenAI API cho cả hai namespace mô hình <code>ag/*</code> và <code>cx/*</code>.
          </p>
          <ul style={{ paddingLeft: 20, marginBottom: 12, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <li>
              <strong>Chuẩn hóa giao thức hai chiều:</strong> Hệ thống tự động dịch chuyển đổi schema giữa giao thức của Google Antigravity và định dạng OpenAI tools. Client chỉ cần gửi payload JSON Schema tiêu chuẩn.
            </li>
            <li>
              <strong>Quy trình phản hồi vòng lặp (Execution Loop):</strong> Khi mô hình quyết định gọi tool, nó sẽ trả về mảng <code>tool_calls</code> (gồm <code>id</code>, <code>name</code>, <code>arguments</code>). Client thực thi code nghiệp vụ tương ứng và gửi tiếp kết quả trở lại API với role <code>"tool"</code> kèm <code>tool_call_id</code>.
            </li>
          </ul>

          <div className="code-card">
            <div className="code-card-header">
              <span className="code-file-tag">Cấu Trúc Payload Khai Báo Tools Mẫu</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => copyToClipboard(toolsSnippet, 'tools-json')}
              >
                {copiedId === 'tools-json' ? <IconCheck size={14} style={{ color: 'var(--success)' }} /> : <IconCopy size={14} />}
                <span>{copiedId === 'tools-json' ? 'Đã sao chép' : 'Sao chép'}</span>
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
          <span>Chẩn Đoán Lỗi Thường Gặp & Cách Khắc Phục (Troubleshooting)</span>
        </h3>

        <div className="troubleshoot-grid">
          {/* 401 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-401">HTTP 401 Unauthorized</div>
            <div className="troubleshoot-title">Khóa API Không Hợp Lệ Hoặc Thiếu Header</div>
            <div className="troubleshoot-desc">
              <strong>Nguyên nhân:</strong> Yêu cầu không có header <code>Authorization: Bearer &lt;key&gt;</code>, hoặc chuỗi khóa API bị sai, có khoảng trắng thừa, hoặc đã bị vô hiệu hóa trong cơ sở dữ liệu.
            </div>
            <div className="troubleshoot-fix">
              <strong>Cách khắc phục:</strong> Kiểm tra lại biến môi trường của bạn. Truy cập tab <em>"Quản Lý Khóa API"</em> trên giao diện quản trị để kiểm tra danh sách khóa đang hoạt động hoặc tạo một khóa mới.
            </div>
          </div>

          {/* 404 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-404">HTTP 404 Not Found / Model Not Found</div>
            <div className="troubleshoot-title">Tên Mô Hình Không Khớp Hoặc Sai Namespace</div>
            <div className="troubleshoot-desc">
              <strong>Nguyên nhân:</strong> Model truyền vào không tồn tại hoặc thiếu tiền tố namespace bắt buộc (ví dụ gõ nhầm <code>gemini-3.8-flash</code> thay vì <code>ag/gemini-3.8-flash-high</code>).
            </div>
            <div className="troubleshoot-fix">
              <strong>Cách khắc phục:</strong> Luôn kiểm tra model có tiền tố hợp lệ (<code>ag/</code> cho Google Antigravity, <code>cx/</code> cho Codex). Kiểm tra danh sách mô hình thực tế đang mở bằng lệnh <code>curl {baseUrl}/models</code>.
            </div>
          </div>

          {/* 429 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-429">HTTP 429 Rate Limit / Cooldown</div>
            <div className="troubleshoot-title">Vượt Quá Tần Suất Yêu Cầu Hoặc Pool Bận</div>
            <div className="troubleshoot-desc">
              <strong>Nguyên nhân:</strong> Tài khoản upstream đang bị Google hoặc OpenAI áp dụng giới hạn tần suất tạm thời, hoặc tài khoản đang trong chu kỳ cooldown.
            </div>
            <div className="troubleshoot-fix">
              <strong>Cách khắc phục:</strong> ezRouter đã tích hợp sẵn cơ chế tự động chuyển tiếp (failover) giữa các tài khoản trong pool. Ở phía client, bạn nên kích hoạt Exponential Backoff with Jitter (chờ 1s, 2s, 4s... rồi retry).
            </div>
          </div>

          {/* 500 / 502 / 503 */}
          <div className="troubleshoot-card">
            <div className="troubleshoot-badge status-500">HTTP 500 / 502 / 503 Upstream Error</div>
            <div className="troubleshoot-title">Lỗi Kết Nối Hoặc Provider Gặp Sự Cố</div>
            <div className="troubleshoot-desc">
              <strong>Nguyên nhân:</strong> Nhà cung cấp dịch vụ gốc gặp sự cố gián đoạn tạm thời, token tài khoản bị từ chối, hoặc máy chủ mạng upstream quá tải.
            </div>
            <div className="troubleshoot-fix">
              <strong>Cách khắc phục:</strong> Kiểm tra tab <em>"Nhà Cung Cấp"</em> để xem trạng thái tài khoản. Thiết lập <em>"Combo"</em> để định cấu hình các mô hình dự phòng (fallback) tự động khi upstream chính không khả dụng.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
