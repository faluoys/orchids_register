import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Loader2 } from "lucide-react";
import {
  getAllConfig,
  getServiceStatus,
  onServiceStatusChanged,
  saveConfig,
  startTurnstileSolver,
  stopTurnstileSolver,
  testProxy,
} from "@/lib/tauri-api";
import type { ServiceSource, ServiceStatus } from "@/lib/types";

const DEFAULTS = {
  conda_env: "orchids-register",
  turnstile_host: "127.0.0.1",
  turnstile_port: "5000",
  turnstile_thread: "2",
  turnstile_browser_type: "chromium",
  turnstile_headless: "true",
  turnstile_debug: "false",
  turnstile_proxy: "false",
  turnstile_random: "false",
  captcha_timeout: "180",
  captcha_poll_interval: "3",
  proxy_pool_api:
    "https://api.douyadaili.com/proxy/?service=GetUnl&authkey=1KB6xBwGlITDeICSw6BI&num=10&lifetime=1&prot=0&format=txt&cstmfmt=%7Bip%7D%7C%7Bport%7D&separator=%5Cr%5Cn&distinct=1&detail=0&portlen=0",
  use_proxy_pool: "false",
  proxy: "",
  refresh_interval: "",
} as const;

const SAVE_KEYS = [
  "conda_env",
  "turnstile_host",
  "turnstile_port",
  "turnstile_thread",
  "turnstile_browser_type",
  "turnstile_headless",
  "turnstile_debug",
  "turnstile_proxy",
  "turnstile_random",
  "captcha_api_url",
  "captcha_timeout",
  "captcha_poll_interval",
  "proxy_pool_api",
  "use_proxy_pool",
  "proxy",
  "refresh_interval",
] as const;

function deriveCaptchaApiUrl(configs: Record<string, string>): string {
  const host = (configs["turnstile_host"] || DEFAULTS.turnstile_host).trim();
  const port = (configs["turnstile_port"] || DEFAULTS.turnstile_port).trim();
  return `http://${host}:${port}`;
}

function normalizeConfigs(config: Record<string, string>): Record<string, string> {
  const next = {
    ...config,
    conda_env: config["conda_env"] || DEFAULTS.conda_env,
    turnstile_host: config["turnstile_host"] || DEFAULTS.turnstile_host,
    turnstile_port: config["turnstile_port"] || DEFAULTS.turnstile_port,
    turnstile_thread: config["turnstile_thread"] || DEFAULTS.turnstile_thread,
    turnstile_browser_type:
      config["turnstile_browser_type"] || DEFAULTS.turnstile_browser_type,
    turnstile_headless: config["turnstile_headless"] || DEFAULTS.turnstile_headless,
    turnstile_debug: config["turnstile_debug"] || DEFAULTS.turnstile_debug,
    turnstile_proxy: config["turnstile_proxy"] || DEFAULTS.turnstile_proxy,
    turnstile_random: config["turnstile_random"] || DEFAULTS.turnstile_random,
    captcha_timeout: config["captcha_timeout"] || DEFAULTS.captcha_timeout,
    captcha_poll_interval:
      config["captcha_poll_interval"] || DEFAULTS.captcha_poll_interval,
    proxy_pool_api: config["proxy_pool_api"] || DEFAULTS.proxy_pool_api,
    use_proxy_pool: config["use_proxy_pool"] || DEFAULTS.use_proxy_pool,
    proxy: config["proxy"] || DEFAULTS.proxy,
    refresh_interval: config["refresh_interval"] || DEFAULTS.refresh_interval,
  };

  return {
    ...next,
    captcha_api_url: deriveCaptchaApiUrl(next),
  };
}

function buildSavePayload(configs: Record<string, string>): Record<string, string> {
  const payload: Record<string, string> = {
    captcha_api_url: deriveCaptchaApiUrl(configs),
  };

  for (const key of SAVE_KEYS) {
    payload[key] = (configs[key] || "").trim();
  }

  return payload;
}

function checkboxValue(current: string | undefined): boolean {
  return current === "true";
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

function buildTurnstileWarnings(configs: Record<string, string>): string[] {
  const warnings: string[] = [];
  if (!(configs["conda_env"] || "").trim()) {
    warnings.push("还没填写 Conda Environment。");
  }
  if (!(configs["turnstile_host"] || "").trim()) {
    warnings.push("还没填写 TurnstileSolver Host。");
  }
  if (!(configs["turnstile_port"] || "").trim()) {
    warnings.push("还没填写 TurnstileSolver Port。");
  }
  if (!(configs["turnstile_thread"] || "").trim()) {
    warnings.push("还没填写线程数。");
  }
  return warnings;
}

export default function SettingsPage() {
  const [configs, setConfigs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");
  const [serviceStatus, setServiceStatus] = useState<ServiceStatus | null>(null);
  const [serviceBusy, setServiceBusy] = useState<"start" | "stop" | "restart" | null>(null);
  const [serviceError, setServiceError] = useState<string | null>(null);
  const [proxyTesting, setProxyTesting] = useState(false);
  const [proxyResult, setProxyResult] = useState<{ ip: string; country: string; city: string } | null>(null);
  const [proxyError, setProxyError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const guidanceWarnings = buildTurnstileWarnings(configs);

  const refreshServiceStatus = useCallback(async () => {
    try {
      const statuses = await getServiceStatus();
      setServiceStatus(statuses.turnstile_solver);
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
      console.error("加载系统配置失败:", error);
      setServiceError(String(error));
    } finally {
      setLoading(false);
    }
  }, [persistConfig, refreshServiceStatus]);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void onServiceStatusChanged((event) => {
      if (disposed || event.service !== "turnstile_solver") {
        return;
      }
      setServiceStatus(event.status);
      setServiceError(null);
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

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
        console.error("自动保存系统配置失败:", error);
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

  const updateToggle = (key: string, checked: boolean) => {
    updateConfig(key, checked ? "true" : "false");
  };

  const runServiceAction = async (action: "start" | "stop" | "restart") => {
    setServiceBusy(action);
    setServiceError(null);
    try {
      await persistConfig(configs);
      if (action === "start") {
        setServiceStatus(await startTurnstileSolver());
      } else if (action === "stop") {
        setServiceStatus(await stopTurnstileSolver());
      } else {
        await stopTurnstileSolver();
        setServiceStatus(await startTurnstileSolver());
      }
      await refreshServiceStatus();
    } catch (error) {
      setServiceError(String(error));
      await refreshServiceStatus();
    } finally {
      setServiceBusy(null);
    }
  };

  const runProxyTest = async () => {
    setProxyTesting(true);
    setProxyResult(null);
    setProxyError(null);
    try {
      const result = await testProxy();
      setProxyResult(result);
    } catch (error) {
      setProxyError(String(error));
    } finally {
      setProxyTesting(false);
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
          <h1 className="page-title">系统设置</h1>
          <span className="page-subtitle">运行环境、TurnstileSolver 与代理参数</span>
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
                    TurnstileSolver 服务
                  </div>
                  <div className="settings-hint" style={{ marginTop: 0 }}>
                    由桌面端直接托管，本地验证码 API 地址自动派生。
                  </div>
                </div>
                <span className={`service-pill ${serviceStatus?.running ? "running" : "stopped"}`}>
                  {serviceStatus?.running ? "运行中" : "未启动"}
                </span>
              </div>

              <div className="service-meta">
                <span>API URL: {deriveCaptchaApiUrl(configs)}</span>
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
              </div>

              {serviceError ? <div className="service-error">{serviceError}</div> : null}
              {serviceStatus?.last_error ? (
                <div className="service-error">{serviceStatus.last_error}</div>
              ) : null}
            </div>
          </div>

          <div className="config-panel">
            <div className="guidance-card">
              <div className="guidance-title">桌面端配置提示</div>
              <div className="guidance-copy">
                这里负责桌面端自己的 TurnstileSolver 和代理相关配置。桌面流程会直接读取这些值，
                不需要你再去同步修改 <code>runtime.local.yaml</code>。
              </div>
              <ul className="guidance-list">
                <li>首次配置先确认 Conda 环境、Host、Port、Thread，再启动 TurnstileSolver。</li>
                <li>服务起来后，注册流程会自动使用这里派生出的本地验证码 API 地址。</li>
                <li>启动失败时，优先检查 Conda 环境名、端口占用、依赖是否已安装。</li>
              </ul>
              {guidanceWarnings.length > 0 ? (
                <div className="guidance-warning">{guidanceWarnings.join(" ")}</div>
              ) : null}
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">运行时环境</div>
            <div className="form-group">
              <label>Conda Environment</label>
              <input
                type="text"
                value={configs["conda_env"] || ""}
                onChange={(event) => updateConfig("conda_env", event.target.value)}
                className="input"
                style={{ width: "100%", maxWidth: 360 }}
              />
              <div className="settings-hint">启动 mail-gateway 和 TurnstileSolver 时统一使用这个环境。</div>
            </div>
            <div className="form-group" style={{ marginBottom: 0 }}>
              <label>本地验证码 API URL</label>
              <input
                type="text"
                value={deriveCaptchaApiUrl(configs)}
                readOnly
                className="input"
                style={{ width: "100%", background: "rgba(255,255,255,0.72)" }}
              />
              <div className="settings-hint">由 TurnstileSolver 的 Host + Port 自动生成，并同步给注册流程。</div>
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">TurnstileSolver 参数</div>
            <div className="form-row">
              <div className="form-group" style={{ flex: 1 }}>
                <label>Host</label>
                <input
                  type="text"
                  value={configs["turnstile_host"] || ""}
                  onChange={(event) => updateConfig("turnstile_host", event.target.value)}
                  className="input"
                />
              </div>
              <div className="form-group" style={{ width: 160 }}>
                <label>Port</label>
                <input
                  type="text"
                  value={configs["turnstile_port"] || ""}
                  onChange={(event) => updateConfig("turnstile_port", event.target.value)}
                  className="input"
                />
              </div>
            </div>
            <div className="form-row">
              <div className="form-group" style={{ width: 180 }}>
                <label>Thread</label>
                <input
                  type="text"
                  value={configs["turnstile_thread"] || ""}
                  onChange={(event) => updateConfig("turnstile_thread", event.target.value)}
                  className="input"
                />
              </div>
              <div className="form-group" style={{ flex: 1 }}>
                <label>Browser Type</label>
                <select
                  value={configs["turnstile_browser_type"] || DEFAULTS.turnstile_browser_type}
                  onChange={(event) => updateConfig("turnstile_browser_type", event.target.value)}
                  className="input"
                >
                  <option value="chromium">chromium</option>
                  <option value="firefox">firefox</option>
                  <option value="webkit">webkit</option>
                </select>
              </div>
            </div>

            <div className="settings-switch-row">
              <span>Headless</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={checkboxValue(configs["turnstile_headless"])}
                  onChange={(event) => updateToggle("turnstile_headless", event.target.checked)}
                />
                <span className="slider" />
              </label>
            </div>
            <div className="settings-switch-row">
              <span>Debug</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={checkboxValue(configs["turnstile_debug"])}
                  onChange={(event) => updateToggle("turnstile_debug", event.target.checked)}
                />
                <span className="slider" />
              </label>
            </div>
            <div className="settings-switch-row">
              <span>启用 Solver Proxy</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={checkboxValue(configs["turnstile_proxy"])}
                  onChange={(event) => updateToggle("turnstile_proxy", event.target.checked)}
                />
                <span className="slider" />
              </label>
            </div>
            <div className="settings-switch-row" style={{ marginBottom: 0 }}>
              <span>Random 模式</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={checkboxValue(configs["turnstile_random"])}
                  onChange={(event) => updateToggle("turnstile_random", event.target.checked)}
                />
                <span className="slider" />
              </label>
            </div>
          </div>

          <div className="config-panel">
            <div className="settings-title">注册参数与代理</div>
            <div className="form-group">
              <label>验证码超时 (秒)</label>
              <input
                type="number"
                value={configs["captcha_timeout"] || DEFAULTS.captcha_timeout}
                onChange={(event) => updateConfig("captcha_timeout", event.target.value)}
                className="input input-sm"
                min={30}
                max={600}
              />
            </div>
            <div className="form-group">
              <label>验证码轮询间隔 (秒)</label>
              <input
                type="number"
                value={configs["captcha_poll_interval"] || DEFAULTS.captcha_poll_interval}
                onChange={(event) => updateConfig("captcha_poll_interval", event.target.value)}
                className="input input-sm"
                min={1}
                max={10}
                step={0.5}
              />
            </div>

            <div className="form-group">
              <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <input
                  type="checkbox"
                  checked={checkboxValue(configs["use_proxy_pool"])}
                  onChange={(event) => updateToggle("use_proxy_pool", event.target.checked)}
                />
                使用代理池
              </label>
            </div>

            {checkboxValue(configs["use_proxy_pool"]) ? (
              <div className="form-group">
                <label>代理池 API</label>
                <input
                  type="text"
                  value={configs["proxy_pool_api"] || ""}
                  onChange={(event) => updateConfig("proxy_pool_api", event.target.value)}
                  className="input"
                  style={{ width: "100%" }}
                />
              </div>
            ) : (
              <div className="form-group">
                <label>单一代理地址</label>
                <input
                  type="text"
                  value={configs["proxy"] || ""}
                  onChange={(event) => updateConfig("proxy", event.target.value)}
                  className="input"
                  style={{ width: "100%" }}
                  placeholder="http://user:pass@host:port 或 socks5://host:port"
                />
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8, flexWrap: "wrap" }}>
                  <button
                    type="button"
                    className="btn btn-sm btn-clear"
                    disabled={proxyTesting}
                    onClick={() => void runProxyTest()}
                  >
                    {proxyTesting ? <Loader2 size={12} className="animate-spin" /> : null}
                    测试代理
                  </button>
                  {proxyResult ? (
                    <span style={{ fontSize: 12, color: "var(--ok)" }}>
                      IP: {proxyResult.ip} | {proxyResult.city}, {proxyResult.country}
                    </span>
                  ) : null}
                  {proxyError ? (
                    <span style={{ fontSize: 12, color: "var(--danger)" }}>
                      {proxyError}
                    </span>
                  ) : null}
                </div>
              </div>
            )}

            <div className="form-group" style={{ marginBottom: 0 }}>
              <label>账户页自动刷新间隔 (秒)</label>
              <input
                type="number"
                value={configs["refresh_interval"] || ""}
                onChange={(event) => updateConfig("refresh_interval", event.target.value)}
                placeholder="30"
                className="input input-sm"
                min={5}
                max={600}
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
