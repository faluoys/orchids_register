import { useCallback, useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Loader2, Pencil, Pin, PinOff, Plus, Save, Trash2, X } from "lucide-react";
import {
  createAccountGroup,
  deleteAccountGroup,
  listAccountGroups,
  moveAccountGroup,
  renameAccountGroup,
  setAccountGroupPinned,
} from "@/lib/tauri-api";
import type { AccountGroup } from "@/lib/types";

export default function GroupManagePage() {
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState("");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingName, setEditingName] = useState("");
  const [busyId, setBusyId] = useState<number | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadGroups = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listAccountGroups();
      setGroups(data);
      setError(null);
    } catch (e: any) {
      console.error("获取分组失败:", e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadGroups();
  }, [loadGroups]);

  const emitGroupChanged = () => {
    window.dispatchEvent(new CustomEvent("groups-changed"));
    window.dispatchEvent(new CustomEvent("accounts-changed"));
  };

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) return;
    setCreating(true);
    try {
      await createAccountGroup(name);
      setNewName("");
      await loadGroups();
      emitGroupChanged();
    } catch (e: any) {
      alert(`创建失败: ${String(e)}`);
    } finally {
      setCreating(false);
    }
  };

  const handleRenameSave = async (id: number) => {
    const name = editingName.trim();
    if (!name) return;
    setBusyId(id);
    try {
      await renameAccountGroup(id, name);
      setEditingId(null);
      setEditingName("");
      await loadGroups();
      emitGroupChanged();
    } catch (e: any) {
      alert(`重命名失败: ${String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  const handleDelete = async (group: AccountGroup) => {
    if (group.is_default) return;
    if (!confirm(`确认删除分组「${group.name}」吗？该分组下账号会自动移动到默认分组。`)) return;
    setBusyId(group.id);
    try {
      await deleteAccountGroup(group.id);
      await loadGroups();
      emitGroupChanged();
    } catch (e: any) {
      alert(`删除失败: ${String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  const handlePin = async (group: AccountGroup) => {
    setBusyId(group.id);
    try {
      await setAccountGroupPinned(group.id, !group.pinned);
      await loadGroups();
      emitGroupChanged();
    } catch (e: any) {
      alert(`更新置顶失败: ${String(e)}`);
    } finally {
      setBusyId(null);
    }
  };

  const handleMove = async (group: AccountGroup, direction: "up" | "down") => {
    setBusyId(group.id);
    try {
      await moveAccountGroup(group.id, direction);
      await loadGroups();
      emitGroupChanged();
    } catch (e: any) {
      alert(`调整顺序失败: ${String(e)}`);
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
          <h1 className="page-title">分组管理</h1>
          <span className="page-subtitle">新建、删除、重命名、置顶、排序</span>
        </div>
      </div>

      {error && (
        <div className="status-bar" style={{ color: "var(--danger)" }}>
          {error}
        </div>
      )}

      <div className="config-panel">
        <div className="settings-title">新建分组</div>
        <div style={{ display: "flex", gap: 8, maxWidth: 420 }}>
          <input
            className="input"
            style={{ flex: 1 }}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="输入分组名称"
          />
          <button className="btn btn-sm" onClick={handleCreate} disabled={creating || !newName.trim()} title="新建" style={{ width: 34, height: 34, padding: 0, justifyContent: "center" }}>
            {creating ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          </button>
        </div>
      </div>

      <div className="table-container" style={{ flex: 1 }}>
        <table className="codex-table">
          <thead>
            <tr>
              <th style={{ width: 70 }}>顺序</th>
              <th>分组名</th>
              <th style={{ width: 60 }}>置顶</th>
              <th style={{ width: 180, textAlign: "center" }}>操作</th>
            </tr>
          </thead>
          <tbody>
            {groups.map((group, idx) => {
              const isEditing = editingId === group.id;
              const busy = busyId === group.id;
              return (
                <tr key={group.id}>
                  <td style={{ color: "var(--muted)" }}>{idx + 1}</td>
                  <td>
                    {isEditing ? (
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <input
                          className="input"
                          style={{ width: 220 }}
                          value={editingName}
                          onChange={(e) => setEditingName(e.target.value)}
                        />
                        <button className="btn btn-sm" onClick={() => handleRenameSave(group.id)} disabled={busy || !editingName.trim()} title="保存" style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}>
                          <Save size={13} />
                        </button>
                        <button
                          className="btn btn-clear btn-sm"
                          onClick={() => {
                            setEditingId(null);
                            setEditingName("");
                          }}
                          title="取消"
                          style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}
                        >
                          <X size={13} />
                        </button>
                      </div>
                    ) : (
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <span style={{ fontWeight: 700, color: "#2f4f76" }}>{group.name}</span>
                        {group.is_default && (
                          <span className="plan-badge plan-badge-team">默认分组</span>
                        )}
                      </div>
                    )}
                  </td>
                  <td>
                    <button className="btn btn-clear btn-sm" onClick={() => handlePin(group)} disabled={busy || group.is_default} title={group.pinned ? "取消置顶" : "置顶"} style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}>
                      {group.pinned ? <PinOff size={13} /> : <Pin size={13} />}
                    </button>
                  </td>
                  <td style={{ textAlign: "center" }}>
                    <div style={{ display: "inline-flex", gap: 6 }}>
                      {!isEditing && (
                        <button
                          className="btn btn-clear btn-sm"
                          onClick={() => {
                            setEditingId(group.id);
                            setEditingName(group.name);
                          }}
                          disabled={group.is_default}
                          title="重命名"
                          style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}
                        >
                          <Pencil size={13} />
                        </button>
                      )}
                      <button className="btn btn-clear btn-sm" onClick={() => handleMove(group, "up")} disabled={busy || group.is_default} title="上移" style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}>
                        <ArrowUp size={13} />
                      </button>
                      <button className="btn btn-clear btn-sm" onClick={() => handleMove(group, "down")} disabled={busy || group.is_default} title="下移" style={{ width: 30, height: 30, padding: 0, justifyContent: "center" }}>
                        <ArrowDown size={13} />
                      </button>
                      <button className="btn btn-sm" onClick={() => handleDelete(group)} disabled={busy || group.is_default} title="删除" style={{ background: "linear-gradient(135deg, #ef4444, #dc2626)", borderColor: "#dc2626", width: 30, height: 30, padding: 0, justifyContent: "center" }}>
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
