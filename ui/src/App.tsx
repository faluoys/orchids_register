import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { LayoutDashboard, Mail, Settings, UserPlus, Users, FolderTree, Inbox } from "lucide-react";
import CloseAppDialog from "./components/CloseAppDialog";
import {
  cancelClosePrompt,
  confirmExit,
  getServiceStatus,
  stopMailGateway,
  stopTurnstileSolver,
} from "./lib/tauri-api";
import DashboardPage from "./pages/DashboardPage";
import RegisterPage from "./pages/RegisterPage";
import AccountsPage from "./pages/AccountsPage";
import GroupManagePage from "./pages/GroupManagePage";
import DomainManagePage from "./pages/DomainManagePage";
import InboxConfigPage from "./pages/InboxConfigPage";
import SettingsPage from "./pages/SettingsPage";
import type { ManagedServiceName, Page, ServiceStatus } from "./lib/types";

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
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);
  const [closeDialogLoading, setCloseDialogLoading] = useState(false);
  const [closeDialogError, setCloseDialogError] = useState<string | null>(null);
  const [closeDialogBusyService, setCloseDialogBusyService] = useState<ManagedServiceName | null>(null);
  const [closeDialogServices, setCloseDialogServices] = useState<
    Record<ManagedServiceName, ServiceStatus | null>
  >({
    mail_gateway: null,
    turnstile_solver: null,
  });

  const refreshCloseDialogServices = useCallback(async () => {
    setCloseDialogLoading(true);
    setCloseDialogError(null);
    try {
      const statuses = await getServiceStatus();
      setCloseDialogServices({
        mail_gateway: statuses.mail_gateway,
        turnstile_solver: statuses.turnstile_solver,
      });
    } catch (error) {
      setCloseDialogError(String(error));
    } finally {
      setCloseDialogLoading(false);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen("app-close-requested", async () => {
      if (disposed) return;
      setCloseDialogOpen(true);
      await refreshCloseDialogServices();
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [refreshCloseDialogServices]);

  const handleCloseDialogCancel = useCallback(async () => {
    try {
      await cancelClosePrompt();
    } finally {
      setCloseDialogBusyService(null);
      setCloseDialogError(null);
      setCloseDialogOpen(false);
    }
  }, []);

  const handleCloseDialogStopService = useCallback(async (service: ManagedServiceName) => {
    setCloseDialogBusyService(service);
    setCloseDialogError(null);
    try {
      if (service === "mail_gateway") {
        const status = await stopMailGateway();
        setCloseDialogServices((current) => ({ ...current, mail_gateway: status }));
      } else {
        const status = await stopTurnstileSolver();
        setCloseDialogServices((current) => ({ ...current, turnstile_solver: status }));
      }
      await refreshCloseDialogServices();
    } catch (error) {
      setCloseDialogError(String(error));
      await refreshCloseDialogServices();
    } finally {
      setCloseDialogBusyService(null);
    }
  }, [refreshCloseDialogServices]);

  const handleCloseDialogExit = useCallback(async () => {
    await confirmExit();
  }, []);

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

      <CloseAppDialog
        open={closeDialogOpen}
        loading={closeDialogLoading}
        services={closeDialogServices}
        busyService={closeDialogBusyService}
        error={closeDialogError}
        onStopService={(service) => void handleCloseDialogStopService(service)}
        onCancel={() => void handleCloseDialogCancel()}
        onExit={() => void handleCloseDialogExit()}
      />
    </div>
  );
}
