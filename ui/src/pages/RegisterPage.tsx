import { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, ChevronRight } from "lucide-react";
import {
  startRegistration,
  startBatchRegistration,
  cancelBatch,
  getAllConfig,
  getServiceStatus,
} from "@/lib/tauri-api";
import type { LogEntry, RegisterArgs, BatchProgress, BatchComplete } from "@/lib/types";

const defaultArgs: RegisterArgs = {
  email: null,
  password: null,
  captcha_token: null,
  use_capmonster: true,
  captcha_api_url: "http://127.0.0.1:5000",
  captcha_timeout: 180,
  captcha_poll_interval: 3.0,
  captcha_website_url: "https://accounts.orchids.app/",
  captcha_website_key: "0x4AAAAAAAWXJGBD7bONzLBd",
  email_code: null,
  locale: "zh-CN",
  timeout: 30,
  mail_mode: "gateway",
  mail_gateway_base_url: null,
  mail_gateway_api_key: null,
  mail_provider: "luckmail",
  mail_provider_mode: "purchased",
  mail_project_code: "orchids",
  mail_domain: null,
  poll_timeout: 180,
  poll_interval: 2.0,
  code_pattern: "\\b(\\d{6})\\b",
  debug_email: true,
  test_desktop_session: true,
  proxy: null,
  use_proxy_pool: false,
  proxy_pool_api:
    "https://api.douyadaili.com/proxy/?service=GetUnl&authkey=1KB6xBwGlITDeICSw6BI&num=10&lifetime=1&prot=0&format=txt&cstmfmt=%7Bip%7D%7C%7Bport%7D&separator=%5Cr%5Cn&distinct=1&detail=0&portlen=0",
};

const LOG_TRUNCATE_LEN = 200;

function deriveUrl(
  host: string | undefined,
  port: string | undefined,
  fallback: string
): string {
  if (host?.trim() && port?.trim()) {
    return `http://${host.trim()}:${port.trim()}`;
  }
  return fallback;
}

function LogLine({ log }: { log: LogEntry }) {
  const [expanded, setExpanded] = useState(false);
  const isLong = log.message.length > LOG_TRUNCATE_LEN;
  const levelClass =
    log.level === "error" ? "error" : log.level === "warn" ? "warn" : log.level === "info" ? "info" : "success";

  return (
    <div className={`log-line ${levelClass}`}>
      <span className="log-step">[{log.step}]</span>
      {isLong && !expanded ? (
        <>
          {log.message.slice(0, LOG_TRUNCATE_LEN)}
          <button className="log-expand-btn" onClick={() => setExpanded(true)}>
            <ChevronRight size={11} />
            展开
          </button>
        </>
      ) : isLong && expanded ? (
        <>
          {log.message}
          <button className="log-expand-btn" onClick={() => setExpanded(false)}>
            <ChevronDown size={11} />
            收起
          </button>
        </>
      ) : (
        log.message
      )}
    </div>
  );
}

function formatTime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

export default function RegisterPage() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [regCount, setRegCount] = useState(1);
  const [regThreads, setRegThreads] = useState(1);
  const [batchProgress, setBatchProgress] = useState<BatchProgress | null>(null);
  const [startedCount, setStartedCount] = useState(0);

  const [totalElapsed, setTotalElapsed] = useState(0);
  const [currentElapsed, setCurrentElapsed] = useState(0);
  const [successCount, setSuccessCount] = useState(0);
  const [failCount, setFailCount] = useState(0);
  const [currentIdx, setCurrentIdx] = useState(0);
  const [preflightError, setPreflightError] = useState<string | null>(null);

  const logEndRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startTimeRef = useRef<number>(0);
  const currentStartRef = useRef<number>(0);
  const regThreadsRef = useRef<number>(regThreads);
  const prevStartedRef = useRef<number>(0);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  useEffect(() => {
    regThreadsRef.current = regThreads;
  }, [regThreads]);

  useEffect(() => {
    if (isRunning) {
      startTimeRef.current = Date.now();
      currentStartRef.current = Date.now();
      timerRef.current = setInterval(() => {
        setTotalElapsed(Math.floor((Date.now() - startTimeRef.current) / 1000));
        setCurrentElapsed(Math.floor((Date.now() - currentStartRef.current) / 1000));
      }, 1000);
    } else if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, [isRunning]);

  useEffect(() => {
    const unlistenLog = listen<LogEntry>("register-log", (event) => {
      setLogs((prev) => [...prev, event.payload]);
    });
    const unlistenProgress = listen<BatchProgress>("batch-progress", (event) => {
      const totalDone = event.payload.completed + event.payload.failed;
      const nextStarted = Math.min(totalDone + regThreadsRef.current, event.payload.total);

      setBatchProgress(event.payload);
      setSuccessCount(event.payload.completed);
      setFailCount(event.payload.failed);
      setStartedCount(nextStarted);
      setCurrentIdx(nextStarted);
    });
    const unlistenComplete = listen<BatchComplete>("batch-complete", (event) => {
      setIsRunning(false);
      setIsStopping(false);
      setSuccessCount(event.payload.completed);
      setFailCount(event.payload.failed);
      setCurrentIdx(event.payload.total);
      setStartedCount(event.payload.total);
      prevStartedRef.current = event.payload.total;
      setBatchProgress({
        completed: event.payload.completed,
        failed: event.payload.failed,
        total: event.payload.total,
        current_email: null,
      });
      setLogs((prev) => [
        ...prev,
        {
          step: "完成",
          message: `批量注册完成: 成功 ${event.payload.completed}/${event.payload.total}，失败 ${event.payload.failed}`,
          level: "info",
          timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
        },
      ]);
      window.dispatchEvent(new CustomEvent("accounts-changed"));
    });

    return () => {
      unlistenLog.then((f) => f());
      unlistenProgress.then((f) => f());
      unlistenComplete.then((f) => f());
    };
  }, []);

  const buildArgs = useCallback(async (): Promise<{
    args: RegisterArgs;
    config: Record<string, string>;
  }> => {
    const config = await getAllConfig();
    const mailGatewayBaseUrl = (
      config["mail_gateway_base_url"] ||
      deriveUrl(
        config["mail_gateway_host"],
        config["mail_gateway_port"],
        defaultArgs.mail_gateway_base_url || "http://127.0.0.1:8081"
      )
    ).trim();
    const captchaApiUrl = (
      config["captcha_api_url"] ||
      deriveUrl(
        config["turnstile_host"],
        config["turnstile_port"],
        defaultArgs.captcha_api_url
      )
    ).trim();

    return {
      config,
      args: {
        ...defaultArgs,
        password: config["password"] || null,
        captcha_api_url: captchaApiUrl || defaultArgs.captcha_api_url,
        captcha_timeout: Number(config["captcha_timeout"]) || defaultArgs.captcha_timeout,
        captcha_poll_interval: Number(config["captcha_poll_interval"]) || defaultArgs.captcha_poll_interval,
        captcha_website_key: config["captcha_website_key"] || defaultArgs.captcha_website_key,
        captcha_website_url: config["captcha_website_url"] || defaultArgs.captcha_website_url,
        locale: config["locale"] || defaultArgs.locale,
        timeout: Number(config["timeout"]) || defaultArgs.timeout,
        poll_timeout: Number(config["poll_timeout"]) || defaultArgs.poll_timeout,
        poll_interval: Number(config["poll_interval"]) || defaultArgs.poll_interval,
        mail_mode: config["mail_mode"] || defaultArgs.mail_mode,
        mail_gateway_base_url: mailGatewayBaseUrl || null,
        mail_gateway_api_key: config["mail_gateway_api_key"] || null,
        mail_provider: config["mail_provider"] || defaultArgs.mail_provider,
        mail_provider_mode: config["mail_provider_mode"] || defaultArgs.mail_provider_mode,
        mail_project_code: config["mail_project_code"] || defaultArgs.mail_project_code,
        mail_domain: config["mail_domain"] || null,
        proxy: config["proxy"] || null,
        use_proxy_pool: config["use_proxy_pool"] === "true",
        proxy_pool_api: config["proxy_pool_api"] || defaultArgs.proxy_pool_api,
      },
    };
  }, []);

  const runDesktopPreflight = useCallback(async (
    args: RegisterArgs,
    config: Record<string, string>
  ) => {
    const statuses = await getServiceStatus();

    if (args.mail_mode === "gateway") {
      if (!args.mail_gateway_base_url?.trim()) {
        throw new Error("Mail Gateway 地址未配置");
      }
      if (!statuses.mail_gateway?.running) {
        throw new Error("请先启动 Mail Gateway 服务");
      }
      if (args.mail_provider === "luckmail" && !(config["luckmail_api_key"] || "").trim()) {
        throw new Error("LuckMail API Key 未配置");
      }
      if (args.mail_provider === "yyds_mail" && !(config["yyds_api_key"] || "").trim()) {
        throw new Error("YYDS API Key 未配置");
      }
    }

    if (args.use_capmonster && !statuses.turnstile_solver?.running) {
      throw new Error("请先启动 TurnstileSolver 服务");
    }
  }, []);

  const handleStart = async () => {
    setIsRunning(true);
    setIsStopping(false);
    setPreflightError(null);
    setLogs([]);
    setSuccessCount(0);
    setFailCount(0);
    setCurrentIdx(0);
    setStartedCount(0);
    prevStartedRef.current = 0;
    setTotalElapsed(0);
    setCurrentElapsed(0);
    setBatchProgress(null);

    try {
      const { args, config } = await buildArgs();
      await runDesktopPreflight(args, config);
      if (regCount === 1) {
        await startRegistration(args);
        setStartedCount(1);
        prevStartedRef.current = 1;
        setSuccessCount(1);
        setLogs((prev) => [
          ...prev,
          {
            step: "完成",
            message: "注册成功！",
            level: "info",
            timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
          },
        ]);
        setIsRunning(false);
        window.dispatchEvent(new CustomEvent("accounts-changed"));
      } else {
        const initialStarted = Math.min(regThreadsRef.current, regCount);
        setStartedCount(initialStarted);
        prevStartedRef.current = initialStarted;
        currentStartRef.current = Date.now();
        await startBatchRegistration(args, regCount, regThreads);
      }
    } catch (e: any) {
      setPreflightError(String(e));
      setLogs((prev) => [
        ...prev,
        {
          step: "错误",
          message: String(e),
          level: "error",
          timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
        },
      ]);
      setIsRunning(false);
    }
  };

  const handleStop = async () => {
    setIsStopping(true);
    try {
      await cancelBatch();
      setLogs((prev) => [
        ...prev,
        {
          step: "取消",
          message: "已发送停止信号，等待当前任务完成...",
          level: "warn",
          timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
        },
      ]);
    } catch (e: any) {
      setIsStopping(false);
      setLogs((prev) => [
        ...prev,
        {
          step: "错误",
          message: `停止失败: ${String(e)}`,
          level: "error",
          timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
        },
      ]);
    }
  };

  useEffect(() => {
    if (!isRunning || startedCount === 0) {
      return;
    }

    if (startedCount > prevStartedRef.current) {
      prevStartedRef.current = startedCount;
      currentStartRef.current = Date.now();
      setCurrentElapsed(0);
    }
  }, [startedCount, isRunning]);

  const totalDone = successCount + failCount;
  const totalTarget = batchProgress?.total ?? regCount;
  const progressPercent = totalTarget > 0 ? (totalDone / totalTarget) * 100 : 0;
  const avgTime = totalDone > 0 ? Math.floor(totalElapsed / totalDone) : 0;
  const runningCount = Math.min(regThreadsRef.current, Math.max(totalTarget - totalDone, 0));
  const runningStart = totalDone + 1;
  const runningIndices = isRunning
    ? Array.from({ length: runningCount }, (_, i) => `#${runningStart + i}`)
    : [];
  const displayCurrentIdx = runningIndices.length > 0 ? runningIndices.join("、") : currentIdx || "-";

  return (
    <>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 16,
          flexWrap: "wrap",
          flexShrink: 0,
        }}
      >
        <h1 className="page-title">自动注册</h1>
      </div>

      <div className="register-layout">
        <div className="register-left">
          {preflightError ? (
            <div
              className="status-bar"
              style={{ color: "var(--danger)", borderColor: "rgba(224, 69, 69, 0.2)" }}
            >
              {preflightError}
            </div>
          ) : null}

          <div className="reg-section">
            <div className="reg-section-title">注册设置</div>
            <div className="reg-field">
              <label>注册数量</label>
              <input
                type="number"
                className="input input-sm"
                value={regCount}
                min={1}
                max={100}
                onChange={(e) => setRegCount(Number(e.target.value))}
                disabled={isRunning}
              />
            </div>
            <div className="reg-field">
              <label>并行线程</label>
              <input
                type="number"
                className="input input-sm"
                value={regThreads}
                min={1}
                max={10}
                onChange={(e) => setRegThreads(Number(e.target.value))}
                disabled={isRunning}
              />
            </div>
            <div className="reg-buttons">
              <button
                className="btn btn-success"
                onClick={handleStart}
                disabled={isRunning}
              >
                开始注册
              </button>
              <button
                className="btn"
                onClick={handleStop}
                disabled={!isRunning || isStopping}
                style={{
                  background: isRunning && !isStopping
                    ? "linear-gradient(135deg, #ef4444, #dc2626)"
                    : "linear-gradient(180deg, #fff3f3, #ffe9e9)",
                  color: isRunning && !isStopping ? "#fff" : "#bd2f2f",
                  borderColor: isRunning && !isStopping ? "#dc2626" : "#f1b8b8",
                  boxShadow: isRunning && !isStopping
                    ? "0 10px 24px rgba(220, 38, 38, 0.28)"
                    : "none",
                }}
              >
                {isStopping ? "停止中..." : "停止注册"}
              </button>
            </div>
          </div>

          <div className="reg-section">
            <div className="reg-section-title">注册进度</div>
            <div className="reg-progress-bar-wrap">
              <div
                className="reg-progress-bar"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
            <div className="reg-progress-text">
              {totalDone} / {batchProgress?.total ?? regCount}
            </div>
            <div className="reg-stats-grid">
              <div className="reg-stat">
                <span className="reg-stat-value reg-stat-success">{successCount}</span>
                <span className="reg-stat-label">成功</span>
              </div>
              <div className="reg-stat">
                <span className="reg-stat-value reg-stat-fail">{failCount}</span>
                <span className="reg-stat-label">失败</span>
              </div>
              <div className="reg-stat">
                <span className="reg-stat-value">{totalDone}</span>
                <span className="reg-stat-label">总计</span>
              </div>
            </div>
          </div>

          <div className="reg-section">
            <div className="reg-section-title">时间统计</div>
            <div className="reg-time-grid">
              <div className="reg-time-item">
                <span className="reg-time-label">总耗时</span>
                <span className="reg-time-value">{formatTime(totalElapsed)}</span>
              </div>
              <div className="reg-time-item">
                <span className="reg-time-label">平均耗时</span>
                <span className="reg-time-value">{formatTime(avgTime)}</span>
              </div>
              <div className="reg-time-item">
                <span className="reg-time-label">当前账号</span>
                <span className="reg-time-value">{displayCurrentIdx}</span>
              </div>
              <div className="reg-time-item">
                <span className="reg-time-label">当前耗时</span>
                <span className="reg-time-value">{formatTime(currentElapsed)}</span>
              </div>
            </div>
          </div>
        </div>

        <div className="register-right">
          <div className="log-container">
            <div className="log-header">
              <span>注册日志</span>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ fontSize: 11, color: "var(--muted)", opacity: 0.6 }}>
                  {logs.length} 条
                </span>
                <button
                  className="btn btn-sm btn-clear"
                  onClick={() => {
                    setLogs([]);
                    setBatchProgress(null);
                  }}
                >
                  清除
                </button>
              </div>
            </div>
            <div className="log-body">
              {logs.length === 0 ? (
                <div className="log-empty">等待开始注册...</div>
              ) : (
                logs.map((log, i) => <LogLine key={i} log={log} />)
              )}
              <div ref={logEndRef} />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
