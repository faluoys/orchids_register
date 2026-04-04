use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use chrono::Local;
use serde::Serialize;
use serde_json::Value;

pub const MAIL_GATEWAY_SERVICE: &str = "mail_gateway";
pub const TURNSTILE_SOLVER_SERVICE: &str = "turnstile_solver";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub workdir: PathBuf,
    pub probe_target: ProbeTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeTarget {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ServiceStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub last_started_at: Option<String>,
    pub last_error: Option<String>,
    pub source: ServiceSource,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceSource {
    #[default]
    Stopped,
    DesktopManaged,
    External,
}

impl ServiceStatus {
    pub fn mark_started(&mut self, pid: Option<u32>) {
        self.running = true;
        self.pid = pid;
        self.last_started_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        self.last_error = None;
        self.source = ServiceSource::DesktopManaged;
    }

    pub fn mark_stopped(&mut self, last_error: Option<String>) {
        self.running = false;
        self.pid = None;
        self.last_started_at = None;
        self.source = ServiceSource::Stopped;
        if let Some(message) = last_error {
            self.last_error = Some(message);
        }
    }

    pub fn mark_failed(&mut self, message: String) {
        self.running = false;
        self.pid = None;
        self.last_started_at = None;
        self.last_error = Some(message);
        self.source = ServiceSource::Stopped;
    }

    pub fn mark_external_running(&mut self, pid: Option<u32>) {
        self.running = true;
        self.pid = pid;
        self.last_started_at = None;
        self.last_error = None;
        self.source = ServiceSource::External;
    }
}

#[derive(Default)]
struct ManagedService {
    child: Option<Child>,
    status: ServiceStatus,
}

impl ManagedService {
    fn refresh(&mut self, probe_target: Option<&ProbeTarget>) {
        let Some(child) = self.child.as_mut() else {
            self.status.running = false;
            self.status.pid = None;
            if let Some(target) = probe_target.filter(|target| is_probe_target_running(target)) {
                self.status
                    .mark_external_running(find_pid_by_probe_target(target));
            }
            return;
        };

        match child.try_wait() {
            Ok(Some(exit_status)) => {
                self.child = None;
                self.status
                    .mark_stopped(Some(format!("进程已退出: {}", exit_status)));
            }
            Ok(None) => {
                self.status.running = true;
                self.status.pid = Some(child.id());
            }
            Err(err) => {
                self.child = None;
                self.status.mark_failed(format!("读取进程状态失败: {}", err));
            }
        }
    }

    fn start(&mut self, spec: CommandSpec) -> Result<ServiceStatus, String> {
        self.refresh(Some(&spec.probe_target));
        if self.status.running {
            return Ok(self.status.clone());
        }

        std::fs::create_dir_all(&spec.workdir)
            .map_err(|err| format!("创建工作目录失败: {}", err))?;

        if let Some(db_dir) = spec
            .envs
            .iter()
            .find(|(key, _)| key == "MAIL_GATEWAY_DB")
            .and_then(|(_, value)| PathBuf::from(value).parent().map(PathBuf::from))
        {
            std::fs::create_dir_all(db_dir).map_err(|err| format!("创建 mail-gateway 数据目录失败: {}", err))?;
        }

        let mut command = Command::new(&spec.program);
        command
            .current_dir(&spec.workdir)
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        for (key, value) in &spec.envs {
            command.env(key, value);
        }

        let child = command
            .spawn()
            .map_err(|err| {
                let message = format!("启动进程失败: {}", err);
                self.status.mark_failed(message.clone());
                message
            })?;

        let pid = Some(child.id());
        self.child = Some(child);
        self.status.mark_started(pid);
        Ok(self.status.clone())
    }

    fn stop(&mut self, probe_target: Option<&ProbeTarget>) -> Result<ServiceStatus, String> {
        self.stop_with(probe_target, stop_process_by_pid)
    }

    fn stop_with<F>(
        &mut self,
        probe_target: Option<&ProbeTarget>,
        stop_pid: F,
    ) -> Result<ServiceStatus, String>
    where
        F: Fn(u32) -> Result<(), String>,
    {
        self.refresh(probe_target);
        let Some(mut child) = self.child.take() else {
            if self.status.running {
                if let Some(target) = probe_target {
                    let pid = find_pid_by_probe_target(target)
                        .ok_or_else(|| "Service is running outside desktop app and PID could not be determined".to_string())?;
                    stop_pid(pid)?;
                    self.status.mark_stopped(None);
                    return Ok(self.status.clone());
                }
                return Err("Service is running outside desktop app and cannot be stopped here".to_string());
            }
            self.status.mark_stopped(None);
            return Ok(self.status.clone());
        };

        child
            .kill()
            .map_err(|err| {
                let message = format!("停止进程失败: {}", err);
                self.status.mark_failed(message.clone());
                message
            })?;
        let _ = child.wait();
        self.status.mark_stopped(None);
        Ok(self.status.clone())
    }
}

#[derive(Default)]
pub struct ServiceManager {
    mail_gateway: ManagedService,
    turnstile_solver: ManagedService,
}

impl ServiceManager {
    pub fn get_status_map_with_targets(
        &mut self,
        mail_gateway_target: Option<ProbeTarget>,
        turnstile_solver_target: Option<ProbeTarget>,
    ) -> HashMap<String, ServiceStatus> {
        self.mail_gateway.refresh(mail_gateway_target.as_ref());
        self.turnstile_solver
            .refresh(turnstile_solver_target.as_ref());

        HashMap::from([
            (MAIL_GATEWAY_SERVICE.to_string(), self.mail_gateway.status.clone()),
            (
                TURNSTILE_SOLVER_SERVICE.to_string(),
                self.turnstile_solver.status.clone(),
            ),
        ])
    }

    pub fn start_mail_gateway(&mut self, spec: CommandSpec) -> Result<ServiceStatus, String> {
        self.mail_gateway.start(spec)
    }

    pub fn stop_mail_gateway(
        &mut self,
        probe_target: Option<ProbeTarget>,
    ) -> Result<ServiceStatus, String> {
        self.mail_gateway.stop(probe_target.as_ref())
    }

    pub fn start_turnstile_solver(&mut self, spec: CommandSpec) -> Result<ServiceStatus, String> {
        self.turnstile_solver.start(spec)
    }

    pub fn stop_turnstile_solver(
        &mut self,
        probe_target: Option<ProbeTarget>,
    ) -> Result<ServiceStatus, String> {
        self.turnstile_solver.stop(probe_target.as_ref())
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        let _ = self.mail_gateway.stop(None);
        let _ = self.turnstile_solver.stop(None);
    }
}

fn required_config(config: &HashMap<String, String>, key: &str) -> Result<String, String> {
    config
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("缺少必填配置: {}", key))
}

fn optional_config(config: &HashMap<String, String>, key: &str, default: &str) -> String {
    config
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn required_port(config: &HashMap<String, String>, key: &str) -> Result<u16, String> {
    let value = required_config(config, key)?;
    value
        .parse::<u16>()
        .map_err(|err| format!("{} is not a valid port: {}", key, err))
}

fn is_probe_target_running(target: &ProbeTarget) -> bool {
    let address = format!("{}:{}", target.host, target.port);
    let Ok(addrs) = address.to_socket_addrs() else {
        return false;
    };

    addrs.into_iter().any(|addr| {
        TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
    })
}

fn find_pid_by_probe_target(target: &ProbeTarget) -> Option<u32> {
    #[cfg(windows)]
    {
        let output = Command::new("netstat")
            .args(["-ano", "-p", "tcp"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        parse_windows_netstat_pid(&stdout, target)
    }

    #[cfg(not(windows))]
    {
        let _ = target;
        None
    }
}

fn parse_windows_netstat_pid(output: &str, target: &ProbeTarget) -> Option<u32> {
    output.lines().find_map(|line| {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() < 5 {
            return None;
        }
        if !columns[0].eq_ignore_ascii_case("TCP") {
            return None;
        }
        if !columns[3].eq_ignore_ascii_case("LISTENING") {
            return None;
        }
        if !local_address_matches_probe_target(columns[1], target) {
            return None;
        }
        columns[4].parse::<u32>().ok()
    })
}

fn local_address_matches_probe_target(local_address: &str, target: &ProbeTarget) -> bool {
    let Some((host, port)) = split_socket_host_port(local_address) else {
        return false;
    };
    if port != target.port {
        return false;
    }

    let target_host = target.host.trim().to_ascii_lowercase();
    let local_host = host.trim_matches(|ch| ch == '[' || ch == ']').to_ascii_lowercase();

    local_host == target_host
        || (target_host == "localhost" && (local_host == "127.0.0.1" || local_host == "::1"))
        || ((target_host == "0.0.0.0" || target_host == "::") && (local_host == "0.0.0.0" || local_host == "::"))
}

fn split_socket_host_port(local_address: &str) -> Option<(&str, u16)> {
    let trimmed = local_address.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        let port = rest[end + 1..].strip_prefix(':')?.parse::<u16>().ok()?;
        return Some((host, port));
    }

    let separator = trimmed.rfind(':')?;
    let host = &trimmed[..separator];
    let port = trimmed[separator + 1..].parse::<u16>().ok()?;
    Some((host, port))
}

fn stop_process_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .map_err(|err| format!("Failed to stop process {}: {}", pid, err))?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            output.status.to_string()
        };
        Err(format!("Failed to stop process {}: {}", pid, detail))
    }

    #[cfg(not(windows))]
    {
        let _ = pid;
        Err("Stopping external services by PID is not implemented on this platform".to_string())
    }
}

fn resolve_repo_path(repo_root: &str, relative_or_absolute: &str) -> PathBuf {
    let candidate = PathBuf::from(relative_or_absolute);
    if candidate.is_absolute() {
        return candidate;
    }

    PathBuf::from(repo_root).join(candidate)
}

pub fn repo_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "无法定位仓库根目录".to_string())
}

pub fn build_mail_gateway_spec(
    config: &HashMap<String, String>,
    repo_root: &str,
) -> Result<CommandSpec, String> {
    let conda_env = required_config(config, "conda_env")?;
    let python_program = resolve_conda_env_python(&conda_env)?;
    build_mail_gateway_spec_with_python(config, repo_root, python_program)
}

fn build_mail_gateway_spec_with_python(
    config: &HashMap<String, String>,
    repo_root: &str,
    python_program: String,
) -> Result<CommandSpec, String> {
    let probe_target = build_mail_gateway_probe_target(config)?;
    let host = probe_target.host.clone();
    let port = probe_target.port.to_string();
    let database_path = optional_config(
        config,
        "mail_gateway_database_path",
        "mail-gateway/data/mail_gateway.db",
    );
    let luckmail_base_url = optional_config(config, "luckmail_base_url", "https://mails.luckyous.com");
    let luckmail_api_key = optional_config(config, "luckmail_api_key", "");
    let yyds_base_url = optional_config(config, "yyds_base_url", "https://maliapi.215.im/v1");
    let yyds_api_key = optional_config(config, "yyds_api_key", "");
    let mail_chatgpt_uk_base_url = optional_config(
        config,
        "mail_chatgpt_uk_base_url",
        "https://mail.chatgpt.org.uk",
    );
    let mail_chatgpt_uk_api_key = optional_config(config, "mail_chatgpt_uk_api_key", "");
    let workdir = PathBuf::from(repo_root).join("mail-gateway");
    let db_path = resolve_repo_path(repo_root, &database_path);

    Ok(CommandSpec {
        program: python_program,
        args: vec![
            "-m".to_string(),
            "mail_gateway.run_server".to_string(),
            "--host".to_string(),
            host,
            "--port".to_string(),
            port,
        ],
        envs: vec![
            ("MAIL_GATEWAY_DB".to_string(), db_path.to_string_lossy().into_owned()),
            ("LUCKMAIL_BASE_URL".to_string(), luckmail_base_url),
            ("LUCKMAIL_API_KEY".to_string(), luckmail_api_key),
            ("YYDS_BASE_URL".to_string(), yyds_base_url),
            ("YYDS_API_KEY".to_string(), yyds_api_key),
            ("MAIL_CHATGPT_UK_BASE_URL".to_string(), mail_chatgpt_uk_base_url),
            ("MAIL_CHATGPT_UK_API_KEY".to_string(), mail_chatgpt_uk_api_key),
        ],
        workdir,
        probe_target,
    })
}

fn resolve_conda_env_python(conda_env: &str) -> Result<String, String> {
    let prefix = resolve_conda_env_prefix(conda_env)?;
    let python = conda_env_python_path(&prefix);
    if !python.exists() {
        return Err(format!(
            "Conda environment python not found: {}",
            python.to_string_lossy()
        ));
    }
    Ok(python.to_string_lossy().into_owned())
}

fn resolve_conda_env_prefix(conda_env: &str) -> Result<PathBuf, String> {
    let direct = PathBuf::from(conda_env);
    if direct.is_absolute() {
        if is_python_executable_path(&direct) {
            return direct
                .parent()
                .map(PathBuf::from)
                .ok_or_else(|| format!("Invalid python path: {}", direct.to_string_lossy()));
        }
        return Ok(direct);
    }

    let output = Command::new("conda")
        .args(["env", "list", "--json"])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|err| format!("Failed to inspect conda environments: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Failed to inspect conda environments: {}",
            if stderr.is_empty() { output.status.to_string() } else { stderr }
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|err| format!("Failed to decode conda environment list: {}", err))?;
    find_conda_env_prefix_in_json(&stdout, conda_env)
}

fn find_conda_env_prefix_in_json(payload: &str, conda_env: &str) -> Result<PathBuf, String> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|err| format!("Failed to parse conda environment list: {}", err))?;
    let envs = value
        .get("envs")
        .and_then(Value::as_array)
        .ok_or_else(|| "Conda environment list is missing envs".to_string())?;

    for entry in envs {
        let Some(prefix) = entry.as_str() else {
            continue;
        };
        let prefix_path = PathBuf::from(prefix);
        let env_name = prefix_path.file_name().and_then(OsStr::to_str);
        if prefix.eq_ignore_ascii_case(conda_env) || env_name == Some(conda_env) {
            return Ok(prefix_path);
        }
    }

    Err(format!("Conda environment not found: {}", conda_env))
}

fn conda_env_python_path(prefix: &Path) -> PathBuf {
    if cfg!(windows) {
        prefix.join("python.exe")
    } else {
        prefix.join("bin").join("python")
    }
}

fn is_python_executable_path(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|name| name.eq_ignore_ascii_case("python.exe") || name == "python")
        .unwrap_or(false)
}

pub fn build_mail_gateway_probe_target(
    config: &HashMap<String, String>,
) -> Result<ProbeTarget, String> {
    Ok(ProbeTarget {
        host: optional_config(config, "mail_gateway_host", "127.0.0.1"),
        port: required_port(config, "mail_gateway_port")?,
    })
}

pub fn build_turnstile_solver_spec(
    config: &HashMap<String, String>,
    repo_root: &str,
) -> Result<CommandSpec, String> {
    let conda_env = required_config(config, "conda_env")?;
    let probe_target = build_turnstile_solver_probe_target(config)?;
    let host = probe_target.host.clone();
    let port = probe_target.port.to_string();
    let thread = optional_config(config, "turnstile_thread", "2");
    let browser_type = optional_config(config, "turnstile_browser_type", "chromium");
    let workdir = PathBuf::from(repo_root).join("TurnstileSolver");
    let mut args = vec![
        "run".to_string(),
        "-n".to_string(),
        conda_env,
        "python".to_string(),
        "api_solver.py".to_string(),
        "--host".to_string(),
        host,
        "--port".to_string(),
        port,
        "--thread".to_string(),
        thread,
        "--browser_type".to_string(),
        browser_type,
    ];

    if optional_config(config, "turnstile_headless", "true") == "false" {
        args.push("--no-headless".to_string());
    }
    if optional_config(config, "turnstile_debug", "false") == "true" {
        args.push("--debug".to_string());
    }
    if optional_config(config, "turnstile_proxy", "false") == "true" {
        args.push("--proxy".to_string());
    }
    if optional_config(config, "turnstile_random", "false") == "true" {
        args.push("--random".to_string());
    }

    Ok(CommandSpec {
        program: "conda".to_string(),
        args,
        envs: Vec::new(),
        workdir,
        probe_target,
    })
}

pub fn build_turnstile_solver_probe_target(
    config: &HashMap<String, String>,
) -> Result<ProbeTarget, String> {
    Ok(ProbeTarget {
        host: optional_config(config, "turnstile_host", "127.0.0.1"),
        port: required_port(config, "turnstile_port")?,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};

    use super::{
        build_mail_gateway_spec_with_python, build_turnstile_solver_spec, conda_env_python_path,
        find_conda_env_prefix_in_json, parse_windows_netstat_pid, CommandSpec, ManagedService,
        ProbeTarget, ServiceManager, ServiceSource, ServiceStatus, MAIL_GATEWAY_SERVICE,
    };

    fn config(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn mail_gateway_spec_uses_saved_config_values() {
        let cfg = config(&[
            ("conda_env", "orchids-register"),
            ("mail_gateway_host", "127.0.0.1"),
            ("mail_gateway_port", "8081"),
            ("mail_gateway_database_path", "mail-gateway/data/mail_gateway.db"),
            ("luckmail_base_url", "https://mails.luckyous.com"),
            ("luckmail_api_key", "key-1"),
            ("yyds_base_url", "https://maliapi.215.im/v1"),
            ("yyds_api_key", "key-2"),
            ("mail_chatgpt_uk_base_url", "https://mail.chatgpt.org.uk"),
            ("mail_chatgpt_uk_api_key", "key-3"),
        ]);

        let spec = build_mail_gateway_spec_with_python(
            &cfg,
            r"D:\repo",
            r"D:\miniconda3\envs\orchids-register\python.exe".to_string(),
        )
        .expect("spec");
        assert_eq!(spec.program, r"D:\miniconda3\envs\orchids-register\python.exe");
        assert!(spec.args.windows(2).any(|items| items == ["-m", "mail_gateway.run_server"]));
        assert!(spec
            .envs
            .iter()
            .any(|(key, value)| key == "LUCKMAIL_API_KEY" && value == "key-1"));
        assert!(spec
            .envs
            .iter()
            .any(|(key, value)| key == "MAIL_CHATGPT_UK_BASE_URL" && value == "https://mail.chatgpt.org.uk"));
        assert!(spec
            .envs
            .iter()
            .any(|(key, value)| key == "MAIL_CHATGPT_UK_API_KEY" && value == "key-3"));
        assert_eq!(spec.workdir.to_string_lossy(), r"D:\repo\mail-gateway");
    }

    #[test]
    fn find_conda_env_prefix_matches_by_env_name() {
        let payload = r#"{
            "envs": [
                "D:\\miniconda3",
                "D:\\miniconda3\\envs\\base",
                "D:\\miniconda3\\envs\\orchids-register"
            ]
        }"#;

        let prefix = find_conda_env_prefix_in_json(payload, "orchids-register").expect("prefix");

        assert_eq!(prefix, PathBuf::from(r"D:\miniconda3\envs\orchids-register"));
    }

    #[test]
    fn conda_env_python_path_uses_windows_python_location() {
        let python = conda_env_python_path(Path::new(r"D:\miniconda3\envs\orchids-register"));

        assert_eq!(python, PathBuf::from(r"D:\miniconda3\envs\orchids-register\python.exe"));
    }

    #[test]
    fn turnstile_spec_requires_conda_env_and_port() {
        let cfg = config(&[]);
        let err = build_turnstile_solver_spec(&cfg, r"D:\repo").expect_err("missing config");
        assert!(err.contains("conda_env"));
    }

    #[test]
    fn service_status_defaults_to_stopped() {
        let status = ServiceStatus::default();
        assert!(!status.running);
        assert!(status.last_error.is_none());
        assert!(status.pid.is_none());
        assert_eq!(status.source, ServiceSource::Stopped);
    }

    #[test]
    fn update_running_service_records_pid() {
        let mut status = ServiceStatus::default();
        status.mark_started(Some(1234));
        assert!(status.running);
        assert_eq!(status.pid, Some(1234));
        assert!(status.last_started_at.is_some());
        assert_eq!(status.source, ServiceSource::DesktopManaged);
    }

    #[test]
    fn get_status_map_marks_external_service_as_running() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let mut manager = ServiceManager::default();

        let statuses = manager.get_status_map_with_targets(
            Some(ProbeTarget {
                host: "127.0.0.1".to_string(),
                port,
            }),
            None,
        );

        assert!(statuses[MAIL_GATEWAY_SERVICE].running);
        assert_eq!(statuses[MAIL_GATEWAY_SERVICE].pid, Some(std::process::id()));
        assert_eq!(statuses[MAIL_GATEWAY_SERVICE].source, ServiceSource::External);
    }

    #[test]
    fn start_returns_running_when_external_service_already_exists() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let mut manager = ServiceManager::default();

        let status = manager
            .start_mail_gateway(CommandSpec {
                program: "missing-program".to_string(),
                args: Vec::new(),
                envs: Vec::new(),
                workdir: PathBuf::from("."),
                probe_target: ProbeTarget {
                    host: "127.0.0.1".to_string(),
                    port,
                },
            })
            .expect("running status");

        assert!(status.running);
        assert_eq!(status.pid, Some(std::process::id()));
        assert_eq!(status.source, ServiceSource::External);
    }

    #[test]
    fn parses_windows_netstat_output_for_pid() {
        let output = "\
  TCP    127.0.0.1:5000         0.0.0.0:0              LISTENING       12345\r\n\
  TCP    127.0.0.1:8081         0.0.0.0:0              LISTENING       45678\r\n";

        assert_eq!(
            parse_windows_netstat_pid(output, &ProbeTarget {
                host: "127.0.0.1".to_string(),
                port: 8081,
            }),
            Some(45678)
        );
    }

    #[test]
    fn stopping_external_service_by_resolved_pid_succeeds() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let mut service = ManagedService::default();

        let status = service
            .stop_with(
                Some(&ProbeTarget {
                    host: "127.0.0.1".to_string(),
                    port,
                }),
                |_pid| Ok(()),
            )
            .expect("external service should be stoppable via resolved pid");

        assert!(!status.running);
        assert!(status.pid.is_none());
        assert_eq!(status.source, ServiceSource::Stopped);
    }
}
