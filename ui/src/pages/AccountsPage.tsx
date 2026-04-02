import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import {
  RefreshCw,
  Trash2,
  ChevronLeft,
  ChevronRight,
  CheckCircle2,
  XCircle,
  Clock,
  Loader2,
  X,
  Copy,
  Check,
  FolderInput,
  Download,
} from "lucide-react";
import {
  getAccounts,
  refreshAccountsProfileMissing,
  refreshAccountProfile,
  deleteAccount,
  deleteAccounts,
  listAccountGroups,
  moveAccountsToGroup,
  saveTextFile,
} from "@/lib/tauri-api";
import type { Account, AccountGroup } from "@/lib/types";

const statusConfig: Record<string, { label: string; icon: typeof Clock; className: string }> = {
  pending: { label: "等待中", icon: Clock, className: "text-muted" },
  running: { label: "运行中", icon: Loader2, className: "text-accent" },
  complete: { label: "成功", icon: CheckCircle2, className: "text-ok" },
  failed: { label: "失败", icon: XCircle, className: "text-danger" },
};

const PAGE_SIZE = 20;

function MoveGroupDialog({
  open,
  onClose,
  groups,
  targetGroupId,
  setTargetGroupId,
  onConfirm,
  moving,
  showMoveCount,
  moveCount,
  maxMoveCount,
  setMoveCount,
}: {
  open: boolean;
  onClose: () => void;
  groups: AccountGroup[];
  targetGroupId: number | null;
  setTargetGroupId: (id: number) => void;
  onConfirm: () => void;
  moving: boolean;
  showMoveCount: boolean;
  moveCount: number;
  maxMoveCount: number;
  setMoveCount: (n: number) => void;
}) {
  if (!open) return null;

  return (
    <div className="modal-overlay animate-fade-in" onClick={onClose}>
      <div className="modal-content animate-rise" style={{ width: "min(720px, 100%)" }} onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>移动账号分组</h3>
          <button className="modal-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body" style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          <div className="form-group" style={{ marginBottom: 0 }}>
            <label>目标分组</label>
            <select
              className="input"
              value={targetGroupId ?? ""}
              onChange={(e) => setTargetGroupId(Number(e.target.value))}
              style={{ width: "100%", maxWidth: 340 }}
            >
              {groups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </select>
          </div>

          {showMoveCount && (
            <div className="config-panel" style={{ padding: 12 }}>
              <div className="settings-title" style={{ marginBottom: 6 }}>移动数量</div>
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <input
                  type="number"
                  className="input input-sm"
                  min={1}
                  max={maxMoveCount}
                  value={moveCount}
                  onChange={(e) => {
                    const raw = Number(e.target.value);
                    if (Number.isNaN(raw)) return;
                    const next = Math.max(1, Math.min(maxMoveCount, Math.floor(raw)));
                    setMoveCount(next);
                  }}
                />
                <span style={{ color: "#2f4d73", fontSize: 13, fontWeight: 700 }}>
                  / {maxMoveCount} 个账号
                </span>
              </div>
              <div style={{ marginTop: 6, color: "var(--muted)", fontSize: 12 }}>
                将按当前筛选结果从上到下移动前 {moveCount} 个账号
              </div>
            </div>
          )}
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-clear btn-sm" onClick={onClose}>取消</button>
          <button className="btn btn-sm" onClick={onConfirm} disabled={moving || !targetGroupId}>
            {moving ? <Loader2 size={14} className="animate-spin" /> : <FolderInput size={14} />}
            确认移动
          </button>
        </div>
      </div>
    </div>
  );
}

type ExportFormat = "cookie" | "orchids-tool";

function ExportCookiesDialog({
  open,
  onClose,
  onConfirm,
  exporting,
  showExportCount,
  exportCount,
  maxExportCount,
  setExportCount,
  selectedCount,
  exportFormat,
  setExportFormat,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  exporting: boolean;
  showExportCount: boolean;
  exportCount: number;
  maxExportCount: number;
  setExportCount: (n: number) => void;
  selectedCount: number;
  exportFormat: ExportFormat;
  setExportFormat: (f: ExportFormat) => void;
}) {
  if (!open) return null;

  const formatOptions: { key: ExportFormat; label: string; desc: string }[] = [
    { key: "cookie", label: "Client Cookie", desc: "仅导出 client_cookie 字段（JSON 数组）" },
    { key: "orchids-tool", label: "Orchids Tool", desc: "导出含 email、password、plan_name、user_id、client_cookie、credits、status 的完整信息" },
  ];

  return (
    <div className="modal-overlay animate-fade-in" onClick={onClose}>
      <div className="modal-content animate-rise" style={{ width: "min(620px, 100%)" }} onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>导出账号数据</h3>
          <button className="modal-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {/* 格式选择 */}
          <div>
            <div className="settings-title" style={{ marginBottom: 8 }}>导出格式</div>
            <div style={{ display: "flex", gap: 10 }}>
              {formatOptions.map((opt) => (
                <div
                  key={opt.key}
                  onClick={() => setExportFormat(opt.key)}
                  style={{
                    flex: 1,
                    padding: "10px 14px",
                    borderRadius: 8,
                    border: exportFormat === opt.key ? "2px solid var(--accent, #5b8cde)" : "2px solid var(--border, #d0d7de)",
                    background: exportFormat === opt.key ? "var(--accent-bg, rgba(91,140,222,0.08))" : "transparent",
                    cursor: "pointer",
                    transition: "all 0.15s ease",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                    <div style={{
                      width: 16, height: 16, borderRadius: "50%",
                      border: exportFormat === opt.key ? "5px solid var(--accent, #5b8cde)" : "2px solid var(--muted, #8b949e)",
                      background: "var(--bg, #fff)",
                      flexShrink: 0,
                    }} />
                    <span style={{ fontSize: 13, fontWeight: 700, color: "var(--fg, #1f2328)" }}>{opt.label}</span>
                  </div>
                  <div style={{ fontSize: 11, color: "var(--muted, #8b949e)", paddingLeft: 24 }}>{opt.desc}</div>
                </div>
              ))}
            </div>
          </div>

          {/* 导出数量 */}
          {showExportCount ? (
            <div className="config-panel" style={{ padding: 12 }}>
              <div className="settings-title" style={{ marginBottom: 6 }}>导出数量</div>
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <input
                  type="number"
                  className="input input-sm"
                  min={1}
                  max={maxExportCount}
                  value={exportCount}
                  onChange={(e) => {
                    const raw = Number(e.target.value);
                    if (Number.isNaN(raw)) return;
                    const next = Math.max(1, Math.min(maxExportCount, Math.floor(raw)));
                    setExportCount(next);
                  }}
                />
                <span style={{ color: "#2f4d73", fontSize: 13, fontWeight: 700 }}>
                  / {maxExportCount} 个账号
                </span>
              </div>
              <div style={{ marginTop: 6, color: "var(--muted)", fontSize: 12 }}>
                将按当前筛选结果从上到下导出前 {exportCount} 个账号的数据
              </div>
            </div>
          ) : (
            <div className="config-panel" style={{ padding: 12, color: "#2f4d73", fontSize: 13, fontWeight: 700 }}>
              将导出已勾选的 {selectedCount} 个账号的数据
            </div>
          )}
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-clear btn-sm" onClick={onClose}>取消</button>
          <button className="btn btn-sm" onClick={onConfirm} disabled={exporting}>
            {exporting ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}
            确认导出
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Detail Dialog (Codex-style modal) ---
function DetailDialog({ account, onClose }: { account: Account; onClose: () => void }) {
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const overlayRef = useRef<HTMLDivElement>(null);

  const copyToClipboard = (text: string, field: string) => {
    navigator.clipboard.writeText(text);
    setCopiedField(field);
    setTimeout(() => setCopiedField(null), 1500);
  };

  const status = statusConfig[account.status] || statusConfig.pending;

  const fields: { label: string; key: string; value: string | null | undefined; mono?: boolean; copyable?: boolean }[] = [
    { label: "ID", key: "id", value: String(account.id) },
    { label: "分组", key: "group_name", value: account.group_name },
    { label: "邮箱", key: "email", value: account.email, mono: true, copyable: true },
    { label: "密码", key: "password", value: account.password, mono: true, copyable: true },
    { label: "Sign Up ID", key: "sign_up_id", value: account.sign_up_id, mono: true, copyable: true },
    { label: "验证码", key: "email_code", value: account.email_code, mono: true },
    { label: "注册完成", key: "register_complete", value: account.register_complete ? "是" : "否" },
    { label: "Session ID", key: "created_session_id", value: account.created_session_id, mono: true, copyable: true },
    { label: "User ID", key: "created_user_id", value: account.created_user_id, mono: true, copyable: true },
    { label: "Client Cookie", key: "client_cookie", value: account.client_cookie, mono: true, copyable: true },
    { label: "错误信息", key: "error_message", value: account.error_message },
    { label: "套餐", key: "plan", value: account.plan },
    { label: "Credits", key: "credits", value: account.credits === null ? null : account.credits.toLocaleString() },
    { label: "批次 ID", key: "batch_id", value: account.batch_id, mono: true },
    { label: "创建时间", key: "created_at", value: account.created_at },
    { label: "更新时间", key: "updated_at", value: account.updated_at },
  ];

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  return (
    <div
      ref={overlayRef}
      className="modal-overlay animate-fade-in"
      onClick={(e) => { if (e.target === overlayRef.current) onClose(); }}
    >
      <div className="modal-content animate-rise" style={{ width: "min(620px, 100%)" }}>
        <div className="modal-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <h3>账号详情</h3>
            <span style={{ fontSize: 12, color: `var(--${account.status === "complete" ? "ok" : account.status === "failed" ? "danger" : "muted"})` }}>
              {status.label}
            </span>
          </div>
          <button className="modal-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          {fields.map(({ label, key, value, mono, copyable }) => {
            const display = value ?? "-";
            const isEmpty = value === null || value === undefined || value === "";
            return (
              <div key={key} className="detail-row">
                <span className="detail-label">{label}</span>
                <div style={{ display: "flex", alignItems: "flex-start", gap: 8, minWidth: 0 }}>
                  <span
                    style={{
                      fontSize: mono && !isEmpty ? 11 : 13,
                      color: isEmpty ? "var(--muted)" : "#2f4d73",
                      overflowWrap: "anywhere" as const,
                      fontFamily: mono && !isEmpty ? '"JetBrains Mono", Consolas, monospace' : undefined,
                      opacity: isEmpty ? 0.4 : 1,
                    }}
                  >
                    {isEmpty ? "-" : display}
                  </span>
                  {copyable && !isEmpty && (
                    <button
                      onClick={() => copyToClipboard(display, key)}
                      style={{
                        flexShrink: 0,
                        padding: 2,
                        border: "none",
                        background: "transparent",
                        cursor: "pointer",
                        color: copiedField === key ? "var(--ok)" : "var(--muted)",
                        transition: "color 0.15s",
                      }}
                      title="复制"
                    >
                      {copiedField === key ? <Check size={13} /> : <Copy size={13} />}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

// --- Main Page ---
export default function AccountsPage() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [loading, setLoading] = useState(false);
  const [refreshingProfiles, setRefreshingProfiles] = useState(false);
  const [groupFilter, setGroupFilter] = useState<number | "all" | null>(null);
  const [emailKeyword, setEmailKeyword] = useState("");
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [page, setPage] = useState(0);
  const [detailAccount, setDetailAccount] = useState<Account | null>(null);
  const [moveOpen, setMoveOpen] = useState(false);
  const [moveTargetGroupId, setMoveTargetGroupId] = useState<number | null>(null);
  const [moveCount, setMoveCount] = useState(1);
  const [moving, setMoving] = useState(false);
  const [refreshingAccountId, setRefreshingAccountId] = useState<number | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [exportCount, setExportCount] = useState(1);
  const [exporting, setExporting] = useState(false);
  const [exportFormat, setExportFormat] = useState<ExportFormat>("cookie");
  const refreshingProfilesRef = useRef(false);

  const defaultGroup = groups.find((g) => g.is_default) || groups[0];
  const currentFilterGroupId = groupFilter === "all" || groupFilter === null ? undefined : groupFilter;

  const fetchGroups = useCallback(async () => {
    try {
      const list = await listAccountGroups();
      setGroups(list);
      setGroupFilter((prev) => {
        if (prev === "all") return "all";
        if (typeof prev === "number" && list.some((g) => g.id === prev)) return prev;
        const def = list.find((g) => g.is_default) || list[0];
        return def ? def.id : "all";
      });
    } catch (e) {
      console.error("获取分组失败:", e);
    }
  }, []);

  const refreshMissingProfiles = useCallback(async () => {
    if (refreshingProfilesRef.current) return;
    refreshingProfilesRef.current = true;
    setRefreshingProfiles(true);
    try {
      for (let i = 0; i < 5; i += 1) {
        const refreshed = await refreshAccountsProfileMissing(10);
        if (refreshed === 0) break;
        const latest = await getAccounts(undefined, currentFilterGroupId);
        setAccounts(latest);
      }
    } catch (e) {
      console.error("自动刷新 plan/credits 失败:", e);
    } finally {
      refreshingProfilesRef.current = false;
      setRefreshingProfiles(false);
    }
  }, [currentFilterGroupId]);

  const fetchAccounts = useCallback(async () => {
    if (groupFilter === null) return;
    setLoading(true);
    try {
      const data = await getAccounts(undefined, currentFilterGroupId);
      setAccounts(data);
      setSelectedIds(new Set());
      void refreshMissingProfiles();
    } catch (e) {
      console.error("获取账号失败:", e);
    } finally {
      setLoading(false);
    }
  }, [groupFilter, currentFilterGroupId, refreshMissingProfiles]);

  useEffect(() => {
    void fetchGroups();
  }, [fetchGroups]);

  useEffect(() => {
    void fetchAccounts();
  }, [fetchAccounts]);

  useEffect(() => {
    const onAccountsChanged = () => { void fetchAccounts(); };
    const onGroupsChanged = () => {
      void fetchGroups();
      void fetchAccounts();
    };
    window.addEventListener("accounts-changed", onAccountsChanged);
    window.addEventListener("groups-changed", onGroupsChanged);
    return () => {
      window.removeEventListener("accounts-changed", onAccountsChanged);
      window.removeEventListener("groups-changed", onGroupsChanged);
    };
  }, [fetchAccounts, fetchGroups]);

  const filteredAccounts = useMemo(() => {
    const kw = emailKeyword.trim().toLowerCase();
    if (!kw) return accounts;
    return accounts.filter((a) => a.email.toLowerCase().includes(kw));
  }, [accounts, emailKeyword]);

  const totalPages = Math.ceil(filteredAccounts.length / PAGE_SIZE);
  const pagedAccounts = filteredAccounts.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);

  const handleDelete = async (id: number) => {
    try {
      await deleteAccount(id);
      await fetchAccounts();
      window.dispatchEvent(new CustomEvent("accounts-changed"));
    } catch (e) {
      console.error("删除失败:", e);
    }
  };

  const handleBatchDelete = async () => {
    if (selectedIds.size === 0) return;
    try {
      await deleteAccounts(Array.from(selectedIds));
      await fetchAccounts();
      window.dispatchEvent(new CustomEvent("accounts-changed"));
    } catch (e) {
      console.error("批量删除失败:", e);
    }
  };

  const handleRefreshAccount = async (id: number) => {
    setRefreshingAccountId(id);
    try {
      const updated = await refreshAccountProfile(id);
      setAccounts((prev) => prev.map((acc) => (acc.id === updated.id ? updated : acc)));
    } catch (e: any) {
      alert(`刷新失败: ${String(e)}`);
    } finally {
      setRefreshingAccountId(null);
    }
  };

  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selectedIds.size === pagedAccounts.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(pagedAccounts.map((a) => a.id)));
    }
  };

  const openMoveDialog = () => {
    const fallbackGroup = groups.find((g) => !g.is_default) || defaultGroup || groups[0];
    setMoveTargetGroupId(fallbackGroup?.id ?? null);
    setMoveCount(Math.max(1, filteredAccounts.length));
    setMoveOpen(true);
  };

  const openExportDialog = () => {
    setExportCount(Math.max(1, filteredAccounts.length));
    setExportOpen(true);
  };

  const handleConfirmExport = async () => {
    setExporting(true);
    try {
      const source = selectedIds.size > 0
        ? accounts.filter((a) => selectedIds.has(a.id))
        : filteredAccounts.slice(0, Math.max(1, Math.min(exportCount, filteredAccounts.length)));

      if (exportFormat === "orchids-tool") {
        const rows = source
          .filter((a) => a.client_cookie && a.client_cookie.trim().length > 0)
          .map((a) => ({
            email: a.email,
            password: a.password,
            plan_name: a.plan || "FREE",
            user_id: a.created_user_id || "",
            client_cookie: (a.client_cookie || "").trim().replace(/^[\"\[\]]+|[\"\[\]]+$/g, "").trim(),
            credits: a.credits ?? 0,
            status: a.status === "complete" ? "valid" : a.status,
          }));

        if (rows.length === 0) {
          alert("没有可导出的账号数据");
          return;
        }

        const now = new Date();
        const pad = (n: number) => String(n).padStart(2, "0");
        const fileName = `Orchids_tool_${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}.json`;

        const saved = await saveTextFile(JSON.stringify(rows, null, 2), fileName);
        if (saved) {
          setExportOpen(false);
        }
      } else {
        const rows = source
          .map((a) => (a.client_cookie || "").trim().replace(/^[\"\[\]]+|[\"\[\]]+$/g, "").trim())
          .filter((c) => c.length > 0);

        if (rows.length === 0) {
          alert("没有可导出的 client cookie 数据");
          return;
        }

        const now = new Date();
        const pad = (n: number) => String(n).padStart(2, "0");
        const fileName = `Orchids_client_cookies_${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}.json`;

        const saved = await saveTextFile(JSON.stringify(rows, null, 2), fileName);
        if (saved) {
          setExportOpen(false);
        }
      }
    } catch (e) {
      console.error("导出失败:", e);
      alert(`导出失败: ${String(e)}`);
    } finally {
      setExporting(false);
    }
  };

  const handleConfirmMove = async () => {
    if (!moveTargetGroupId) return;
    const ids = selectedIds.size > 0
      ? Array.from(selectedIds)
      : filteredAccounts.slice(0, Math.max(1, Math.min(moveCount, filteredAccounts.length))).map((a) => a.id);
    if (ids.length === 0) return;
    setMoving(true);
    try {
      await moveAccountsToGroup(ids, moveTargetGroupId);
      setMoveOpen(false);
      setSelectedIds(new Set());
      await fetchAccounts();
      window.dispatchEvent(new CustomEvent("accounts-changed"));
    } catch (e: any) {
      alert(`移动失败: ${String(e)}`);
    } finally {
      setMoving(false);
    }
  };

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap" }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
          <h1 className="page-title">账号管理</h1>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center", marginLeft: "auto", flexWrap: "wrap" }}>
          <input
            type="text"
            className="input"
            style={{ width: 260 }}
            placeholder="搜索邮箱地址..."
            value={emailKeyword}
            onChange={(e) => {
              setEmailKeyword(e.target.value);
              setPage(0);
            }}
          />

          <select
            value={groupFilter ?? ""}
            onChange={(e) => {
              const v = e.target.value;
              setGroupFilter(v === "all" ? "all" : Number(v));
              setPage(0);
            }}
            className="input"
            style={{ width: "auto", minWidth: 150, cursor: "pointer" }}
          >
            <option value="all">全部账号</option>
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {group.name}
              </option>
            ))}
          </select>

          <button
            className="btn btn-clear btn-sm"
            onClick={fetchAccounts}
            title="刷新"
            style={{ width: 34, height: 34, padding: 0, justifyContent: "center" }}
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          </button>

          <button
            className="btn btn-clear btn-sm"
            onClick={openMoveDialog}
            disabled={filteredAccounts.length === 0}
            title={selectedIds.size > 0 ? `移动分组（${selectedIds.size}）` : "移动分组"}
            style={{ width: 34, height: 34, padding: 0, justifyContent: "center" }}
          >
            <FolderInput size={14} />
          </button>

          <button
            className="btn btn-clear btn-sm"
            onClick={openExportDialog}
            disabled={filteredAccounts.length === 0}
            title={selectedIds.size > 0 ? `导出 client cookie（${selectedIds.size}）` : "导出 client cookie（JSON 行格式）"}
            style={{ width: 34, height: 34, padding: 0, justifyContent: "center" }}
          >
            <Download size={14} />
          </button>

          {refreshingProfiles && (
            <span style={{ fontSize: 11, color: "var(--muted)" }}>刷新中...</span>
          )}

          {selectedIds.size > 0 && (
            <button
              className="btn btn-sm"
              onClick={handleBatchDelete}
              title={`删除 (${selectedIds.size})`}
              style={{ background: "linear-gradient(135deg, #ef4444, #dc2626)", color: "#fff", borderColor: "#dc2626", width: 34, height: 34, padding: 0, justifyContent: "center", position: "relative" }}
            >
              <Trash2 size={14} />
              <span style={{ position: "absolute", top: -6, right: -6, background: "#fff", color: "#dc2626", fontSize: 10, fontWeight: 700, borderRadius: "50%", width: 18, height: 18, display: "flex", alignItems: "center", justifyContent: "center", border: "1.5px solid #dc2626" }}>{selectedIds.size}</span>
            </button>
          )}
        </div>
      </div>

      <div className="table-container" style={{ flex: 1 }}>
        <table className="codex-table">
          <thead>
            <tr>
              <th style={{ width: 40, textAlign: "center" }}>
                <input
                  type="checkbox"
                  checked={pagedAccounts.length > 0 && selectedIds.size === pagedAccounts.length}
                  onChange={toggleSelectAll}
                  style={{ accentColor: "var(--accent)" }}
                />
              </th>
              <th>ID</th>
              <th>邮箱</th>
              <th>状态</th>
              <th>套餐</th>
              <th>Credits</th>
              <th>创建时间</th>
              <th style={{ width: 88, textAlign: "center" }}>操作</th>
            </tr>
          </thead>
          <tbody>
            {pagedAccounts.length === 0 ? (
              <tr>
                <td colSpan={8} style={{ textAlign: "center", color: "var(--muted)", padding: "28px 12px" }}>
                  {loading ? "加载中..." : "暂无账号数据"}
                </td>
              </tr>
            ) : (
              pagedAccounts.map((account) => {
                const st = statusConfig[account.status] || statusConfig.pending;
                const StIcon = st.icon;
                return (
                  <tr
                    key={account.id}
                    style={{ cursor: "pointer" }}
                    onClick={(e) => {
                      const target = e.target as HTMLElement;
                      if (target.closest("input, button")) return;
                      setDetailAccount(account);
                    }}
                  >
                    <td style={{ textAlign: "center" }}>
                      <input
                        type="checkbox"
                        checked={selectedIds.has(account.id)}
                        onChange={() => toggleSelect(account.id)}
                        style={{ accentColor: "var(--accent)" }}
                      />
                    </td>
                    <td style={{ color: "var(--muted)" }}>{account.id}</td>
                    <td className="email-cell">{account.email}</td>
                    <td>
                      <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 600 }} className={st.className}>
                        <StIcon size={14} />
                        {st.label}
                      </span>
                    </td>
                    <td>
                      {account.plan ? (
                        <span className={`plan-badge plan-badge-${(account.plan || "unknown").toLowerCase()}`}>
                          {account.plan}
                        </span>
                      ) : (
                        <span style={{ color: "var(--muted)", opacity: 0.4 }}>-</span>
                      )}
                    </td>
                    <td style={{ fontFamily: '"JetBrains Mono", Consolas, monospace', fontSize: 12 }}>
                      {account.credits === null ? (
                        <span style={{ color: "var(--muted)", opacity: 0.4 }}>-</span>
                      ) : (
                        account.credits.toLocaleString()
                      )}
                    </td>
                    <td style={{ fontSize: 12, color: "var(--muted)" }}>{account.created_at}</td>
                    <td style={{ textAlign: "center" }}>
                      <div style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                        <button
                          onClick={() => handleRefreshAccount(account.id)}
                          style={{
                            padding: 4,
                            border: "none",
                            background: "transparent",
                            cursor: "pointer",
                            color: "var(--muted)",
                            borderRadius: 6,
                            transition: "color 0.15s, background 0.15s",
                          }}
                          onMouseOver={(e) => { e.currentTarget.style.color = "var(--accent)"; e.currentTarget.style.background = "rgba(29,125,242,0.1)"; }}
                          onMouseOut={(e) => { e.currentTarget.style.color = "var(--muted)"; e.currentTarget.style.background = "transparent"; }}
                          title="刷新套餐和 Credits"
                          disabled={refreshingAccountId === account.id}
                        >
                          <RefreshCw size={14} className={refreshingAccountId === account.id ? "animate-spin" : ""} />
                        </button>
                        <button
                          onClick={() => handleDelete(account.id)}
                          style={{
                            padding: 4,
                            border: "none",
                            background: "transparent",
                            cursor: "pointer",
                            color: "var(--muted)",
                            borderRadius: 6,
                            transition: "color 0.15s, background 0.15s",
                          }}
                          onMouseOver={(e) => { e.currentTarget.style.color = "var(--danger)"; e.currentTarget.style.background = "rgba(224,69,69,0.1)"; }}
                          onMouseOut={(e) => { e.currentTarget.style.color = "var(--muted)"; e.currentTarget.style.background = "transparent"; }}
                          title="删除"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div className="status-bar" style={{ flex: 1 }}>
          共 {filteredAccounts.length} 个账号
          {totalPages > 1 && ` · 第 ${page + 1}/${totalPages} 页`}
        </div>
        {totalPages > 1 && (
          <div style={{ display: "flex", gap: 6, marginLeft: 10 }}>
            <button
              onClick={() => setPage((p) => Math.max(0, p - 1))}
              disabled={page === 0}
              className="btn btn-clear btn-sm"
              style={{ padding: "4px 8px" }}
            >
              <ChevronLeft size={16} />
            </button>
            <button
              onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={page >= totalPages - 1}
              className="btn btn-clear btn-sm"
              style={{ padding: "4px 8px" }}
            >
              <ChevronRight size={16} />
            </button>
          </div>
        )}
      </div>

      {detailAccount && (
        <DetailDialog
          account={detailAccount}
          onClose={() => setDetailAccount(null)}
        />
      )}

      <MoveGroupDialog
        open={moveOpen}
        onClose={() => setMoveOpen(false)}
        groups={groups}
        targetGroupId={moveTargetGroupId}
        setTargetGroupId={setMoveTargetGroupId}
        onConfirm={handleConfirmMove}
        moving={moving}
        showMoveCount={selectedIds.size === 0}
        moveCount={selectedIds.size === 0 ? moveCount : selectedIds.size}
        maxMoveCount={filteredAccounts.length}
        setMoveCount={setMoveCount}
      />

      <ExportCookiesDialog
        open={exportOpen}
        onClose={() => setExportOpen(false)}
        onConfirm={handleConfirmExport}
        exporting={exporting}
        showExportCount={selectedIds.size === 0}
        exportCount={selectedIds.size === 0 ? exportCount : selectedIds.size}
        maxExportCount={filteredAccounts.length}
        setExportCount={setExportCount}
        selectedCount={selectedIds.size}
        exportFormat={exportFormat}
        setExportFormat={setExportFormat}
      />
    </>
  );
}
