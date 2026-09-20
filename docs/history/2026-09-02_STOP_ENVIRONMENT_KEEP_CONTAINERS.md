# 一键停止后保留容器、展示「已停用」状态

**日期**：2026-09-02
**类型**：Bug 修复 / 交互优化
**影响范围**：环境管理页（dashboard）、后端 `stop_environment` 命令、中文文案

---

## 问题

在环境管理页点击「一键停止环境」后，容器列表变成空状态，显示：

> 未发现 ps- 前缀的容器
> 请先在「环境配置」中配置 PHP、Nginx 等环境

用户期望：停止后仍能看到之前启动过的容器，状态标记为「已停用」。

## 根因

`stop_environment` 命令内部使用 `docker compose down`：

- `down` = 停止 **并删除** 容器；
- 容器被删除后，`docker ps -a`（`list_ps_containers` 用 `all: true`）也查不到；
- 前端 `containers.length === 0` → 落入空状态。

前端展示能力其实早已就绪（后端 `list_containers` 已用 `all: true`；`App.vue` 容器卡片按 `isRunning()` 为 false 显示 stopped 状态 + 单个「启动」按钮），只是容器被删后无从展示。

## 修复

### 1. `src-tauri/src/commands/env_config.rs`

`stop_environment` 由 `docker compose down` 改为 `docker compose stop`：

```rust
// 使用 docker compose stop 停止容器（保留容器，前端可显示"已停用"状态）
stop_cmd.args(["compose", "stop"])
```

- `stop` 只停止、保留容器（EXITED 状态）。
- 停止后前端可查到 EXITED 容器，卡片显示「已停用」+「启动」按钮。
- 单个「启动」按钮走 bollard `start_container`，对显式 `container_name` 的 ps- 容器可正常拉起。
- `start_environment` 已有 `docker compose down --remove-orphans` 清理旧容器，下次一键启动依然干净，不受影响。

### 2. `src/i18n/locales/zh-CN.json`

状态标签中文化，消除中英混杂：

| key | 之前 | 之后 |
|---|---|---|
| `dashboard.container.running` | `Running` | `运行中` |
| `dashboard.container.stopped` | `Stopped` | `已停用` |

## 验证

- `cargo check` 通过（1m17s）。
- `cargo build --release` 通过（3m29s + 1m30s 两次，第二次为强制重编译内嵌新前端）。
- 未新增单测：`stop_environment` 依赖真实 Docker，单测不可覆盖。

## 关联提交

- `571adc1` fix(env): 一键停止改用 compose stop，保留容器并展示已停用状态
- `de7c443` i18n(zh-CN): 容器状态 running 文案中文化为「运行中」

## 注意事项

- 停止后容器仍占用容器名与镜像磁盘（不占 CPU/内存）；彻底清理依赖下次一键启动的 `down` 或手动 `docker rm`。
- 改前端资源后裸跑 `cargo build --release` 不会自动重新内嵌 dist（`tauri_build::build()` 未对 `../dist` 设有效 `rerun-if-changed`），需先 `touch src-tauri/build.rs` 强制重编译。
