import { Loader2, Power, ShieldCheck, X } from "lucide-react";
import type { ManagedServiceName, ServiceStatus } from "../lib/types";

const serviceOrder: ManagedServiceName[] = ["mail_gateway", "turnstile_solver"];

const serviceMeta: Record<ManagedServiceName, { title: string; description: string }> = {
  mail_gateway: {
    title: "Mail Gateway",
    description: "邮箱申请与收码服务",
  },
  turnstile_solver: {
    title: "TurnstileSolver",
    description: "本地验证码求解服务",
  },
};

function describeServiceSource(source?: ServiceStatus["source"]): string {
  switch (source) {
    case "desktop_managed":
      return "桌面端托管";
    case "external":
      return "外部运行";
    default:
      return "未运行";
  }
}

export default function CloseAppDialog({
  open,
  loading,
  services,
  busyService,
  error,
  onStopService,
  onCancel,
  onExit,
}: {
  open: boolean;
  loading: boolean;
  services: Record<ManagedServiceName, ServiceStatus | null>;
  busyService: ManagedServiceName | null;
  error: string | null;
  onStopService: (service: ManagedServiceName) => void;
  onCancel: () => void;
  onExit: () => void;
}) {
  if (!open) return null;

  const canExit =
    !loading &&
    busyService === null &&
    serviceOrder.every((service) => services[service] && !services[service]!.running);

  return (
    <div className="modal-overlay animate-fade-in">
      <div
        className="modal-content animate-rise close-app-dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <h3>退出应用前先关闭服务</h3>
            <p className="close-app-copy">
              必须手动停止下面两个服务，两个都停掉后才允许退出应用。
            </p>
          </div>
          <button className="modal-close" onClick={onCancel} disabled={busyService !== null}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body close-app-body">
          {serviceOrder.map((service) => {
            const status = services[service];
            const meta = serviceMeta[service];
            const stopping = busyService === service;
            const running = !!status?.running;

            return (
              <div key={service} className="close-app-service">
                <div className="close-app-service-head">
                  <div>
                    <div className="close-app-service-title">{meta.title}</div>
                    <div className="close-app-service-desc">{meta.description}</div>
                  </div>
                  <span className={`service-pill ${running ? "running" : "stopped"}`}>
                    {running ? "运行中" : "已停止"}
                  </span>
                </div>

                <div className="service-meta">
                  <span>来源: {describeServiceSource(status?.source)}</span>
                  <span>PID: {status?.pid ?? "-"}</span>
                  <span>最近启动: {status?.last_started_at || "-"}</span>
                </div>

                {status?.last_error ? <div className="service-error">{status.last_error}</div> : null}

                <div className="close-app-service-actions">
                  <button
                    type="button"
                    className="btn btn-sm btn-danger"
                    disabled={loading || stopping || !running}
                    onClick={() => onStopService(service)}
                  >
                    {stopping ? <Loader2 size={14} className="animate-spin" /> : <Power size={14} />}
                    关闭服务
                  </button>
                </div>
              </div>
            );
          })}

          {loading ? (
            <div className="close-app-status">
              <Loader2 size={14} className="animate-spin" />
              正在读取服务状态...
            </div>
          ) : null}
          {error ? <div className="service-error">{error}</div> : null}
        </div>

        <div className="close-app-footer">
          <button className="btn btn-clear btn-sm" disabled={busyService !== null} onClick={onCancel}>
            取消
          </button>
          <button className="btn btn-sm" disabled={!canExit} onClick={onExit}>
            <ShieldCheck size={14} />
            退出应用
          </button>
        </div>
      </div>
    </div>
  );
}
