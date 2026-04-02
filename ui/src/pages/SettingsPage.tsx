import { useState, useEffect, useCallback, useRef } from "react";
import { Loader2, Check } from "lucide-react";
import { getAllConfig, saveConfig, testProxy } from "@/lib/tauri-api";

const DEFAULT_CAPTCHA_API_URL = "http://127.0.0.1:5000";
const DEFAULT_PROXY_POOL_API = "https://api.douyadaili.com/proxy/?service=GetUnl&authkey=1KB6xBwGlITDeICSw6BI&num=10&lifetime=1&prot=0&format=txt&cstmfmt=%7Bip%7D%7C%7Bport%7D&separator=%5Cr%5Cn&distinct=1&detail=0&portlen=0";

export default function SettingsPage() {
  const [configs, setConfigs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [proxyTesting, setProxyTesting] = useState(false);
  const [proxyResult, setProxyResult] = useState<{ ip: string; country: string; city: string } | null>(null);
  const [proxyError, setProxyError] = useState<string | null>(null);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const config = await getAllConfig();

      const next = {
        ...config,
        captcha_api_url: config["captcha_api_url"] || DEFAULT_CAPTCHA_API_URL,
        proxy_pool_api: config["proxy_pool_api"] || DEFAULT_PROXY_POOL_API,
        use_proxy_pool: config["use_proxy_pool"] || "false",
      };

      setConfigs(next);

      const defaultsToSave: Record<string, string> = {
        captcha_api_url: next.captcha_api_url,
        proxy_pool_api: next.proxy_pool_api,
        use_proxy_pool: next.use_proxy_pool,
      };
      await saveConfig(defaultsToSave);
    } catch (e) {
      console.error("加载配置失败:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  // Auto-save: debounced saveConfig on every input change
  const autoSave = useCallback((newConfigs: Record<string, string>) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      setSaveStatus("saving");
      try {
        const toSave: Record<string, string> = {};
        for (const [k, v] of Object.entries(newConfigs)) {
          if (typeof v === 'string' && v.trim()) toSave[k] = v.trim();
          else if (typeof v === 'string') toSave[k] = v;
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

  // Cleanup debounce on unmount
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
      {/* Page header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap", flexShrink: 0 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
          <h1 className="page-title">系统设置</h1>
          <span className="page-subtitle">配置参数</span>
        </div>
        {/* Auto-save indicator */}
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

      {/* Settings */}
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        <div className="settings-grid">
          {/* 打码 API 配置 */}
          <div className="config-panel">
            <div className="settings-title">验证码求解 (本地打码 API)</div>
            <div className="form-group">
              <label>打码 API 地址</label>
              <input
                type="text"
                value={configs["captcha_api_url"] || DEFAULT_CAPTCHA_API_URL}
                onChange={(e) => updateConfig("captcha_api_url", e.target.value)}
                placeholder="http://127.0.0.1:5000"
                className="input"
                style={{ width: "100%", maxWidth: 640 }}
              />
              <div className="settings-hint">
                本地打码 API 地址（需要先启动打码服务）
              </div>
            </div>
            <div className="form-group">
              <label>打码超时时间 (秒)</label>
              <input
                type="number"
                value={configs["captcha_timeout"] || "180"}
                onChange={(e) => updateConfig("captcha_timeout", e.target.value)}
                placeholder="180"
                className="input input-sm"
                min={30}
                max={600}
                style={{ width: 120 }}
              />
              <div className="settings-hint">
                等待打码完成的最长时间（秒）
              </div>
            </div>
            <div className="form-group">
              <label>打码轮询间隔 (秒)</label>
              <input
                type="number"
                value={configs["captcha_poll_interval"] || "3"}
                onChange={(e) => updateConfig("captcha_poll_interval", e.target.value)}
                placeholder="3"
                className="input input-sm"
                min={1}
                max={10}
                step={0.5}
                style={{ width: 120 }}
              />
              <div className="settings-hint">
                轮询打码结果的时间间隔（秒）
              </div>
            </div>
          </div>

          {/* 代理配置 */}
          <div className="config-panel">
            <div className="settings-title">代理设置</div>
            <div className="form-group">
              <label>
                <input
                  type="checkbox"
                  checked={configs["use_proxy_pool"] === "true"}
                  onChange={(e) => updateConfig("use_proxy_pool", e.target.checked ? "true" : "false")}
                  style={{ marginRight: 8 }}
                />
                启用代理池
              </label>
              <div className="settings-hint">
                启用后，每个注册线程将使用独立的代理（从代理池 API 获取）
              </div>
            </div>
            {configs["use_proxy_pool"] === "true" && (
              <div className="form-group">
                <label>代理池 API 地址</label>
                <input
                  type="text"
                  value={configs["proxy_pool_api"] || DEFAULT_PROXY_POOL_API}
                  onChange={(e) => updateConfig("proxy_pool_api", e.target.value)}
                  placeholder="代理池 API 地址"
                  className="input"
                  style={{ width: "100%", maxWidth: 640 }}
                />
                <div className="settings-hint">
                  代理池 API 地址（默认使用豆芽代理）
                </div>
              </div>
            )}
            {configs["use_proxy_pool"] !== "true" && (
              <div className="form-group">
                <label>单一代理地址</label>
                <input
                  type="text"
                  value={configs["proxy"] || ""}
                  onChange={(e) => updateConfig("proxy", e.target.value)}
                  placeholder="http://user:pass@host:port 或 socks5://host:port"
                  className="input"
                  style={{ width: "100%", maxWidth: 640 }}
                />
                <div className="settings-hint">
                  支持 HTTP / HTTPS / SOCKS5 代理，留空则不使用代理
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8 }}>
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={proxyTesting}
                    onClick={async () => {
                      setProxyTesting(true);
                      setProxyResult(null);
                      setProxyError(null);
                      try {
                        const res = await testProxy();
                        setProxyResult(res);
                      } catch (e: any) {
                        setProxyError(String(e));
                      } finally {
                        setProxyTesting(false);
                      }
                    }}
                    style={{ minWidth: 90, justifyContent: "center" }}
                  >
                    {proxyTesting ? (
                      <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                        <Loader2 size={12} className="animate-spin" />
                        测试中...
                      </span>
                    ) : (
                      "测试代理"
                    )}
                  </button>
                  {proxyResult && (
                    <span style={{ fontSize: 12, color: "var(--ok)" }}>
                      IP: {proxyResult.ip} | {proxyResult.city}, {proxyResult.country}
                    </span>
                  )}
                  {proxyError && (
                    <span style={{ fontSize: 12, color: "var(--error)" }}>
                      {proxyError}
                    </span>
                  )}
                </div>
              </div>
            )}
          </div>

          {/* 其他配置 */}
          <div className="config-panel">
            <div className="settings-title">其他设置</div>
            <div className="form-group">
              <label>刷新间隔 (秒)</label>
              <input
                type="number"
                value={configs["refresh_interval"] || ""}
                onChange={(e) => updateConfig("refresh_interval", e.target.value)}
                placeholder="30"
                className="input input-sm"
                min={5}
                max={600}
                style={{ width: 120 }}
              />
              <div className="settings-hint">
                自动刷新账号列表的时间间隔（秒）
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
