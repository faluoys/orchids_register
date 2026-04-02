# GitHub 提交指南

更新时间：2026-04-02

本文档用于把当前目录 `orchids_register` 初始化为一个新的 Git 仓库，并推送到 GitHub。

## 1. 提交前先确认什么不会被上传

当前本地敏感配置文件：

- `config/runtime.local.yaml`

它已经在 [`.gitignore`](../.gitignore) 里，正常情况下不会被提交。

公开仓库里会保留这些文件：

- `config/runtime.example.yaml`
- `docs/run-build-guide.md`
- `scripts/*.ps1`
- `scripts/*.bat`

## 2. 首次初始化 Git 仓库

在项目根目录执行：

```powershell
cd orchids_register
git init
git branch -M main
```

## 3. 先验证忽略规则是否生效

执行：

```powershell
git status --short --ignored
```

你应该重点确认：

- `config/runtime.local.yaml` 显示为 `!! config/runtime.local.yaml`
- `config/runtime.example.yaml` 会显示为未跟踪文件，准备提交

如果 `runtime.local.yaml` 没被忽略，先停下，不要提交。

也可以单独检查：

```powershell
git check-ignore -v config/runtime.local.yaml
```

## 4. 暂存并检查提交内容

先执行：

```powershell
git add .
git status --short
```

再次确认输出里**不要出现**：

```powershell
config/runtime.local.yaml
```

如果它真的出现在已暂存列表里，立刻执行：

```powershell
git rm --cached config/runtime.local.yaml
```

然后再看一次：

```powershell
git status --short --ignored
```

## 5. 提交

```powershell
git commit -m "feat: add mail gateway runtime config and startup scripts"
```

## 6. 在 GitHub 创建空仓库

在 GitHub 网页上新建一个空仓库。

要求：

- 不要勾选 `Add a README`
- 不要勾选 `.gitignore`
- 不要勾选 `license`

否则首次 push 会多一次处理远程历史的步骤。

## 7. 绑定远程并推送

把下面的 URL 改成你自己的 GitHub 仓库地址：

```powershell
git remote add origin https://github.com/<你的用户名>/<你的仓库名>.git
git push -u origin main
```

## 8. 如果远程仓库已经有内容

如果你在 GitHub 上已经提前加了 README 或别的文件，就用这一组：

```powershell
git remote add origin https://github.com/<你的用户名>/<你的仓库名>.git
git fetch origin
git pull --rebase origin main --allow-unrelated-histories
git push -u origin main
```

如果 `origin` 已经存在，先改远程地址：

```powershell
git remote set-url origin https://github.com/<你的用户名>/<你的仓库名>.git
```

## 9. 最短可执行命令清单

如果你现在就要直接推，按这个顺序执行：

```powershell
cd orchids_register
git init
git branch -M main
git status --short --ignored
git add .
git status --short
git commit -m "feat: add mail gateway runtime config and startup scripts"
git remote add origin https://github.com/<你的用户名>/<你的仓库名>.git
git push -u origin main
```

## 10. 推送前最后检查

在真正 `git push` 前，至少再看一次这两条：

```powershell
git status --short
git status --short --ignored
```

你需要确保：

- 没有把 `config/runtime.local.yaml` 加进去
- 没有把测试结果文件误提交
- 没有把你本机自己的额外敏感文件误提交

## 11. 推荐做法

推荐你长期保持这个习惯：

- 把真实配置只写在 `config/runtime.local.yaml`
- 把共享模板写在 `config/runtime.example.yaml`
- 提交前先看一次 `git status --short --ignored`
- 永远不要用 `git add -f config/runtime.local.yaml`

