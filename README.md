# Orchids Register

基于 `Rust + Tauri + React + Python` 的 Orchids 注册工具。

当前推荐使用方式已经切换为：

- 桌面端 `Tauri UI` 是主配置入口
- `Mail Gateway` 和 `TurnstileSolver` 由桌面端直接管理
- `runtime.local.yaml` 不再是桌面流程的必填配置
- `runtime.*` 和少量 CLI 兼容脚本仍保留，用于非桌面端场景

## 当前主流程

如果你是正常使用桌面版，优先走这条路径：

1. 安装 Python、Rust、Node.js、Conda 环境
2. 安装 `mail-gateway` 和 `TurnstileSolver` 依赖
3. 启动 Tauri 桌面端
4. 在桌面端页面里填写：
   - `Mail Gateway` 运行参数和 API Key
   - `TurnstileSolver` 运行参数
   - 注册流程所需的邮箱与验证码相关配置
5. 直接在桌面端启动 / 停止服务并执行注册

## 兼容路径

下面这些内容仍然保留，但已经不是桌面主流程：

- `config/runtime.example.yaml`
- `config/runtime.local.yaml`
- `scripts/run-cli-registration.ps1`
- `scripts/build-desktop.ps1`

它们适合以下场景：

- 你仍然使用 CLI 跑注册流程
- 你需要脱离桌面端单独调试兼容脚本
- 你在排查旧环境或兼容历史用法

## 项目结构

```text
orchids_register/
  config/           兼容旧脚本的 runtime YAML 模板
  docs/             运行、发布和历史设计文档
  mail-gateway/     邮箱网关服务
  scripts/          兼容 CLI 的辅助脚本
  src/              Rust core / CLI
  src-tauri/        Tauri 桌面后端
  TurnstileSolver/  验证码求解服务
  ui/               React 前端
```

## 环境准备

建议在 `Windows + PowerShell` 下使用。

```powershell
conda activate orchids-register
cd orchids_register
python -m pip install -r .\mail-gateway\requirements.txt
python -m pip install -r .\TurnstileSolver\requirements.txt
cd .\ui
npm install
```

## 启动桌面开发

```powershell
cd orchids_register
cargo tauri dev
```

启动后，优先在桌面端完成配置，不必先去编辑 `runtime.local.yaml`。

## 文档入口

- 当前运行说明：[docs/run-build-guide.md](docs/run-build-guide.md)
- GitHub 提交说明：[docs/github-publish-guide.md](docs/github-publish-guide.md)
- 历史设计 / 实现记录：[docs/superpowers/](docs/superpowers/)

## 历史文档说明

`docs/superpowers/specs` 和 `docs/superpowers/plans` 记录的是当时的设计与实施过程。

这些文档可以帮助理解项目如何演进，但它们不是当前的最终操作手册。
