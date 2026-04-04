# GitHub 提交指南

更新时间：2026-04-04

## 1. 这份文档现在解决什么问题

这份文档只负责一件事：

- 帮你把当前仓库安全地提交到 GitHub

它不是运行说明，也不是桌面配置说明。

## 2. 提交前先确认哪些文件不该进仓库

仍然需要重点保护的本地敏感配置：

- `config/runtime.local.yaml`

虽然桌面端主流程现在不依赖这个文件，但它仍然可能包含：

- 本地 API Key
- 旧脚本兼容配置
- 调试环境里的敏感参数

所以它依然不应该上传到 GitHub。

公开仓库通常会保留这些文件：

- `config/runtime.example.yaml`
- `docs/run-build-guide.md`
- `scripts/*.ps1`
- `scripts/*.bat`

## 3. 提交前检查忽略规则

```powershell
git status --short --ignored
```

你要重点确认：

- `config/runtime.local.yaml` 显示为被忽略
- 它没有出现在 staged 列表里

也可以单独检查：

```powershell
git check-ignore -v config/runtime.local.yaml
```

## 4. 如果误加了 runtime.local.yaml

立刻把它从索引里移掉：

```powershell
git rm --cached config/runtime.local.yaml
```

然后重新检查：

```powershell
git status --short --ignored
```

## 5. 正常提交流程

```powershell
git add .
git status --short
git commit -m "feat: update desktop workflow and config guidance"
```

## 6. 首次推送到 GitHub

```powershell
git init
git branch -M main
git remote add origin https://github.com/<your-user>/<your-repo>.git
git push -u origin main
```

如果远程仓库已经有内容：

```powershell
git remote add origin https://github.com/<your-user>/<your-repo>.git
git fetch origin
git pull --rebase origin main --allow-unrelated-histories
git push -u origin main
```

## 7. 当前仓库文档层级

为了避免再把历史计划当成主说明，建议这样理解文档：

- `README.md`：当前项目总入口
- `docs/run-build-guide.md`：当前运行说明
- `docs/github-publish-guide.md`：当前提交说明
- `docs/superpowers/`：历史设计与实施记录

## 8. 不要做的事

- 不要把 `config/runtime.local.yaml` 强制提交
- 不要把历史计划文档当成当前用户手册
- 不要在提交前只看 UI，不检查 `git status`
