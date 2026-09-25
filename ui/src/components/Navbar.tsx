import React from 'react';
import { IconMenu, IconRefresh, IconLogOut, IconKey, EzRouterMark } from '../icons';

interface NavbarProps {
  title: string;
  onToggleSidebar: () => void;
  onRefresh: () => void;
  refreshing: boolean;
  adminKey: string;
  onLogout: () => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  title,
  onToggleSidebar,
  onRefresh,
  refreshing,
  adminKey,
  onLogout,
}) => {
  const maskedKey =
    adminKey.length > 8
      ? `${adminKey.slice(0, 4)}••••${adminKey.slice(-4)}`
      : '••••••••';

  return (
    <header className="top-navbar">
      <div className="nav-left">
        <button
          className="menu-toggle-btn"
          onClick={onToggleSidebar}
          aria-label="Mở danh mục điều hướng"
        >
          <IconMenu size={20} />
        </button>
        <div className="nav-title-group" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="mobile-brand-mark" style={{ display: 'none' }}>
            <EzRouterMark size={22} />
          </span>
          <h1 className="page-title">{title}</h1>
        </div>
      </div>

      <div className="nav-right">
        <div
          className="badge badge-neutral nav-key-badge"
          title={`Khóa quản trị hiện tại: ${maskedKey}`}
        >
          <IconKey size={14} style={{ color: 'var(--accent-primary)' }} />
          <span style={{ fontSize: 12 }}>{maskedKey}</span>
        </div>

        <button
          className="btn btn-secondary btn-sm nav-action-btn"
          onClick={onRefresh}
          disabled={refreshing}
          title="Làm mới dữ liệu"
          aria-label="Làm mới dữ liệu"
        >
          <IconRefresh size={16} className={refreshing ? 'spinner' : ''} />
          <span className="btn-label">Làm mới</span>
        </button>

        <button
          className="btn btn-secondary btn-sm nav-action-btn"
          onClick={onLogout}
          title="Đăng xuất khỏi phiên quản trị"
          aria-label="Đăng xuất khỏi phiên quản trị"
        >
          <IconLogOut size={16} />
          <span className="btn-label">Đăng xuất</span>
        </button>
      </div>
    </header>
  );
};
