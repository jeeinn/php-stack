# PHP-Stack 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 构建一个轻量级、跨平台的 PHP 开发环境可视化管理工具，支持多版本切换、镜像源优化及深度导入导出。

**架构：** 采用 Tauri 架构，前端使用 Vue 3 + Tailwind CSS 提供 UI，后端使用 Rust 通过 `bollard` 库与 Docker Engine 通信，配置文件基于 Handlebars 模板动态生成。

**技术栈：** Tauri, Rust (bollard, serde, zip), Vue 3, Tailwind CSS, Vite.

---

## 第一阶段：项目基础与环境准备

### 任务 1：初始化 Tauri 项目结构

**文件：**
- 创建：`src-tauri/Cargo.toml`
- 创建：`package.json`
- 创建：`src/main.ts`

- [ ] **步骤 1：使用 Vite 创建前端项目并安装 Tauri CLI**
```bash
npm create vite@latest . -- --template vue-ts
npm install @tauri-apps/api @tauri-apps/cli
```

- [ ] **步骤 2：初始化 Tauri 后端**
```bash
npx tauri init
```

- [ ] **步骤 3：配置 Cargo.toml 添加依赖**
修改 `src-tauri/Cargo.toml`，添加 `bollard`, `serde`, `serde_json`, `zip`, `handlebars`。

- [ ] **步骤 4：Commit**
```bash
git add .
git commit -m "chore: initialize tauri project with dependencies"
```

---

## 第二阶段：Docker 核心逻辑实现 (Rust)

### 任务 2：实现容器管理基础类

**文件：**
- 创建：`src-tauri/src/docker/manager.rs`
- 修改：`src-tauri/src/main.rs`

- [ ] **步骤 1：编写 Docker 客户端初始化代码**
使用 `bollard::Docker::connect_with_local_defaults()`。

- [ ] **步骤 2：实现容器列表获取功能 (过滤 ps- 前缀)**
```rust
pub async fn list_ps_containers(docker: &Docker) -> Result<Vec<ContainerSummary>, Error> {
    let mut filters = HashMap::new();
    filters.insert("name", vec!["ps-"]);
    // ... 调用 docker.list_containers
}
```

- [ ] **步骤 3：编写容器启动/停止/重启指令**

- [ ] **步骤 4：Commit**
```bash
git add src-tauri/src/docker/manager.rs
git commit -m "feat: add docker container management logic"
```

### 任务 3：镜像源切换与 PHP 扩展安装逻辑

**文件：**
- 创建：`src-tauri/src/docker/mirror.rs`

- [ ] **步骤 1：实现修改 Docker daemon.json 的逻辑 (区分 OS)**
- [ ] **步骤 2：编写 PHP Dockerfile 模板，支持替换 apt/apk 镜像源**
- [ ] **步骤 3：实现 Composer 镜像源一键设置命令**

---

## 第三阶段：前端 UI 开发 (Vue 3)

### 任务 4：构建可视化仪表盘

**文件：**
- 创建：`src/components/Dashboard.vue`
- 创建：`src/components/ServiceCard.vue`

- [ ] **步骤 1：实现 ServiceCard 组件，展示 ps- 前缀容器状态**
- [ ] **步骤 2：通过 Tauri Command 调用 Rust 后端获取容器实时状态**
- [ ] **步骤 3：添加“启动/停止”按钮的交互逻辑**

---

## 第四阶段：导入导出引擎

### 任务 5：实现深度导出功能

**文件：**
- 创建：`src-tauri/src/engine/export.rs`

- [ ] **步骤 1：实现配置文件扫描逻辑 (nginx/vhosts)**
- [ ] **步骤 2：实现项目文件通配符匹配逻辑**
使用 `glob` 库处理用户输入的 `*` 表达式。
- [ ] **步骤 3：实现 MySQL 导出逻辑**
执行 `docker exec ps-mysql mysqldump ...` 并捕获输出流。
- [ ] **步骤 4：打包为 ZIP 文件**

---

## 第五阶段：集成测试与验证

### 任务 6：端到端验证

- [ ] **步骤 1：测试一键启动所有服务**
- [ ] **步骤 2：测试修改镜像源后，PHP 容器内安装扩展的速度**
- [ ] **步骤 3：执行一次完整的导出 -> 删除环境 -> 导入恢复流程**
