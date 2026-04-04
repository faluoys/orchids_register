# Orchids Register 运行与构建指北

更新时间：2026-04-05

## 1. 先记住当前推荐流程

桌面端现在是主配置入口。

如果你使用 Tauri 桌面版，推荐顺序是：

1. 安装依赖
2. 启动桌面端
3. 在桌面配置页面里填写运行参数和 API Key
4. 在桌面里启动 `Mail Gateway` 和 `TurnstileSolver`
5. 直接从桌面执行注册和测试

`runtime.local.yaml` 已经不是桌面主流程的前置步骤。

## 2. 什么情况下还需要 runtime YAML

这些文件和兼容脚本仍然保留：

- `config/runtime.example.yaml`
- `config/runtime.local.yaml`
- `scripts/run-cli-registration.ps1`
- `scripts/run-cli-registration.bat`
- `scripts/build-desktop.ps1`
- `scripts/build-desktop.bat`

它们只在这些场景仍然有用：

- 你要跑 CLI
- 你要单独调试兼容脚本
- 你要复现历史流程

如果你只使用桌面端，可以先不管这些文件。

## 3. 安装依赖

```powershell
conda activate orchids-register
cd orchids_register
python -m pip install -r .\mail-gateway\requirements.txt
python -m pip install -r .\TurnstileSolver\requirements.txt
cd .\ui
npm install
```

## 4. 启动桌面开发

```powershell
cd orchids_register
cargo tauri dev
```

启动后主要在两个页面完成配置：

- `Mail Gateway` 页面
- `系统设置 / TurnstileSolver` 页面

## 5. 桌面端首次配置建议

### 5.1 Mail Gateway

至少确认这些值：

- `Host`
- `Port`
- `Database Path`
- 对应供应商的 `API Key`
- `mail_provider`
- `mail_provider_mode`

常见情况：

- 用 `LuckMail` 时，重点检查 `luckmail_api_key`
- 用 `YYDS` 时，重点检查 `yyds_api_key`

### 5.2 TurnstileSolver

至少确认这些值：

- `conda_env`
- `Host`
- `Port`
- `Thread`
- `browser_type`

如果启动失败，先看：

- Conda 环境名是否正确
- 端口是否被别的程序占用
- `TurnstileSolver` 依赖是否已经安装

## 6. 常见操作

### 6.1 启动桌面后端服务

在桌面端中直接启动：

- `Mail Gateway`
- `TurnstileSolver`

现在推荐这样做，不再建议优先跑旧脚本。

### 6.2 只做健康检查

- `Mail Gateway` 页面可以直接点健康检查
- `TurnstileSolver` 页面可以直接查看服务状态和启动日志反馈

### 6.3 构建前端

```powershell
cd orchids_register\ui
npm run build
```

### 6.4 运行 Rust 后端测试

```powershell
cd orchids_register
cargo test -p orchids-auto-register-portable --lib -- --nocapture
```

## 7. 兼容脚本说明

如果你确实要走非桌面端的兼容路径，现在只保留这些：

- `scripts/run-cli-registration.ps1`
- `scripts/build-desktop.ps1`

但要注意：

- 这些脚本属于兼容入口
- 它们仍可能读取 `runtime.local.yaml`
- 不应再把它们当成桌面版默认使用方式
- `Mail Gateway` 和 `TurnstileSolver` 的启动与停止应优先在桌面端完成

## 8. 历史文档

`docs/superpowers/` 目录保存的是设计稿和实施计划，主要用于追溯历史决策，不是当前操作手册。
