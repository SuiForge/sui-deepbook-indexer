# Clean Release Guide - 隐藏 Commit 历史

本指南说明如何在开源前移除仓库的 commit 历史记录，发布一个干净的版本。

## 方案 1: Orphan 分支（推荐）

保留现有 git 仓库，但创建一个无历史的分支用于发布。

### 步骤

```bash
# 创建一个孤立分支（不继承任何历史）
git checkout --orphan clean-main

# 暂存所有当前代码
git add -A

# 提交为初始 commit
git commit -m "Initial commit: DeepBook Indexer v1.0.0"

# （可选）推送到远程
git branch -D main
git branch -m main
git push -f origin main
```

**优点**：
- 保留现有仓库结构
- 可以只发布特定分支
- 不影响现有开发分支

**缺点**：
- 若强制推送可能影响其他开发者

---

## 方案 2: 删除 .git 并重新初始化

彻底移除所有历史，重新开始。

### 步骤

```bash
# 在干净的目录中
rm -rf .git

# 重新初始化
git init

# 配置（可选，用于初始 commit）
git config user.email "you@example.com"
git config user.name "Your Name"

# 添加所有文件
git add -A

# 初始提交
git commit -m "Initial commit: DeepBook Indexer v1.0.0"

# 添加远程（GitHub 示例）
git remote add origin https://github.com/yourorg/deepbook-indexer.git

# 推送
git branch -M main
git push -u origin main
```

**优点**：
- 完全干净，零历史
- 简洁直接

**缺点**：
- 本地现有 git 信息全部丢失
- 需要重新配置远程

---

## 方案 3: 单文件压缩包发布

不提供 git 仓库，直接发布代码压缩包。

```bash
# 排除 .git 和敏感文件
tar --exclude='.git' \
    --exclude='node_modules' \
    --exclude='.env.local' \
    --exclude='target' \
    -czf deepbook-indexer-v1.0.0.tar.gz .

# 或使用 zip（Windows）
```

**优点**：
- 完全没有 git 历史
- 适合一次性发布

**缺点**：
- 用户无法通过 git 跟踪更新
- 不利于协作开发

---

## 建议

对于开源项目：
1. **推荐使用方案 1（Orphan 分支）**：在 GitHub 创建一个 `public` 分支作为发布分支，内部继续用 `main` 开发
2. 或在发布前执行方案 2，清空历史后发布到新仓库
3. 如果要保持两个仓库，公开仓库用方案 2（无历史），内部仓库保留完整历史

---

## 发布清单

发布前检查：

- [ ] 移除所有 `.env.local`、密钥、敏感信息
- [ ] 更新 `README.md` 和中文版本
- [ ] 更新版本号（`Cargo.toml`, `go.mod` 等）
- [ ] 运行 `cargo test` 和 `go test` 确保无错
- [ ] 添加 `LICENSE` 文件
- [ ] 更新 `CHANGELOG.md`
- [ ] 执行选定的清理方案（orphan 分支或重新初始化）
- [ ] 上传至 GitHub / GitLab

---

## 示例：使用 Orphan 分支发布流程

```bash
# 假设当前在 main 分支
git status  # 确保工作目录干净

# 创建发布分支
git checkout --orphan public-release

# 保留代码，移除历史
git add -A
git commit -m "v1.0.0: DeepBook Indexer - Initial Public Release"

# 推送到 GitHub
git push origin public-release:main  # 推送到 GitHub main 分支

# 或创建新仓库
git remote set-url origin https://github.com/yourorg/deepbook-indexer.git
git push -u origin public-release:main
```

完成！GitHub 上的仓库现在只有一个初始 commit，无历史记录。
