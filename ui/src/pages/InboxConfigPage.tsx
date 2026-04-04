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
import type { MailGatewayHealthResult, ServiceSource, ServiceStatus } from "@/lib/types";

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
  mail_chatgpt_uk_base_url: "https://mail.chatgpt.org.uk",
  mail_chatgpt_uk_api_key: "",
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
  "mail_chatgpt_uk_base_url",
  "mail_chatgpt_uk_api_key",
] as const;

const CLEARABLE_KEYS = new Set<string>([
  "mail_gateway_api_key",
  "mail_project_code",
  "mail_domain",
  "luckmail_api_key",
  "yyds_api_key",
  "mail_chatgpt_uk_api_key",
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
    mail_chatgpt_uk_base_url:
      config["mail_chatgpt_uk_base_url"] || DEFAULTS.mail_chatgpt_uk_base_url,
    mail_chatgpt_uk_api_key: config["mail_chatgpt_uk_api_key"] || DEFAULTS.mail_chatgpt_uk_api_key,
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

function describeServiceSource(source: ServiceSource | undefined): string {
  if (source === "desktop_managed") {
    return "桌面托管";
  }
  if (source === "external") {
    return "外部运行";
  }
  return "未启动";
}

function buildMailGatewayWarnings(configs: Record<string, string>): string[] {
  const warnings: string[] = [];
  if (!(configs["mail_gateway_host"] || "").trim()) {
    warnings.push("还没填写 Mail Gateway Host。");
  }
  if (!(configs["mail_gateway_port"] || "").trim()) {
    warnings.push("还没填写 Mail Gateway Port。");
  }
  if (!(configs["mail_gateway_database_path"] || "").trim()) {
    warnings.push("还没填写数据库路径。");
  }

  const provider = (configs["mail_provider"] || "").trim().toLowerCase();
  if (provider === "luckmail" && !(configs["luckmail_api_key"] || "").trim()) {
    warnings.push("当前选择的是 LuckMail，但还没填写 LuckMail API Key。");
  }
  if (provider === "yyds_mail" && !(configs["yyds_api_key"] || "").trim()) {
    warnings.push("当前选择的是 YYDS Mail，但还没填写 YYDS API Key。");
  }
  if (provider === "mail_chatgpt_uk" && !(configs["mail_chatgpt_uk_api_key"] || "").trim()) {
    warnings.push("当前选择的是 GPTMail（provider: mail_chatgpt_uk），但还没填写 API Key。");
  }
  if ((configs["mail_chatgpt_uk_api_key"] || "").trim() && provider !== "mail_chatgpt_uk") {
    warnings.push("已填写 GPTMail API Key；如果要使用它，请把 Provider 改成 `mail_chatgpt_uk`。");
  }
  if (provider === "mail_chatgpt_uk" && (configs["mail_provider_mode"] || "").trim() !== "persistent") {
    warnings.push("使用 GPTMail 时，Provider Mode 应设置为 `persistent`。");
  }
  return warnings;
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
  const guidanceWarnings = buildMailGatewayWarnings(configs);

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
                <span>来源: {describeServiceSource(serviceStatus?.source)}</span>
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
            <div className="guidance-card">
              <div className="guidance-title">桌面端配置提示</div>
              <div className="guidance-copy">
                这里已经是 Mail Gateway 的主配置入口。桌面版不再要求你先去改
                <code> runtime.local.yaml </code>
                ，只有在跑旧脚本或 CLI 时才需要回去看 YAML。
              </div>
              <ul className="guidance-list">
                <li>首次配置先填 Host、Port、Database Path，再补对应邮箱供应商的 API Key。</li>
                <li>保存后先点“启动”，服务起来之后再点“健康检查”，再去跑注册。</li>
                <li>健康检查失败时，优先看 Provider Key、Base URL、端口占用和来源状态。</li>
              </ul>
              {guidanceWarnings.length > 0 ? (
                <div className="guidance-warning">{guidanceWarnings.join(" ")}</div>
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
            <div className="form-group">
              <label>YYDS API Key</label>
              <input
                type="password"
                value={configs["yyds_api_key"] || ""}
                onChange={(event) => updateConfig("yyds_api_key", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
            </div>
            <div className="form-group">
              <label>GPTMail Base URL</label>
              <input
                type="text"
                value={configs["mail_chatgpt_uk_base_url"] || ""}
                onChange={(event) => updateConfig("mail_chatgpt_uk_base_url", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
              <div className="settings-hint">对应 provider 标识是 `mail_chatgpt_uk`。</div>
            </div>
            <div className="form-group" style={{ marginBottom: 0 }}>
              <label>GPTMail API Key</label>
              <input
                type="password"
                value={configs["mail_chatgpt_uk_api_key"] || ""}
                onChange={(event) => updateConfig("mail_chatgpt_uk_api_key", event.target.value)}
                className="input"
                style={{ width: "100%" }}
              />
              <div className="settings-hint">使用 GPTMail 时，同时把 Provider 设为 `mail_chatgpt_uk`、Provider Mode 设为 `persistent`。</div>
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">注册使用的收件参数</div>
            <div className="guidance-card" style={{ marginBottom: 14 }}>
              <div className="guidance-title">这一组参数决定注册时怎么取邮箱</div>
              <div className="guidance-copy">
                常用组合现在有三种：
                <code> yyds_mail + persistent </code>
                <code> mail_chatgpt_uk + persistent </code>
                <code> luckmail + purchased </code>
                。其中 `mail_chatgpt_uk` 对应的就是 GPTMail。如果你没有特殊需求，`Project Code` 通常保持
                <code> orchids </code>
                ，域名留空即可。
              </div>
            </div>
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
              <div className="settings-hint">桌面端当前主流程用的是 `gateway`，通常不需要改成 `manual`。</div>
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
              <div className="settings-hint">
                填写 mail-gateway 里要使用的 provider，例如 `yyds_mail`、`luckmail`、`mail_chatgpt_uk`。如果使用 GPTMail，这里填 `mail_chatgpt_uk`。
              </div>
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
              <div className="settings-hint">
                `persistent` 多用于 `yyds_mail`/`mail_chatgpt_uk`（GPTMail），`purchased` 多用于 `luckmail`。
              </div>
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
              <div className="settings-hint">通常填写 `orchids`，除非你的 mail-gateway 侧明确要求用别的项目代号。</div>
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
              <div className="settings-hint">留空时由 mail-gateway/provider 自行选择，只有你想固定后缀时才填写。</div>
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
