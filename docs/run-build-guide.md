# Orchids Register 运行与打包指南

更新时间：2026-04-02

本文档已经切换为“优先改 YAML 配置文件，再执行脚本”的方式。

## 1. 新增文件

本次新增了这些文件：

- `config/runtime.example.yaml`：配置模板
- `config/runtime.local.yaml`：本地实际配置，优先读取，已加入 `.gitignore`
- `scripts/common.ps1`：脚本公共函数
- `scripts/init-runtime-config.ps1`：显式生成或覆盖 `runtime.local.yaml`
- `scripts/start-mail-gateway.ps1`：启动邮箱网关
- `scripts/start-turnstile-solver.ps1`：启动验证码求解服务
- `scripts/start-desktop-dev.ps1`：启动桌面开发模式
- `scripts/start-dev-stack.ps1`：一键启动开发全套
- `scripts/build-desktop.ps1`：一键打包桌面应用
- `scripts/run-cli-registration.ps1`：按 YAML 配置执行一次 CLI 注册验证
- `scripts/init-runtime-config.bat`：PowerShell 初始化脚本包装
- `scripts/start-dev-stack.bat`：PowerShell 启动器包装
- `scripts/build-desktop.bat`：PowerShell 打包器包装
- `scripts/run-cli-registration.bat`：PowerShell CLI 验证包装

## 2. 配置文件说明

主配置文件：

- [runtime.local.yaml](/D:/workspace/github/Orchids_register_TurnstileSolver/orchids_register/config/runtime.local.yaml)

模板文件：

- [runtime.example.yaml](/D:/workspace/github/Orchids_register_TurnstileSolver/orchids_register/config/runtime.example.yaml)

脚本读取顺序：

1. `config/runtime.local.yaml`
2. `config/runtime.example.yaml`

也就是说，你平时只需要改 `runtime.local.yaml`。

### 2.1 自动生成规则

现在开始，你不需要手动复制模板。

当下面任意脚本执行时：

- `init-runtime-config.ps1`
- `start-mail-gateway.ps1`
- `start-turnstile-solver.ps1`
- `start-desktop-dev.ps1`
- `start-dev-stack.ps1`
- `build-desktop.ps1`
- `run-cli-registration.ps1`

如果 `config/runtime.local.yaml` 不存在，脚本会自动从 `config/runtime.example.yaml` 生成一份本地配置。

如果你想显式初始化：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\init-runtime-config.ps1
```

如果你想强制覆盖已有本地配置：

```powershell
.\scripts\init-runtime-config.ps1 -Force
```

### 2.2 你必须先改的字段

先打开 [runtime.local.yaml](/D:/workspace/github/Orchids_register_TurnstileSolver/orchids_register/config/runtime.local.yaml)，至少确认这些值：

```yaml
conda_env: orchids-register

mail_gateway:
  host: 127.0.0.1
  port: 8081
  database_path: mail-gateway/data/mail_gateway.db
  luckmail_base_url: https://mails.luckyous.com
  luckmail_api_key: REPLACE_WITH_REAL_LUCKMAIL_KEY

turnstile_solver:
  host: 127.0.0.1
  port: 5000
  thread: 2
  browser_type: chromium
  headless: true
  debug: false
  proxy: false
  random: false

orchids:
  mail_mode: gateway
  mail_gateway_base_url: http://127.0.0.1:8081
  mail_provider: luckmail
  mail_provider_mode: purchased
  mail_project_code: orchids
  poll_timeout: 180
  poll_interval: 2
  captcha_api_url: http://127.0.0.1:5000
  result_json: register_result.json
```

最重要的是：

- 把 `luckmail_api_key` 改成你的真实 Key
- 如果你想改端口，也要同步改 `orchids.mail_gateway_base_url`

### 2.3 YAML 使用限制

当前脚本里的 YAML 解析器是我为这个项目写的简化版，所以请按现有格式改，不要超出这个范围。

允许：

- 顶层键值
- 一级 section
- section 里再写简单键值
- 布尔值 `true/false`
- 数字
- 字符串

不要这样写：

- 行内注释，例如 `port: 8081 # 注释`
- 数组
- 多级嵌套
- 复杂 YAML 语法

## 3. 一次性准备

第一次使用这台机器时，先执行：

```powershell
conda activate orchids-register
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
python -m pip install -r .\mail-gateway\requirements.txt
python -m pip install -r .\TurnstileSolver\requirements.txt
cd .\ui
npm install
```

## 4. 最短使用方式

### 4.1 一键初始化本地配置

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\init-runtime-config.ps1
```

### 4.2 一键启动开发全套

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

这个脚本会分别打开 3 个窗口：

- `mail-gateway`
- `TurnstileSolver`
- `cargo tauri dev`

### 4.3 一键打包桌面应用

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

### 4.4 一键执行一次 CLI 注册验证

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

结果文件默认输出到：

```powershell
D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\register_result.json
```

## 5. 单独启动某个部分

### 5.1 单独初始化配置

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\init-runtime-config.ps1
```

### 5.2 单独启动 mail-gateway

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\start-mail-gateway.ps1
```

### 5.3 单独启动 TurnstileSolver

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\start-turnstile-solver.ps1
```

### 5.4 单独启动桌面开发模式

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\start-desktop-dev.ps1
```

## 6. 不直接启动，只看脚本会执行什么

每个 PowerShell 脚本都支持 `-DryRun`。

例如：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
.\scripts\init-runtime-config.ps1 -DryRun
.\scripts\start-mail-gateway.ps1 -DryRun
.\scripts\start-turnstile-solver.ps1 -DryRun
.\scripts\start-desktop-dev.ps1 -DryRun
.\scripts\start-dev-stack.ps1 -DryRun
.\scripts\build-desktop.ps1 -DryRun
.\scripts\run-cli-registration.ps1 -DryRun
```

这会打印最终动作，但不会真正启动进程。

## 7. 手动命令兜底

如果你不想用脚本，也可以按下面的手动命令执行。

### 7.1 启动 mail-gateway

```powershell
conda activate orchids-register
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\mail-gateway
python -m uvicorn mail_gateway.app:app --host 127.0.0.1 --port 8081
```

### 7.2 启动 TurnstileSolver

```powershell
conda activate orchids-register
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\TurnstileSolver
python api_solver.py --host 127.0.0.1 --port 5000 --thread 2 --browser_type chromium
```

### 7.3 桌面开发模式

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\src-tauri
cargo tauri dev
```

### 7.4 正式打包

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\src-tauri
cargo tauri build
```

## 8. 产物位置

桌面打包产物通常在：

```powershell
D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\target\release\bundle
```

CLI 验证结果默认在：

```powershell
D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\register_result.json
```

mail-gateway 数据库默认在：

```powershell
D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\mail-gateway\data\mail_gateway.db
```

## 9. 排查顺序

如果你点击脚本后失败，按这个顺序查。

### 9.1 先看配置文件

确认 [runtime.local.yaml](/D:/workspace/github/Orchids_register_TurnstileSolver/orchids_register/config/runtime.local.yaml) 里至少这几个值没写错：

- `conda_env`
- `mail_gateway.host`
- `mail_gateway.port`
- `mail_gateway.luckmail_api_key`
- `turnstile_solver.port`
- `orchids.mail_gateway_base_url`

### 9.2 再跑 DryRun

```powershell
.\scripts\start-dev-stack.ps1 -DryRun
```

### 9.3 再单独跑各组件

```powershell
.\scripts\start-mail-gateway.ps1
```

```powershell
.\scripts\start-turnstile-solver.ps1
```

```powershell
.\scripts\start-desktop-dev.ps1
```

### 9.4 最后再打包

```powershell
.\scripts\build-desktop.ps1
```

## 10. 常见问题

### 10.1 `runtime.local.yaml` 不存在

现在脚本会自动生成它。

你也可以手动执行：

```powershell
.\scripts\init-runtime-config.ps1
```

### 10.2 `luckmail` 不是 `enabled`

大概率是 [runtime.local.yaml](/D:/workspace/github/Orchids_register_TurnstileSolver/orchids_register/config/runtime.local.yaml) 里的 `luckmail_api_key` 还没换成真实值。

### 10.3 一键脚本打开了窗口，但服务没起来

先看新窗口里打印的实际命令，再单独执行对应 `.ps1`。

### 10.4 `cargo tauri build` 失败

先拆开执行：

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register\ui
npm run build
```

```powershell
cd D:\workspace\github\Orchids_register_TurnstileSolver\orchids_register
cargo check -p orchids-auto-register-portable
```

### 10.5 我不想碰 PowerShell，只想双击

直接双击：

- `scripts\init-runtime-config.bat`
- `scripts\start-dev-stack.bat`
- `scripts\build-desktop.bat`
- `scripts\run-cli-registration.bat`
