import { useState } from "react";
import { LayoutDashboard, Mail, Settings, UserPlus, Users, FolderTree, Inbox } from "lucide-react";
import DashboardPage from "./pages/DashboardPage";
import RegisterPage from "./pages/RegisterPage";
import AccountsPage from "./pages/AccountsPage";
import GroupManagePage from "./pages/GroupManagePage";
import DomainManagePage from "./pages/DomainManagePage";
import InboxConfigPage from "./pages/InboxConfigPage";
import SettingsPage from "./pages/SettingsPage";
import type { Page } from "./lib/types";

const navItems: { id: Page; label: string; icon: string; Icon: typeof UserPlus }[] = [
  { id: "dashboard", label: "仪表盘", icon: "\uD83D\uDCCA", Icon: LayoutDashboard },
  { id: "register", label: "自动注册", icon: "\u270D", Icon: UserPlus },
  { id: "accounts", label: "账号管理", icon: "\uD83D\uDC64", Icon: Users },
  { id: "groups", label: "分组管理", icon: "\uD83D\uDCC1", Icon: FolderTree },
  { id: "domains", label: "域名管理", icon: "\uD83D\uDCEC", Icon: Mail },
  { id: "inbox_config", label: "收件配置", icon: "\uD83D\uDCE5", Icon: Inbox },
  { id: "settings", label: "系统设置", icon: "\u2699", Icon: Settings },
];

export default function App() {
  const [currentPage, setCurrentPage] = useState<Page>("register");

  return (
    <div className="app-layout">
      {/* Sidebar (Codex-style) */}
      <nav className="sidebar">
        <div className="sidebar-header">
          <div className="sidebar-title">Orchids Register</div>
        </div>
        <ul className="nav-list">
          {navItems.map((item) => (
            <li
              key={item.id}
              className={`nav-item ${currentPage === item.id ? "active" : ""}`}
              onClick={() => setCurrentPage(item.id)}
            >
              <span style={{ width: 18, textAlign: "center", fontSize: 14 }}>
                {item.icon}
              </span>
              <span style={{ whiteSpace: "nowrap" }}>{item.label}</span>
            </li>
          ))}
        </ul>
        <div style={{ flex: 1 }} />
        <div
          style={{
            padding: "16px",
            fontSize: 11,
            color: "var(--muted)",
            opacity: 0.6,
          }}
        >
          v0.1.0
        </div>
      </nav>

      {/* Content area (Codex-style) */}
      <main className="content-area">
        {/* All pages stay mounted; inactive ones are hidden via CSS */}
        <div className="page" style={{ display: currentPage === "dashboard" ? undefined : "none" }}>
          <DashboardPage />
        </div>
        <div className="page" style={{ display: currentPage === "register" ? undefined : "none" }}>
          <RegisterPage />
        </div>
        <div className="page" style={{ display: currentPage === "accounts" ? undefined : "none" }}>
          <AccountsPage />
        </div>
        <div className="page" style={{ display: currentPage === "groups" ? undefined : "none" }}>
          <GroupManagePage />
        </div>
        <div className="page" style={{ display: currentPage === "domains" ? undefined : "none" }}>
          <DomainManagePage />
        </div>
        <div className="page" style={{ display: currentPage === "inbox_config" ? undefined : "none" }}>
          <InboxConfigPage />
        </div>
        <div className="page" style={{ display: currentPage === "settings" ? undefined : "none" }}>
          <SettingsPage />
        </div>
      </main>
    </div>
  );
}
