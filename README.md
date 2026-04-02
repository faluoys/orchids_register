# Orchids Register

一个基于 `Rust + Tauri + React + Python` 的 Orchids 自动注册工具。

当前主路径已经切到统一的 `mail-gateway` 架构：

- 桌面端只关心统一的邮箱网关协议
- 验证码求解继续由独立的 `TurnstileSolver` 服务负责
- 当前已接通 `LuckMail purchased inbox` 流程
- 本地运行参数统一收敛到 `YAML` 配置文件，不再要求你每次手敲环境变量

## 功能概览

- 桌面端 UI：Tauri + React
- CLI 注册入口：`cargo run --bin orchids-auto-register`
- 邮箱网关服务：`mail-gateway`
- 验证码求解服务：`TurnstileSolver`
- 一键脚本：支持开发启动、CLI 验证、桌面打包

## 当前邮箱接入策略

当前仓库的活动路径是：

- `mail_mode = gateway`
- `mail_provider = luckmail`
- `mail_provider_mode = purchased`

也就是说，桌面端和 CLI 现在都优先通过本地 `mail-gateway` 服务去申请邮箱、轮询验证码和释放会话，而不是直接在客户端里对接供应商协议。

这样做的好处是：

- 后续切换邮箱供应商时，客户端不需要再大改
- 敏感凭据不必直接写进桌面代码
- 可以继续往 `YYDS Mail / DuckMail / 其他供应商` 扩展

## 技术栈

- Rust 2021
- Tauri 2
- React 19 + Vite + TypeScript
- FastAPI + httpx + SQLite
- PowerShell / BAT 启动脚本

## 目录结构

```text
orchids_register/
  config/           YAML 运行配置
  docs/             运行、发布、设计文档
  mail-gateway/     邮箱网关服务
  scripts/          一键启动 / 打包 / CLI 验证脚本
  src/              Rust core / CLI
  src-tauri/        Tauri 桌面壳
  TurnstileSolver/  验证码求解服务
  ui/               React 前端
```

## 快速开始

### 1. 安装依赖

默认按 Windows + PowerShell 使用。

```powershell
conda activate orchids-register
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
python -m pip install -r .\mail-gateway\requirements.txt
python -m pip install -r .\TurnstileSolver\requirements.txt
cd .\ui
npm install
```

### 2. 修改本地配置

编辑：

- [`config/runtime.local.yaml`](config/runtime.local.yaml)

至少先改这个字段：

```yaml
mail_gateway:
  luckmail_api_key: REPLACE_WITH_REAL_LUCKMAIL_KEY
```

注意：

- `config/runtime.local.yaml` 已加入 `.gitignore`
- 提交到 GitHub 时不会上传你的真实本地配置
- 仓库里保留的是模板文件 [`config/runtime.example.yaml`](config/runtime.example.yaml)

### 3. 一键启动开发全套

PowerShell：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\start-dev-stack.ps1
```

BAT：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\start-dev-stack.bat
```

这个脚本会分别启动：

- `mail-gateway`
- `TurnstileSolver`
- `cargo tauri dev`

### 4. 一键打包桌面应用

PowerShell：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\build-desktop.ps1
```

BAT：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\build-desktop.bat
```

桌面包产物通常在：

```text
target/release/bundle
```

### 5. 一键跑一次 CLI 注册验证

PowerShell：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\run-cli-registration.ps1
```

BAT：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\run-cli-registration.bat
```

默认结果文件：

```text
register_result.json
```

## 主要脚本

- [`scripts/start-dev-stack.ps1`](scripts/start-dev-stack.ps1)：一键启动 `mail-gateway + TurnstileSolver + cargo tauri dev`
- [`scripts/build-desktop.ps1`](scripts/build-desktop.ps1)：一键打包桌面端
- [`scripts/run-cli-registration.ps1`](scripts/run-cli-registration.ps1)：一键跑一次 CLI 注册验证
- [`scripts/start-mail-gateway.ps1`](scripts/start-mail-gateway.ps1)：单独启动邮箱网关
- [`scripts/start-turnstile-solver.ps1`](scripts/start-turnstile-solver.ps1)：单独启动验证码服务
- [`scripts/start-desktop-dev.ps1`](scripts/start-desktop-dev.ps1)：单独启动桌面开发模式

所有 `ps1` 脚本都支持：

```powershell
-DryRun
```

例如：

```powershell
.\scripts\start-dev-stack.ps1 -DryRun
```

这样只打印最终命令，不真正启动进程。

## 文档索引

- [运行与打包指南](docs/run-build-guide.md)
- [GitHub 提交指南](docs/github-publish-guide.md)
- [mail-gateway 设计文档](docs/superpowers/specs/2026-04-02-mail-gateway-design.md)
- [LuckMail Gateway Phase 1 计划](docs/superpowers/plans/2026-04-02-luckmail-gateway-phase1.md)

## 当前已知前提

要真正跑完整注册，你还需要：

- 可用的 `LuckMail API Key`
- 本地启动 `TurnstileSolver`
- 本地启动 `mail-gateway`
- `orchids-register` Conda 环境

## 安全建议

不要把这些内容提交到仓库：

- 真实 `LuckMail API Key`
- `config/runtime.local.yaml`
- 本地环境目录 `.conda/`
- `ui/node_modules/`
- `target/`
- 临时测试产物和运行结果

## 状态

当前仓库已经包含：

- `mail-gateway` Phase 1 主链路
- Rust core 的 gateway/manual 邮箱流程
- Tauri / React 配置页迁移
- YAML 运行配置
- PowerShell / BAT 一键脚本
- GitHub 提交流程文档
