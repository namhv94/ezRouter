import React, { useState, useEffect, useCallback } from 'react';
import {
  AdminStats,
  ApiKey,
  HealthInfo,
  ModelRequestSummary,
  ProviderResponse,
  AccountResponse,
  CodexAccountRecord,
} from './types';
import {
  api,
  getStoredAdminKey,
  clearStoredAdminKey,
} from './api';
import { Sidebar, TabType } from './components/Sidebar';
import { Navbar } from './components/Navbar';
import { LoginModal } from './components/LoginModal';
import { OverviewTab } from './components/OverviewTab';
import { PlaygroundTab } from './components/PlaygroundTab';
import { IntegrationTab } from './components/IntegrationTab';
import { RequestsTab } from './components/RequestsTab';
import { ProvidersTab } from './components/ProvidersTab';
import { CombosTab } from './components/CombosTab';
import { ApiKeysTab } from './components/ApiKeysTab';
import { TokenSaverTab } from './components/TokenSaverTab';
import { ConsoleLogTab } from './components/ConsoleLogTab';

export const App: React.FC = () => {
  const [adminKey, setAdminKey] = useState<string>(getStoredAdminKey());
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(false);
  const [initialChecking, setInitialChecking] = useState<boolean>(true);

  const [currentTab, setCurrentTab] = useState<TabType>('overview');
  const [sidebarOpen, setSidebarOpen] = useState<boolean>(false);

  // Core Data
  const [health, setHealth] = useState<HealthInfo | null>(null);
  const [stats, setStats] = useState<AdminStats | null>(null);
  const [summaries, setSummaries] = useState<ModelRequestSummary[]>([]);
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [accounts, setAccounts] = useState<AccountResponse[]>([]);
  const [codexAccounts, setCodexAccounts] = useState<CodexAccountRecord[]>([]);
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);

  const [loading, setLoading] = useState<boolean>(false);
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Initial authentication check
  useEffect(() => {
    const key = getStoredAdminKey();
    if (!key) {
      setInitialChecking(false);
      setIsAuthenticated(false);
      return;
    }

    api
      .getStats(key)
      .then((res) => {
        setStats(res);
        setAdminKey(key);
        setIsAuthenticated(true);
      })
      .catch(() => {
        clearStoredAdminKey();
        setIsAuthenticated(false);
      })
      .finally(() => {
        setInitialChecking(false);
      });
  }, []);

  const refreshAllData = useCallback(async () => {
    if (!isAuthenticated) return;
    setRefreshing(true);
    setError(null);
    try {
      const [healthRes, statsRes, summariesRes, providersRes, accountsRes, keysRes, codexRes] =
        await Promise.allSettled([
          api.getHealth(),
          api.getStats(),
          api.getRequestSummary(),
          api.getProviders(),
          api.getAccounts(),
          api.getApiKeys(),
          api.getCodexAccounts(),
        ]);

      if (healthRes.status === 'fulfilled') setHealth(healthRes.value);
      if (statsRes.status === 'fulfilled') setStats(statsRes.value);
      if (summariesRes.status === 'fulfilled') setSummaries(summariesRes.value);
      if (providersRes.status === 'fulfilled') setProviders(providersRes.value);
      if (accountsRes.status === 'fulfilled') setAccounts(accountsRes.value.accounts || []);
      if (keysRes.status === 'fulfilled') setApiKeys(keysRes.value);
      if (codexRes.status === 'fulfilled') {
        const cVal = codexRes.value;
        const list = Array.isArray(cVal) ? cVal : (cVal as any)?.accounts || [];
        setCodexAccounts(list);
      }

      // Check if critical stats failed with auth error
      if (statsRes.status === 'rejected') {
        const err = statsRes.reason;
        if (err?.status === 401) {
          clearStoredAdminKey();
          setIsAuthenticated(false);
        } else {
          setError(err?.message || 'Không thể đồng bộ số liệu thống kê.');
        }
      }
    } catch (err: any) {
      setError(err?.message || 'Lỗi khi làm mới dữ liệu.');
    } finally {
      setRefreshing(false);
      setLoading(false);
    }
  }, [isAuthenticated]);

  useEffect(() => {
    if (isAuthenticated) {
      setLoading(true);
      refreshAllData();
    }
  }, [isAuthenticated, refreshAllData]);

  const handleLoginSuccess = (key: string) => {
    setAdminKey(key);
    setIsAuthenticated(true);
  };

  const handleLogout = () => {
    clearStoredAdminKey();
    setAdminKey('');
    setIsAuthenticated(false);
    setStats(null);
  };

  const tabTitles: Record<TabType, string> = {
    overview: 'Tổng quan',
    integration: 'Tích hợp',
    traffic: 'Lưu lượng',
    playground: 'Thử nghiệm',
    requests: 'Lưu lượng',
    providers: 'Nhà cung cấp',
    accounts: 'Tài khoản Antigravity',
    codex: 'Tài khoản Codex',
    combos: 'Combo',
    api_keys: 'Khóa API',
    token_saver: 'Token Saver',
    logs: 'Nhật ký hệ thống',
  };

  if (initialChecking) {
    return (
      <div
        className="state-container"
        style={{ minHeight: '100vh', justifyContent: 'center' }}
      >
        <div className="spinner" />
        <p style={{ marginTop: 12 }}>Đang kết nối ezRouter...</p>
      </div>
    );
  }

  if (!isAuthenticated) {
    return <LoginModal onSuccess={handleLoginSuccess} />;
  }

  const activeAccountsCount = accounts.filter(
    (a) => a.is_active && a.cooldown_remaining <= 0
  ).length;

  return (
    <div className="app-container">
      <Sidebar
        currentTab={currentTab}
        onSelectTab={setCurrentTab}
        isOpen={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
        health={health}
        activeAccountsCount={activeAccountsCount}
        totalProvidersCount={providers.length}
        totalKeysCount={apiKeys.length}
      />

      <div className="main-wrapper">
        <Navbar
          title={tabTitles[currentTab]}
          onToggleSidebar={() => setSidebarOpen(!sidebarOpen)}
          onRefresh={refreshAllData}
          refreshing={refreshing}
          adminKey={adminKey}
          onLogout={handleLogout}
        />

        <main className="content-body">
          {currentTab === 'overview' && (
            <OverviewTab
              stats={stats}
              summaries={summaries}
              accounts={accounts}
              codexAccounts={codexAccounts}
              health={health}
              loading={loading}
              refreshing={refreshing}
              onRefresh={refreshAllData}
              error={error}
              onSelectTab={setCurrentTab}
            />
          )}

          {currentTab === 'integration' && <IntegrationTab />}

          {currentTab === 'playground' && (
            <PlaygroundTab adminKey={adminKey} />
          )}

          {(currentTab === 'traffic' || currentTab === 'requests') && <RequestsTab />}

          {(currentTab === 'providers' || currentTab === 'accounts' || currentTab === 'codex') && (
            <ProvidersTab
              providers={providers}
              accounts={accounts}
              loading={loading}
              error={error}
              onRefresh={refreshAllData}
            />
          )}

          {currentTab === 'combos' && <CombosTab />}

          {currentTab === 'token_saver' && <TokenSaverTab />}

          {currentTab === 'api_keys' && (
            <ApiKeysTab
              apiKeys={apiKeys}
              loading={loading}
              error={error}
              onRefresh={refreshAllData}
            />
          )}

          {currentTab === 'logs' && <ConsoleLogTab />}
        </main>
      </div>
    </div>
  );
};
