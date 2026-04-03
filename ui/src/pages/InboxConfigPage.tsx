import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Loader2 } from "lucide-react";
import {
  getAllConfig,
  getServiceStatus,
  saveConfig,
  startMailGateway,
  stopMailGateway,
  testMailGatewayHealth,
} from "@/lib/tauri-api";
import type { MailGatewayHealthResult, ServiceStatus } from "@/lib/types";

const DEFAULTS = {
  mail_mode: "gateway",
  mail_gateway_host: "127.0.0.1",
  mail_gateway_port: "8081",
  mail_gateway_database_path: "mail-gateway/data/mail_gateway.db",
  mail_gateway_api_key: "",
  mail_provider: "luckmail",
  mail_provider_mode: "purchased",
  mail_project_code: "orchids",
  mail_domain: "",
  luckmail_base_url: "https://mails.luckyous.com",
  luckmail_api_key: "",
  yyds_base_url: "https://maliapi.215.im/v1",
  yyds_api_key: "",
} as const;

const SAVE_KEYS = [
  "mail_mode",
  "mail_gateway_host",
  "mail_gateway_port",
  "mail_gateway_database_path",
  "mail_gateway_base_url",
  "mail_gateway_api_key",
  "mail_provider",
  "mail_provider_mode",
  "mail_project_code",
  "mail_domain",
  "luckmail_base_url",
  "luckmail_api_key",
  "yyds_base_url",
  "yyds_api_key",
] as const;

const CLEARABLE_KEYS = new Set<string>([
  "mail_gateway_api_key",
  "mail_project_code",
  "mail_domain",
  "luckmail_api_key",
  "yyds_api_key",
]);

function deriveMailGatewayBaseUrl(configs: Record<string, string>): string {
  const host = (configs["mail_gateway_host"] || DEFAULTS.mail_gateway_host).trim();
  const port = (configs["mail_gateway_port"] || DEFAULTS.mail_gateway_port).trim();
  return `http://${host}:${port}`;
}

function normalizeConfigs(config: Record<string, string>): Record<string, string> {
  const next = {
    ...config,
    mail_mode: config["mail_mode"] || DEFAULTS.mail_mode,
    mail_gateway_host: config["mail_gateway_host"] || DEFAULTS.mail_gateway_host,
    mail_gateway_port: config["mail_gateway_port"] || DEFAULTS.mail_gateway_port,
    mail_gateway_database_path:
      config["mail_gateway_database_path"] || DEFAULTS.mail_gateway_database_path,
    mail_gateway_api_key: config["mail_gateway_api_key"] || DEFAULTS.mail_gateway_api_key,
    mail_provider: config["mail_provider"] || DEFAULTS.mail_provider,
    mail_provider_mode: config["mail_provider_mode"] || DEFAULTS.mail_provider_mode,
    mail_project_code: config["mail_project_code"] || DEFAULTS.mail_project_code,
    mail_domain: config["mail_domain"] || DEFAULTS.mail_domain,
    luckmail_base_url: config["luckmail_base_url"] || DEFAULTS.luckmail_base_url,
    luckmail_api_key: config["luckmail_api_key"] || DEFAULTS.luckmail_api_key,
    yyds_base_url: config["yyds_base_url"] || DEFAULTS.yyds_base_url,
    yyds_api_key: config["yyds_api_key"] || DEFAULTS.yyds_api_key,
  };

  return {
    ...next,
    mail_gateway_base_url: deriveMailGatewayBaseUrl(next),
  };
}

function buildSavePayload(configs: Record<string, string>): Record<string, string> {
  const payload: Record<string, string> = {
    mail_gateway_base_url: deriveMailGatewayBaseUrl(configs),
  };

  for (const key of SAVE_KEYS) {
    const value = (configs[key] || "").trim();
    if (value || CLEARABLE_KEYS.has(key)) {
      payload[key] = value;
    }
  }

  return payload;
}

export default function InboxConfigPage() {
  const [configs, setConfigs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");
  const [serviceStatus, setServiceStatus] = useState<ServiceStatus | null>(null);
  const [serviceBusy, setServiceBusy] = useState<"start" | "stop" | "restart" | null>(null);
  const [serviceError, setServiceError] = useState<string | null>(null);
  const [healthTesting, setHealthTesting] = useState(false);
  const [healthResult, setHealthResult] = useState<MailGatewayHealthResult | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const refreshServiceStatus = useCallback(async () => {
    try {
      const statuses = await getServiceStatus();
      setServiceStatus(statuses.mail_gateway);
    } catch (error) {
      setServiceError(String(error));
    }
  }, []);

  const persistConfig = useCallback(async (next: Record<string, string>) => {
    await saveConfig(buildSavePayload(next));
  }, []);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const config = await getAllConfig();
      const next = normalizeConfigs(config);
      setConfigs(next);
      await persistConfig(next);
      await refreshServiceStatus();
    } catch (error) {
      console.error("加载收件配置失败:", error);
      setServiceError(String(error));
    } finally {
      setLoading(false);
    }
  }, [persistConfig, refreshServiceStatus]);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const autoSave = useCallback((next: Record<string, string>) => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(async () => {
      setSaveStatus("saving");
      try {
        await persistConfig(next);
        setSaveStatus("saved");
        setTimeout(() => setSaveStatus("idle"), 1500);
      } catch (error) {
        console.error("自动保存收件配置失败:", error);
        setSaveStatus("idle");
      }
    }, 600);
  }, [persistConfig]);

  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, []);

  const updateConfig = (key: string, value: string) => {
    const next = normalizeConfigs({ ...configs, [key]: value });
    setConfigs(next);
    autoSave(next);
  };

  const runServiceAction = async (action: "start" | "stop" | "restart") => {
    setServiceBusy(action);
    setServiceError(null);
    try {
      await persistConfig(configs);
      if (action === "start") {
        setServiceStatus(await startMailGateway());
      } else if (action === "stop") {
        setServiceStatus(await stopMailGateway());
      } else {
        await stopMailGateway();
        setServiceStatus(await startMailGateway());
      }
      await refreshServiceStatus();
    } catch (error) {
      setServiceError(String(error));
      await refreshServiceStatus();
    } finally {
      setServiceBusy(null);
    }
  };

  const runHealthCheck = async () => {
    const baseUrl = deriveMailGatewayBaseUrl(configs);
    const apiKey = (configs["mail_gateway_api_key"] || "").trim() || null;
    setHealthTesting(true);
    setHealthResult(null);
    setHealthError(null);
    try {
      const result = await testMailGatewayHealth(baseUrl, apiKey);
      setHealthResult(result);
    } catch (error) {
      setHealthError(String(error));
    } finally {
      setHealthTesting(false);
    }
  };

  if (loading) {
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%" }}>
        <Loader2 className="animate-spin" size={24} style={{ color: "var(--accent)" }} />
      </div>
    );
  }

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap", flexShrink: 0 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
          <h1 className="page-title">收件配置</h1>
          <span className="page-subtitle">Mail Gateway 运行时与注册参数</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6, marginLeft: "auto", fontSize: 12 }}>
          {saveStatus === "saving" && (
            <span style={{ color: "var(--muted)", display: "flex", alignItems: "center", gap: 4 }}>
              <Loader2 size={12} className="animate-spin" />
              保存中...
            </span>
          )}
          {saveStatus === "saved" && (
            <span style={{ color: "var(--ok)", display: "flex", alignItems: "center", gap: 4 }}>
              <Check size={12} />
              已保存
            </span>
          )}
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        <div className="settings-grid">
          <div className="config-panel">
            <div className="service-card">
              <div className="service-card-head">
                <div>
                  <div className="settings-title" style={{ marginBottom: 4 }}>
                    Mail Gateway 服务
                  </div>
                  <div className="settings-hint" style={{ marginTop: 0 }}>
                    由桌面端直接托管，不再依赖 runtime.local.yaml
                  </div>
                </div>
                <span className={`service-pill ${serviceStatus?.running ? "running" : "stopped"}`}>
                  {serviceStatus?.running ? "运行中" : "未启动"}
                </span>
              </div>

              <div className="service-meta">
                <span>Base URL: {deriveMailGatewayBaseUrl(configs)}</span>
                <span>PID: {serviceStatus?.pid ?? "-"}</span>
                <span>最近启动: {serviceStatus?.last_started_at || "-"}</span>
              </div>

              <div className="service-actions">
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={serviceBusy !== null}
                  onClick={() => void runServiceAction("start")}
                >
                  {serviceBusy === "start" ? <Loader2 size={12} className="animate-spin" /> : null}
                  启动
                </button>
                <button
                  type="button"
                  className="btn btn-sm btn-clear"
                  disabled={serviceBusy !== null}
                  onClick={() => void runServiceAction("restart")}
                >
                  {serviceBusy === "restart" ? <Loader2 size={12} className="animate-spin" /> : null}
                  重启
                </button>
                <button
                  type="button"
                  className="btn btn-sm btn-danger"
                  disabled={serviceBusy !== null}
                  onClick={() => void runServiceAction("stop")}
                >
                  {serviceBusy === "stop" ? <Loader2 size={12} className="animate-spin" /> : null}
                  停止
                </button>
                <button
                  type="button"
                  className="btn btn-sm btn-clear"
                  disabled={healthTesting}
                  onClick={() => void runHealthCheck()}
                >
                  {healthTesting ? <Loader2 size={12} className="animate-spin" /> : null}
                  健康检查
                </button>
              </div>

              {serviceError ? (
                <div className="service-error">{serviceError}</div>
              ) : null}
              {serviceStatus?.last_error ? (
                <div className="service-error">{serviceStatus.last_error}</div>
              ) : null}
              {healthError ? (
                <div className="service-error">{healthError}</div>
              ) : null}
              {healthResult ? (
                <div className="service-ok">
                  健康检查通过: status={healthResult.status}, timestamp={healthResult.timestamp}
                </div>
              ) : null}
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">Mail Gateway 运行参数</div>
            <div className="form-row">
              <div className="form-group" style={{ flex: 1 }}>
                <label>Host</label>
                <input
                  type="text"
                  value={configs["mail_gateway_host"] || ""}
                  onChange={(event) => updateConfig("mail_gateway_host", event.target.value)}
                  className="input"
                />
              </div>
              <div className="form-group" style={{ width: 160 }}>
                <label>Port</label>
                <input
                  type="text"
                  value={configs["mail_gateway_port"] || ""}
                  onChange={(event) => updateConfig("mail_gateway_port", event.target.value)}
                  className="input"
                />
              </div>
            </div>
            <div className="form-group">
              <label>数据库路径</label>
              <input
                type="text"
                value={configs["mail_gateway_database_path"] || ""}
                onChange={(event) => updateConfig("mail_gateway_database_path", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
              <div className="settings-hint">相对路径会按仓库根目录解析。</div>
            </div>
            <div className="form-group">
              <label>桌面端使用的 Base URL</label>
              <input
                type="text"
                value={deriveMailGatewayBaseUrl(configs)}
                readOnly
                className="input"
                style={{ width: "100%", background: "rgba(255,255,255,0.72)" }}
              />
              <div className="settings-hint">由 Host + Port 自动生成，并同步给注册流程使用。</div>
            </div>
            <div className="form-group">
              <label>Gateway Client API Key</label>
              <input
                type="password"
                value={configs["mail_gateway_api_key"] || ""}
                onChange={(event) => updateConfig("mail_gateway_api_key", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
              <div className="settings-hint">当前若网关没有启用鉴权，可以留空。</div>
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">Provider 源配置</div>
            <div className="form-group">
              <label>LuckMail Base URL</label>
              <input
                type="text"
                value={configs["luckmail_base_url"] || ""}
                onChange={(event) => updateConfig("luckmail_base_url", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
            </div>
            <div className="form-group">
              <label>LuckMail API Key</label>
              <input
                type="password"
                value={configs["luckmail_api_key"] || ""}
                onChange={(event) => updateConfig("luckmail_api_key", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
            </div>
            <div className="form-group">
              <label>YYDS Base URL</label>
              <input
                type="text"
                value={configs["yyds_base_url"] || ""}
                onChange={(event) => updateConfig("yyds_base_url", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
            </div>
            <div className="form-group" style={{ marginBottom: 0 }}>
              <label>YYDS API Key</label>
              <input
                type="password"
                value={configs["yyds_api_key"] || ""}
                onChange={(event) => updateConfig("yyds_api_key", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">注册使用的收件参数</div>
            <div className="form-group">
              <label>邮件模式</label>
              <select
                value={configs["mail_mode"] || DEFAULTS.mail_mode}
                onChange={(event) => updateConfig("mail_mode", event.target.value)}
                className="input"
                style={{ width: 220 }}
              >
                <option value="gateway">gateway</option>
                <option value="manual">manual</option>
              </select>
            </div>
            <div className="form-group">
              <label>Provider</label>
              <input
                type="text"
                value={configs["mail_provider"] || ""}
                onChange={(event) => updateConfig("mail_provider", event.target.value)}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group">
              <label>Provider Mode</label>
              <input
                type="text"
                value={configs["mail_provider_mode"] || ""}
                onChange={(event) => updateConfig("mail_provider_mode", event.target.value)}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group">
              <label>Project Code</label>
              <input
                type="text"
                value={configs["mail_project_code"] || ""}
                onChange={(event) => updateConfig("mail_project_code", event.target.value)}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group" style={{ marginBottom: 0 }}>
              <label>指定域名</label>
              <input
                type="text"
                value={configs["mail_domain"] || ""}
                onChange={(event) => updateConfig("mail_domain", event.target.value)}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
              <div className="settings-hint">留空时由 mail-gateway/provider 自行选择。</div>
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">Provider 健康状态</div>
            <div className="settings-hint" style={{ marginBottom: 10 }}>
              健康检查会读取当前服务返回的 provider 可用状态。
            </div>
            <div style={{ display: "grid", gap: 8 }}>
              {healthResult ? (
                Object.entries(healthResult.providers).map(([provider, status]) => (
                  <div key={provider} className="service-provider-row">
                    <span>{provider}</span>
                    <span style={{ color: status === "enabled" ? "var(--ok)" : "var(--muted)" }}>
                      {status}
                    </span>
                  </div>
                ))
              ) : (
                <div className="settings-hint">尚未执行健康检查。</div>
              )}
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
