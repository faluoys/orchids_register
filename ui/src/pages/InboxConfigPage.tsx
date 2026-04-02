import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Loader2 } from "lucide-react";
import { getAllConfig, saveConfig, testMailGatewayHealth } from "@/lib/tauri-api";
import type { MailGatewayHealthResult } from "@/lib/types";

const DEFAULT_MAIL_MODE = "gateway";
const DEFAULT_MAIL_GATEWAY_BASE_URL = "http://127.0.0.1:8081";
const DEFAULT_MAIL_PROVIDER = "luckmail";
const DEFAULT_MAIL_PROVIDER_MODE = "purchased";
const DEFAULT_MAIL_PROJECT_CODE = "orchids";
const MANAGED_CONFIG_KEYS = [
  "mail_mode",
  "mail_gateway_base_url",
  "mail_gateway_api_key",
  "mail_provider",
  "mail_provider_mode",
  "mail_project_code",
  "mail_domain",
] as const;
const CLEARABLE_CONFIG_KEYS = new Set<string>([
  "mail_gateway_api_key",
  "mail_project_code",
  "mail_domain",
]);

export default function InboxConfigPage() {
  const [configs, setConfigs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");
  const [healthTesting, setHealthTesting] = useState(false);
  const [healthResult, setHealthResult] = useState<MailGatewayHealthResult | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const config = await getAllConfig();
      const next = {
        ...config,
        mail_mode: config["mail_mode"] || DEFAULT_MAIL_MODE,
        mail_gateway_base_url: config["mail_gateway_base_url"] || DEFAULT_MAIL_GATEWAY_BASE_URL,
        mail_gateway_api_key: config["mail_gateway_api_key"] || "",
        mail_provider: config["mail_provider"] || DEFAULT_MAIL_PROVIDER,
        mail_provider_mode: config["mail_provider_mode"] || DEFAULT_MAIL_PROVIDER_MODE,
        mail_project_code: config["mail_project_code"] || DEFAULT_MAIL_PROJECT_CODE,
        mail_domain: config["mail_domain"] || "",
      };
      setConfigs(next);
      await saveConfig({
        mail_mode: next.mail_mode,
        mail_gateway_base_url: next.mail_gateway_base_url,
        mail_gateway_api_key: next.mail_gateway_api_key,
        mail_provider: next.mail_provider,
        mail_provider_mode: next.mail_provider_mode,
        mail_project_code: next.mail_project_code,
        mail_domain: next.mail_domain,
      });
    } catch (e) {
      console.error("加载收件配置失败:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const autoSave = useCallback((newConfigs: Record<string, string>) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      setSaveStatus("saving");
      try {
        const toSave: Record<string, string> = {};
        for (const key of MANAGED_CONFIG_KEYS) {
          const value = (newConfigs[key] || "").trim();
          if (value || CLEARABLE_CONFIG_KEYS.has(key)) {
            toSave[key] = value;
          }
        }
        await saveConfig(toSave);
        setSaveStatus("saved");
        setTimeout(() => setSaveStatus("idle"), 1500);
      } catch (e) {
        console.error("自动保存失败:", e);
        setSaveStatus("idle");
      }
    }, 600);
  }, []);

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  const updateConfig = (key: string, value: string) => {
    const next = { ...configs, [key]: value };
    setConfigs(next);
    autoSave(next);
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
        <h1 className="page-title">收件配置</h1>
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
            <div className="settings-title">Mail Gateway 配置</div>
            <div className="form-group">
              <label>邮件模式</label>
              <select
                value={configs["mail_mode"] || DEFAULT_MAIL_MODE}
                onChange={(e) => updateConfig("mail_mode", e.target.value)}
                className="input"
                style={{ width: 220 }}
              >
                <option value="gateway">gateway</option>
                <option value="manual">manual</option>
              </select>
              <div className="settings-hint">
                当前主活动路径应使用 gateway；manual 仅用于手动提供邮箱。
              </div>
            </div>
            <div className="form-group">
              <label>Base URL</label>
              <div style={{ display: "flex", alignItems: "center", gap: 8, maxWidth: 760 }}>
                <input
                  type="text"
                  value={configs["mail_gateway_base_url"] || ""}
                  onChange={(e) => updateConfig("mail_gateway_base_url", e.target.value)}
                  placeholder={DEFAULT_MAIL_GATEWAY_BASE_URL}
                  className="input"
                  style={{ flex: 1, minWidth: 0 }}
                />
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={healthTesting}
                  onClick={async () => {
                    const baseUrl = (configs["mail_gateway_base_url"] || DEFAULT_MAIL_GATEWAY_BASE_URL).trim();
                    const apiKey = (configs["mail_gateway_api_key"] || "").trim() || null;
                    setHealthTesting(true);
                    setHealthResult(null);
                    setHealthError(null);
                    try {
                      const res = await testMailGatewayHealth(baseUrl, apiKey);
                      setHealthResult(res);
                    } catch (e: any) {
                      setHealthError(String(e));
                    } finally {
                      setHealthTesting(false);
                    }
                  }}
                  style={{ minWidth: 86, justifyContent: "center" }}
                >
                  {healthTesting ? (
                    <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                      <Loader2 size={12} className="animate-spin" />
                      测试中...
                    </span>
                  ) : (
                    "测试"
                  )}
                </button>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8 }}>
                {healthResult && (
                  <span style={{ fontSize: 12, color: "var(--ok)" }}>
                    健康检查通过：status={healthResult.status}，timestamp={healthResult.timestamp}
                  </span>
                )}
                {healthError && (
                  <span style={{ fontSize: 12, color: "var(--error)" }}>
                    {healthError}
                  </span>
                )}
              </div>
            </div>
            <div className="form-group">
              <label>API Key</label>
              <input
                type="password"
                value={configs["mail_gateway_api_key"] || ""}
                onChange={(e) => updateConfig("mail_gateway_api_key", e.target.value)}
                placeholder="可选，网关开启鉴权时填写"
                className="input"
                style={{ width: "100%", maxWidth: 640 }}
              />
            </div>
            <div className="form-group">
              <label>Provider</label>
              <input
                type="text"
                value={configs["mail_provider"] || DEFAULT_MAIL_PROVIDER}
                onChange={(e) => updateConfig("mail_provider", e.target.value)}
                placeholder={DEFAULT_MAIL_PROVIDER}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group">
              <label>Provider Mode</label>
              <input
                type="text"
                value={configs["mail_provider_mode"] || DEFAULT_MAIL_PROVIDER_MODE}
                onChange={(e) => updateConfig("mail_provider_mode", e.target.value)}
                placeholder={DEFAULT_MAIL_PROVIDER_MODE}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group">
              <label>Project Code</label>
              <input
                type="text"
                value={configs["mail_project_code"] || ""}
                onChange={(e) => updateConfig("mail_project_code", e.target.value)}
                placeholder={DEFAULT_MAIL_PROJECT_CODE}
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group">
              <label>Domain</label>
              <input
                type="text"
                value={configs["mail_domain"] || ""}
                onChange={(e) => updateConfig("mail_domain", e.target.value)}
                placeholder="留空表示由网关/provider 自行选择"
                className="input"
                style={{ width: "100%", maxWidth: 320 }}
              />
            </div>
            <div className="form-group">
              <label>Provider 状态</label>
              <div className="settings-hint" style={{ marginBottom: 8 }}>
                健康检查会返回各 provider 的可用状态。
              </div>
              <div style={{ display: "grid", gap: 8, maxWidth: 420 }}>
                {healthResult ? (
                  Object.entries(healthResult.providers).map(([provider, status]) => (
                    <div
                      key={provider}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        padding: "10px 12px",
                        border: "1px solid var(--border)",
                        borderRadius: 10,
                        background: "var(--panel)",
                        fontSize: 13,
                      }}
                    >
                      <span style={{ color: "var(--text)" }}>{provider}</span>
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
      </div>
    </>
  );
}
