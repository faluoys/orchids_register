import { useCallback, useEffect, useState } from "react";
import { Check, Loader2, Pencil, Plus, Save, Trash2, X } from "lucide-react";
import { createDomain, deleteDomain, listDomains, updateDomain } from "@/lib/tauri-api";
import type { Domain } from "@/lib/types";

export default function DomainManagePage() {
  const [domains, setDomains] = useState<Domain[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingDomain, setEditingDomain] = useState("");
  const [newDomain, setNewDomain] = useState("");
  const [error, setError] = useState<string | null>(null);

  const loadDomains = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listDomains();
      setDomains(data);
      setError(null);
    } catch (e: any) {
      console.error("获取域名列表失败:", e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDomains();
  }, [loadDomains]);

  const handleCreate = async () => {
    const domain = newDomain.trim();
    if (!domain) return;
    setCreating(true);
    try {
      await createDomain(domain, true);
      setNewDomain("");
      await loadDomains();
    } catch (e: any) {
      alert(`新增域名失败: ${String(e)}`);
    } finally {
      setCreating(false);
    }
  };

  const handleSave = async (item: Domain) => {
    const domain = editingDomain.trim();
    if (!domain) return;
    setBusyId(item.id);
    try {
      await updateDomain(item.id, domain, item.enabled);
      setEditingId(null);
      setEditingDomain("");
      await loadDomains();
    } catch (e: any) {
      alert(`更新域名失败: ${String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  const handleToggleEnabled = async (item: Domain) => {
    setBusyId(item.id);
    try {
      await updateDomain(item.id, item.domain, !item.enabled);
      await loadDomains();
    } catch (e: any) {
      alert(`更新状态失败: ${String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  const handleDelete = async (item: Domain) => {
    if (!confirm(`确认删除域名「${item.domain}」吗？`)) return;
    setBusyId(item.id);
    try {
      await deleteDomain(item.id);
      await loadDomains();
    } catch (e: any) {
      alert(`删除域名失败: ${String(e)}`);
    } finally {
      setBusyId(null);
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
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, flexWrap: "wrap" }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
          <h1 className="page-title">域名管理</h1>
          <span className="page-subtitle">新增、编辑、启用/停用、删除</span>
        </div>
      </div>

      {error && (
        <div className="status-bar" style={{ color: "var(--danger)" }}>
          {error}
        </div>
      )}

      <div className="config-panel">
        <div className="settings-title">新增域名</div>
        <div style={{ display: "flex", gap: 8, maxWidth: 520 }}>
          <input
            className="input"
            style={{ flex: 1 }}
            value={newDomain}
            onChange={(e) => setNewDomain(e.target.value)}
            placeholder="例如 mail.example.com"
          />
          <button
            className="btn btn-sm"
            onClick={handleCreate}
            disabled={creating || !newDomain.trim()}
            title="新增"
            style={{ width: 34, height: 34, padding: 0, justifyContent: "center" }}
          >
            {creating ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          </button>
        </div>
      </div>

      <div className="table-container" style={{ flex: 1 }}>
        <table className="codex-table">
          <thead>
            <tr>
              <th style={{ width: 80 }}>ID</th>
              <th>域名</th>
              <th style={{ width: 120 }}>状态</th>
              <th style={{ width: 180, textAlign: "center" }}>操作</th>
            </tr>
          </thead>
          <tbody>
            {domains.map((item) => {
              const isEditing = editingId === item.id;
              const busy = busyId === item.id;
              return (
                <tr key={item.id}>
                  <td style={{ color: "var(--muted)" }}>{item.id}</td>
                  <td>
                    {isEditing ? (
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <input
                          className="input"
                          style={{ width: 320 }}
                          value={editingDomain}
                          onChange={(e) => setEditingDomain(e.target.value)}
                        />
                        <button
                          className="btn btn-sm"
                          onClick={() => handleSave(item)}
                          disabled={busy || !editingDomain.trim()}
                          title="保存"
                          style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}
                        >
                          <Save size={13} />
                        </button>
                        <button
                          className="btn btn-clear btn-sm"
                          onClick={() => {
                            setEditingId(null);
                            setEditingDomain("");
                          }}
                          title="取消"
                          style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}
                        >
                          <X size={13} />
                        </button>
                      </div>
                    ) : (
                      <span style={{ fontWeight: 700, color: "#2f4f76" }}>{item.domain}</span>
                    )}
                  </td>
                  <td>
                    <span className={item.enabled ? "plan-badge plan-badge-team" : "plan-badge"}>
                      {item.enabled ? "已启用" : "已停用"}
                    </span>
                  </td>
                  <td style={{ textAlign: "center" }}>
                    <div style={{ display: "inline-flex", gap: 6 }}>
                      {!isEditing && (
                        <button
                          className="btn btn-clear btn-sm"
                          onClick={() => {
                            setEditingId(item.id);
                            setEditingDomain(item.domain);
                          }}
                          title="编辑"
                          style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}
                        >
                          <Pencil size={13} />
                        </button>
                      )}
                      <button
                        className="btn btn-clear btn-sm"
                        onClick={() => handleToggleEnabled(item)}
                        disabled={busy}
                        title={item.enabled ? "停用" : "启用"}
                        style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}
                      >
                        <Check size={13} />
                      </button>
                      <button
                        className="btn btn-sm"
                        onClick={() => handleDelete(item)}
                        disabled={busy}
                        title="删除"
                        style={{ background: "linear-gradient(135deg, #ef4444, #dc2626)", borderColor: "#dc2626", width: 30, height: 30, padding: 0, justifyContent: "center" }}
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </>
  );
}
