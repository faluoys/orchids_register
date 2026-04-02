import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  FolderOpen,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { getAccounts, listAccountGroups } from "@/lib/tauri-api";
import type { Account, AccountGroup, BatchProgress, BatchComplete } from "@/lib/types";

function parseTime(value: string): number {
  const ts = Date.parse(value);
  return Number.isNaN(ts) ? 0 : ts;
}

function formatElapsed(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

export default function DashboardPage() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedGroupId, setSelectedGroupId] = useState(0);

  // ── Live batch registration tracking ──
  const [batchProgress, setBatchProgress] = useState<BatchProgress | null>(null);
  const [batchRunning, setBatchRunning] = useState(false);
  const [batchElapsed, setBatchElapsed] = useState(0);
  const batchStartRef = useRef(0);
  const batchTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const batchRunningRef = useRef(false);

  const filteredAccounts = useMemo(() => {
    if (selectedGroupId === 0) return accounts;
    return accounts.filter((a) => a.group_id === selectedGroupId);
  }, [accounts, selectedGroupId]);

  const scopedAccounts = filteredAccounts;

  const fetchDashboard = useCallback(async (initial = false) => {
    if (initial) setLoading(true);
    else setRefreshing(true);
    try {
      const [accs, grps] = await Promise.all([getAccounts(), listAccountGroups()]);
      setAccounts(accs);
      setGroups(grps);
      setError(null);
    } catch (e) {
      console.error("加载仪表盘失败:", e);
      setError("加载数据失败，请稍后重试");
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => { void fetchDashboard(true); }, [fetchDashboard]);
  useEffect(() => {
    const handler = () => { void fetchDashboard(false); };
    window.addEventListener("accounts-changed", handler);
    return () => window.removeEventListener("accounts-changed", handler);
  }, [fetchDashboard]);
  useEffect(() => {
    const timer = setInterval(() => { void fetchDashboard(false); }, 30000);
    return () => clearInterval(timer);
  }, [fetchDashboard]);
  useEffect(() => {
    const unlistenProgress = listen<BatchProgress>("batch-progress", (event) => {
      const p = event.payload;
      setBatchProgress(p);
      if (!batchRunningRef.current) {
        batchRunningRef.current = true;
        batchStartRef.current = Date.now();
        setBatchRunning(true);
      }
      void fetchDashboard(false);
    });
    const unlistenComplete = listen<BatchComplete>("batch-complete", () => {
      batchRunningRef.current = false;
      setBatchRunning(false);
      setBatchProgress(null);
      if (batchTimerRef.current) {
        clearInterval(batchTimerRef.current);
        batchTimerRef.current = null;
      }
      void fetchDashboard(false);
    });
    return () => {
      unlistenProgress.then((f) => f());
      unlistenComplete.then((f) => f());
    };
  }, [fetchDashboard]);

  // ── Batch elapsed timer ──
  useEffect(() => {
    if (batchRunning) {
      if (!batchStartRef.current) batchStartRef.current = Date.now();
      batchTimerRef.current = setInterval(() => {
        setBatchElapsed(Math.floor((Date.now() - batchStartRef.current) / 1000));
      }, 1000);
    } else {
      if (batchTimerRef.current) {
        clearInterval(batchTimerRef.current);
        batchTimerRef.current = null;
      }
      batchStartRef.current = 0;
      setBatchElapsed(0);
    }
    return () => {
      if (batchTimerRef.current) clearInterval(batchTimerRef.current);
    };
  }, [batchRunning]);

  // ── Core metrics ──
  const metrics = useMemo(() => {
    const total = scopedAccounts.length;
    const complete = scopedAccounts.filter((a) => a.status === "complete").length;
    const failed = scopedAccounts.filter((a) => a.status === "failed").length;
    const running = scopedAccounts.filter((a) => a.status === "running").length;
    const pending = scopedAccounts.filter((a) => a.status === "pending").length;
    const successRate = total > 0 ? Number(((complete / total) * 100).toFixed(1)) : 0;
    return { total, complete, failed, running, pending, successRate };
  }, [scopedAccounts]);

  // ── Group performance ──
  const groupStats = useMemo(() => {
    const map = new Map<number, { name: string; total: number; complete: number; failed: number }>();
    for (const acc of scopedAccounts) {
      const existing = map.get(acc.group_id) || { name: acc.group_name, total: 0, complete: 0, failed: 0 };
      existing.total++;
      if (acc.status === "complete") existing.complete++;
      if (acc.status === "failed") existing.failed++;
      map.set(acc.group_id, existing);
    }
    return Array.from(map.values()).sort((a, b) => b.total - a.total);
  }, [scopedAccounts]);

  // ── Credits analysis ──
  const creditsStats = useMemo(() => {
    const creditsAccounts = scopedAccounts.filter(
      (a): a is Account & { credits: number } => typeof a.credits === "number"
    );
    const totalCredits = creditsAccounts.reduce((sum, a) => sum + a.credits, 0);
    const avgCredits = creditsAccounts.length > 0 ? Math.round(totalCredits / creditsAccounts.length) : 0;
    const maxCredits = creditsAccounts.length > 0 ? Math.max(...creditsAccounts.map((a) => a.credits)) : 0;

    const buckets = [
      { label: "0", min: 0, max: 0 },
      { label: "1-1K", min: 1, max: 1000 },
      { label: "1K-10K", min: 1001, max: 10000 },
      { label: "10K-50K", min: 10001, max: 50000 },
      { label: "50K-100K", min: 50001, max: 100000 },
      { label: "100K-150K", min: 100001, max: 150000 },
    ].map((bucket) => ({
      ...bucket,
      count: creditsAccounts.filter((a) => a.credits >= bucket.min && a.credits <= bucket.max).length,
    }));

    return {
      totalCredits,
      avgCredits,
      maxCredits,
      creditsAccountCount: creditsAccounts.length,
      noCreditsCount: scopedAccounts.length - creditsAccounts.length,
      buckets,
    };
  }, [scopedAccounts]);

  const creditsBucketMax = Math.max(1, ...creditsStats.buckets.map((b) => b.count));

  // ── Recent accounts ──
  const recentAccounts = useMemo(() => {
    return [...scopedAccounts]
      .sort((a, b) => parseTime(b.created_at) - parseTime(a.created_at))
      .slice(0, 8);
  }, [scopedAccounts]);

  if (loading) {
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%" }}>
        <Loader2 className="animate-spin" size={24} style={{ color: "var(--accent)" }} />
      </div>
    );
  }

  return (
    <>
      {/* Header - same as reference: page-header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap", flexShrink: 0 }}>
        <h1 className="page-title">仪表盘</h1>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginLeft: "auto" }}>
          <select
            className="input input-sm"
            style={{ width: "auto", minWidth: 140, textAlign: "left" }}
            value={selectedGroupId}
            onChange={(e) => setSelectedGroupId(Number(e.target.value))}
          >
            <option value={0}>全部分组</option>
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {group.name}
              </option>
            ))}
          </select>
          <button className="btn btn-icon btn-primary" onClick={() => fetchDashboard(false)} title="刷新">
            <RefreshCw size={16} className={refreshing ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      {error && <div className="status-bar" style={{ color: "var(--danger)" }}>{error}</div>}

      {/* ── Live registration progress ── */}
      {batchRunning && batchProgress && (() => {
        const done = batchProgress.completed + batchProgress.failed;
        const pct = batchProgress.total > 0 ? (done / batchProgress.total) * 100 : 0;
        const avg = done > 0 ? Math.floor(batchElapsed / done) : 0;
        return (
          <div className="dash-section" style={{ borderColor: "rgba(29, 125, 242, 0.35)", background: "linear-gradient(180deg, #eef6ff, #e3f0ff)" }}>
            <div className="dash-section-title" style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <Loader2 className="animate-spin" size={14} style={{ color: "var(--accent)" }} />
              正在注册中...
            </div>
            <div className="reg-progress-bar-wrap">
              <div className="reg-progress-bar" style={{ width: `${pct}%` }} />
            </div>
            <div className="reg-progress-text">
              {done} / {batchProgress.total}
            </div>
            <div className="reg-stats-grid">
              <div className="reg-stat">
                <span className="reg-stat-value reg-stat-success">{batchProgress.completed}</span>
                <span className="reg-stat-label">成功</span>
              </div>
              <div className="reg-stat">
                <span className="reg-stat-value reg-stat-fail">{batchProgress.failed}</span>
                <span className="reg-stat-label">失败</span>
              </div>
              <div className="reg-stat">
                <span className="reg-stat-value">{done}</span>
                <span className="reg-stat-label">已完成</span>
              </div>
            </div>
            <div className="reg-time-grid">
              <div className="reg-time-item">
                <span className="reg-time-label">总耗时</span>
                <span className="reg-time-value">{formatElapsed(batchElapsed)}</span>
              </div>
              <div className="reg-time-item">
                <span className="reg-time-label">平均耗时</span>
                <span className="reg-time-value">{formatElapsed(avg)}</span>
              </div>
              <div className="reg-time-item">
                <span className="reg-time-label">当前第</span>
                <span className="reg-time-value">{done + 1} / {batchProgress.total}</span>
              </div>
              <div className="reg-time-item">
                <span className="reg-time-label">当前邮箱</span>
                <span className="reg-time-value" style={{ fontSize: 11, wordBreak: "break-all" }}>
                  {batchProgress.current_email || "..."}
                </span>
              </div>
            </div>
          </div>
        );
      })()}

      {/* ── Row 1: 总览统计 (dash-grid, 4 cards) ── */}
      <div className="dash-grid">
        <div className="dash-stat-card">
          <span className="dash-stat-value">{metrics.total}</span>
          <span className="dash-stat-label">总账号数</span>
        </div>
        <div className="dash-stat-card">
          <span className="dash-stat-value">{groups.length}</span>
          <span className="dash-stat-label">分组数量</span>
        </div>
        <div className="dash-stat-card dash-stat-ok">
          <span className="dash-stat-value">{creditsStats.totalCredits.toLocaleString()}</span>
          <span className="dash-stat-label">总 Credits</span>
        </div>
        <div className="dash-stat-card">
          <span className="dash-stat-value">{creditsStats.avgCredits.toLocaleString()}</span>
          <span className="dash-stat-label">平均 Credits</span>
        </div>
      </div>

      {/* ── Row 2: Credits 总览 + 分组概览 (dash-row) ── */}
      <div className="dash-row">
        <div className="dash-section">
          <div className="dash-section-title">Credits 总览</div>
          <div className="dash-usage-card">
            <div className="dash-usage-avg">
              <span className="dash-usage-avg-value">{creditsStats.totalCredits.toLocaleString()}</span>
              <span className="dash-usage-avg-label">总 Credits</span>
            </div>
            <div className="dash-usage-metrics">
              <div className="dash-usage-metric">
                <span className="dash-usage-metric-label">平均 Credits</span>
                <span className="dash-usage-metric-value">{creditsStats.avgCredits.toLocaleString()}</span>
              </div>
              <div className="dash-usage-metric">
                <span className="dash-usage-metric-label">最高 Credits</span>
                <span className="dash-usage-metric-value">{creditsStats.maxCredits.toLocaleString()}</span>
              </div>
              <div className="dash-usage-metric">
                <span className="dash-usage-metric-label">有 Credits 账号</span>
                <span className="dash-usage-metric-value">{creditsStats.creditsAccountCount}</span>
              </div>
            </div>
            <div className="dash-dist">
              <div className="dash-dist-bar">
                {scopedAccounts.length > 0 ? (
                  <>
                    <div
                      className="dash-dist-seg dash-dist-seg-ok"
                      style={{ width: `${(creditsStats.creditsAccountCount / scopedAccounts.length) * 100}%` }}
                    />
                    <div
                      className="dash-dist-seg dash-dist-seg-unknown"
                      style={{ width: `${(creditsStats.noCreditsCount / scopedAccounts.length) * 100}%` }}
                    />
                  </>
                ) : null}
              </div>
              <div className="dash-dist-legend">
                <span className="dash-dist-legend-item"><i className="dash-dist-dot dash-dist-dot-ok" /> 有 Credits</span>
                <span className="dash-dist-legend-item"><i className="dash-dist-dot dash-dist-dot-unknown" /> 无 Credits</span>
              </div>
            </div>
          </div>
        </div>
        {/* 分组概览 */}
        <div className="dash-section">
          <div className="dash-section-title">
            <FolderOpen size={14} style={{ verticalAlign: -2, marginRight: 6 }} />
            分组概览
          </div>
          {groupStats.length === 0 ? (
            <div className="dash-empty">暂无分组数据</div>
          ) : (
            <div className="dash-group-list">
              {groupStats.map((g, i) => (
                <div key={i} className="dash-group-item">
                  <span className="dash-group-name">{g.name}</span>
                  <div className="dash-group-bar-bg">
                    <div className="dash-group-bar-fill" style={{ width: `${(g.total / (groupStats[0]?.total || 1)) * 100}%` }} />
                  </div>
                  <span className="dash-group-count">{g.total}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* ── Credits 区间分布 ── */}
      <div className="dash-section">
        <div className="dash-section-title">Credits 区间分布</div>
        {creditsStats.buckets.every((b) => b.count === 0) ? (
          <div className="dash-empty">暂无 Credits 数据</div>
        ) : (
          <div className="dash-credit-buckets">
            {creditsStats.buckets.map((bucket) => {
              const pct = creditsStats.creditsAccountCount > 0
                ? ((bucket.count / creditsStats.creditsAccountCount) * 100).toFixed(1)
                : "0";
              return (
                <div key={bucket.label} className="dash-credit-bucket-col">
                  <div className="dash-credit-bucket-track">
                    <div
                      className="dash-credit-bucket-fill"
                      style={{ height: `${(bucket.count / creditsBucketMax) * 100}%` }}
                    />
                    <div className="dash-credit-bucket-tooltip">
                      <div>{bucket.label} credits</div>
                      <div>{bucket.count} 个账号 ({pct}%)</div>
                    </div>
                  </div>
                  <span className="dash-credit-bucket-count">{bucket.count}</span>
                  <span className="dash-credit-bucket-label">{bucket.label}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* ── Row 7: 最近注册 ── */}
      <div className="dash-section">
        <div className="dash-section-title">最近注册</div>
        <div className="table-container" style={{ boxShadow: "none", border: "none", background: "transparent" }}>
          <table className="codex-table">
            <thead>
              <tr>
                <th>邮箱</th>
                <th>状态</th>
                <th>分组</th>
                <th>套餐</th>
                <th>创建时间</th>
                <th>Credits</th>
              </tr>
            </thead>
            <tbody>
              {recentAccounts.length === 0 ? (
                <tr><td colSpan={6} className="empty-msg">暂无账号数据</td></tr>
              ) : (
                recentAccounts.map((account) => (
                  <tr key={account.id}>
                    <td className="email-cell">{account.email}</td>
                    <td>
                      <span className={`plan-badge plan-badge-${account.status === "complete" ? "team" : account.status === "failed" ? "free" : "plus"}`}>
                        {account.status === "complete" ? "成功" : account.status === "failed" ? "失败" : account.status === "running" ? "运行中" : "等待中"}
                      </span>
                    </td>
                    <td style={{ fontSize: 12, color: "var(--muted)" }}>{account.group_name}</td>
                    <td>
                      {account.plan ? (
                        <span className={`plan-badge plan-badge-${(account.plan || "unknown").toLowerCase()}`}>{account.plan}</span>
                      ) : (
                        <span style={{ color: "var(--muted)", opacity: 0.4 }}>-</span>
                      )}
                    </td>
                    <td style={{ fontSize: 12, color: "var(--muted)" }}>{account.created_at}</td>
                    <td style={{ fontSize: 12, color: "#2b4d76", fontFamily: '"JetBrains Mono", Consolas, monospace' }}>
                      {typeof account.credits === "number" ? account.credits.toLocaleString() : "-"}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </>
  );
}
