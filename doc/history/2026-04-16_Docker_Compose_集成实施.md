# Docker Compose 集成实施计划

## 项目概述

将 PHP-Stack 从孤立容器管理升级为统一编排管理，借鉴 DNMP 项目实践，实现所有容器共享 `php-stack-network`，通过服务名互相访问，并动态生成 `docker-compose.yml` 进行集中配置管理。

**数据存放位置**：项目根目录 `data/` 文件夹统一管理所有持久化数据

---

## Phase 1: 基础网络支持（预计 1-2 天）

### Task 1.1: 创建统一网络管理器

**文件**: `src-tauri/src/engine/network_manager.rs`（新建）

实现网络创建和管理功能：

```rust
use bollard::Docker;
use bollard::models::{NetworkCreateRequest, NetworkConnectRequest, EndpointSettings};

pub struct NetworkManager {
    docker: Docker,
    network_name: String,
}

impl NetworkManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            docker: Docker::connect_with_local_defaults()?,
            network_name: "php-stack-network".to_string(),
        })
    }

    // 确保网络存在，不存在则创建
    pub async fn ensure_network_exists(&self) -> Result<(), Box<dyn std::error::Error>>;
    
    // 将容器连接到网络，并设置服务别名
    pub async fn connect_container(&self, container_name: &str, alias: &str) 
        -> Result<(), Box<dyn std::error::Error>>;
    
    // 从容器名提取服务别名（ps-php-8-2 -> php）
    fn extract_service_alias(&self, container_name: &str) -> String;
}
```

**关键点**：
- 使用 `bollard::models::NetworkCreateRequest` 创建桥接网络
- 连接时设置 `EndpointSettings` 的 `aliases` 字段，实现短名称服务发现
- 网络名称固定为 `php-stack-network`

---

### Task 1.2: 集成到 SoftwareManager

**文件**: `src-tauri/src/engine/software_manager.rs`（修改）

在 `SoftwareManager` 结构体中添加 `network_manager` 字段：

```rust
pub struct SoftwareManager {
    docker: Docker,
    network_manager: NetworkManager,  // 新增
}
```

修改 `install_software` 方法，在容器启动后调用网络连接：

```rust
pub async fn install_software(&self, spec: SoftwareSpec) -> Result<String, Error> {
    // ... 现有逻辑：创建并启动容器 ...
    
    // 新增：将容器加入统一网络
    let alias = self.network_manager.extract_service_alias(&container_name);
    self.network_manager.connect_container(&container_name, &alias).await?;
    
    log::info!("✅ 容器 {} 已加入网络（别名: {}）", container_name, alias);
    
    Ok(container_name)
}
```

**测试点**：
- 安装 PHP 8.2 后，执行 `docker network inspect php-stack-network` 验证容器已加入
- 进入 PHP 容器，`ping mysql` 应该能解析（即使 MySQL 还未安装，DNS 查询应正常）

---

### Task 1.3: 迁移已有容器（可选但推荐）

**文件**: `src-tauri/src/commands.rs`（新增命令）

添加迁移命令，将现有孤立容器迁移到统一网络：

```rust
#[tauri::command]
pub async fn migrate_containers_to_network() -> Result<String, String> {
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let containers = manager.list_installed_software().await
        .map_err(|e| format!("获取容器列表失败: {}", e))?;
    
    let mut migrated = 0;
    for container in containers {
        let alias = /* 提取别名 */;
        manager.network_manager.connect_container(&container.name, &alias).await
            .map_err(|e| format!("迁移 {} 失败: {}", container.name, e))?;
        migrated += 1;
    }
    
    Ok(format!("成功迁移 {} 个容器到统一网络", migrated))
}
```

在 `lib.rs` 中注册此命令。

---

## Phase 2: Compose 管理器（预计 3-4 天）

### Task 2.1: 创建 ComposeManager 核心模块

**文件**: `src-tauri/src/engine/compose_manager.rs`（新建）

实现 YAML 生成和文件管理：

```rust
use serde::{Serialize, Deserialize};
use std::fs;
use std::collections::HashMap;

// Docker Compose 配置文件结构
#[derive(Debug, Serialize, Deserialize)]
pub struct DockerCompose {
    pub version: String,
    pub networks: HashMap<String, NetworkConfig>,
    pub services: HashMap<String, ServiceConfig>,
    pub volumes: Option<HashMap<String, VolumeConfig>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub driver: String,
    pub external: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub image: String,
    pub container_name: String,
    pub networks: Vec<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub depends_on: Option<Vec<String>>,
    pub restart: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VolumeConfig {
    pub driver: String,
}

pub struct ComposeManager {
    compose_path: String,
    data_dir: String,
}

impl ComposeManager {
    pub fn new(project_root: &str) -> Self {
        Self {
            compose_path: format!("{}/docker-compose.yml", project_root),
            data_dir: format!("{}/data", project_root),
        }
    }

    // 根据已安装容器重建 docker-compose.yml
    pub async fn rebuild_from_containers(
        &self,
        containers: &[InstalledSoftware]
    ) -> Result<(), Box<dyn std::error::Error>>;
    
    // 构建单个服务的配置
    fn build_service_config(
        &self,
        container: &InstalledSoftware
    ) -> Result<ServiceConfig, Box<dyn std::error::Error>>;
    
    // 确定服务依赖关系（PHP 依赖 MySQL/Redis，Nginx 依赖 PHP）
    fn determine_dependencies(
        &self,
        software_type: &SoftwareType,
        all_containers: &[InstalledSoftware]
    ) -> Option<Vec<String>>;
    
    // 从容器名提取服务名（ps-php-8-2 -> php）
    fn extract_service_name(&self, container_name: &str) -> String;
    
    // 执行 docker compose up -d 应用变更
    pub async fn apply_changes(&self) -> Result<(), Box<dyn std::error::Error>>;
}
```

**关键实现细节**：

1. **端口映射格式**：`"宿主机端口:容器端口"` 例如 `"80:80"`
2. **数据卷路径**：统一使用 `./data/{service}` 映射到容器内默认路径
   - MySQL: `./data/mysql:/var/lib/mysql`
   - Redis: `./data/redis:/data`
   - MongoDB: `./data/mongodb:/data/db`
3. **依赖关系**：
   - PHP 依赖已安装的 MySQL/Redis
   - Nginx 依赖已安装的 PHP
4. **YAML 序列化**：使用 `serde_yaml::to_string_pretty()` 生成格式化输出

---

### Task 2.2: 集成到 SoftwareManager

**文件**: `src-tauri/src/engine/software_manager.rs`（修改）

在 `SoftwareManager` 中添加 `compose_manager` 字段：

```rust
pub struct SoftwareManager {
    docker: Docker,
    network_manager: NetworkManager,
    compose_manager: ComposeManager,  // 新增
}
```

修改初始化和关键方法：

```rust
impl SoftwareManager {
    pub fn new() -> Result<Self, Error> {
        let docker = Docker::connect_with_local_defaults()?;
        let network_manager = NetworkManager::new()?;
        let compose_manager = ComposeManager::new(".");  // 项目根目录
        
        Ok(Self { 
            docker, 
            network_manager,
            compose_manager,
        })
    }

    pub async fn install_software(&self, spec: SoftwareSpec) -> Result<String, Error> {
        // ... 创建容器、启动、连接网络 ...
        
        // 新增：更新 docker-compose.yml
        self.update_compose_file().await?;
        
        Ok(container_name)
    }

    pub async fn uninstall_software(&self, name: &str) -> Result<(), Error> {
        // ... 停止并删除容器 ...
        
        // 新增：更新 docker-compose.yml
        self.update_compose_file().await?;
        
        Ok(())
    }

    // 新增辅助方法
    async fn update_compose_file(&self) -> Result<(), Error> {
        let containers = self.list_installed_software().await?;
        self.compose_manager.rebuild_from_containers(&containers).await
            .map_err(|e| format!("更新 compose 文件失败: {}", e))?;
        
        // 应用变更（不会重启未变化的容器）
        self.compose_manager.apply_changes().await
            .map_err(|e| format!("应用 compose 变更失败: {}", e))?;
        
        log::info!("✅ docker-compose.yml 已更新并应用");
        Ok(())
    }
}
```

---

### Task 2.3: 添加 Cargo 依赖

**文件**: `src-tauri/Cargo.toml`（修改）

确认已添加 `serde_yaml = "0.9"`（之前已添加）

---

### Task 2.4: 创建 data 目录结构

**文件**: 项目根目录（手动创建或在代码中自动创建）

在 `ComposeManager::new()` 中添加自动创建目录逻辑：

```rust
use std::fs;
use std::path::Path;

pub fn new(project_root: &str) -> Self {
    let data_dir = format!("{}/data", project_root);
    
    // 自动创建 data 目录及子目录
    let _ = fs::create_dir_all(&data_dir);
    let _ = fs::create_dir_all(format!("{}/mysql", data_dir));
    let _ = fs::create_dir_all(format!("{}/redis", data_dir));
    let _ = fs::create_dir_all(format!("{}/mongodb", data_dir));
    
    Self {
        compose_path: format!("{}/docker-compose.yml", project_root),
        data_dir,
    }
}
```

---

### Task 2.5: 测试动态增删服务

**测试场景**：

1. 安装 MySQL 8.0 → 检查 `docker-compose.yml` 包含 mysql 服务
2. 安装 PHP 8.2 → 检查 `docker-compose.yml` 包含 php 服务，且 `depends_on` 包含 mysql
3. 安装 Redis 7.0 → 检查 `docker-compose.yml` 包含 redis 服务
4. 卸载 Redis → 检查 `docker-compose.yml` 不再包含 redis 服务
5. 执行 `docker compose ps` 验证所有服务运行正常

---

## Phase 3: 智能重启优化（预计 1-2 天）

### Task 3.1: 实现依赖关系分析

**文件**: `src-tauri/src/engine/compose_manager.rs`（扩展）

添加智能重启方法：

```rust
impl ComposeManager {
    // 智能重启：只重启受影响的容器及其依赖者
    pub async fn smart_restart(
        &self,
        target_service: &str,
        all_containers: &[InstalledSoftware]
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut affected_services = vec![target_service.to_string()];
        
        // 找出依赖目标服务的其他服务
        for container in all_containers {
            let service_name = self.extract_service_name(&container.name);
            if let Some(deps) = &container.spec.depends_on {
                if deps.contains(&target_service.to_string()) {
                    affected_services.push(service_name);
                }
            }
        }
        
        // 去重
        affected_services.sort();
        affected_services.dedup();
        
        // 逐个重启
        for service in &affected_services {
            log::info!("🔄 重启服务: {}", service);
            self.restart_service(service).await?;
        }
        
        Ok(affected_services)
    }
    
    // 重启单个服务
    async fn restart_service(&self, service: &str) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::process::Command;
        
        let output = Command::new("docker")
            .args(&["compose", "-f", &self.compose_path, "restart", service])
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(format!(
                "重启 {} 失败: {}",
                service,
                String::from_utf8_lossy(&output.stderr)
            ).into());
        }
        
        Ok(())
    }
}
```

---

### Task 3.2: 暴露重启命令给前端

**文件**: `src-tauri/src/commands.rs`（新增命令）

```rust
#[tauri::command]
pub async fn restart_service(name: String) -> Result<String, String> {
    let manager = SoftwareManager::new().map_err(|e| e.to_string())?;
    let containers = manager.list_installed_software().await
        .map_err(|e| format!("获取容器列表失败: {}", e))?;
    
    let affected = manager.compose_manager.smart_restart(&name, &containers).await
        .map_err(|e| format!("智能重启失败: {}", e))?;
    
    Ok(format!("已重启 {} 个服务: {:?}", affected.len(), affected))
}
```

在 `lib.rs` 中注册此命令。

---

### Task 3.3: 前端添加重启确认对话框

**文件**: `src/components/SoftwareCenter.vue`（修改）

在已安装的服务卡片中添加重启按钮：

```vue
<div v-if="isInstalled(version.version)" class="mt-4">
  <div class="p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg mb-3">
    <div class="text-sm text-blue-400 font-medium">✓ 已安装</div>
    <div class="text-xs text-slate-500 mt-1">
      状态: {{ getInstalledInstance(version.version)?.status || 'Unknown' }}
    </div>
  </div>
  
  <div class="flex gap-2">
    <button
      @click="handleRestart(getInstalledInstance(version.version)!.name)"
      class="flex-1 py-2 bg-amber-600/20 hover:bg-amber-600 text-amber-400 hover:text-white border border-amber-600/30 rounded-lg text-sm font-medium transition-all"
    >
      重启
    </button>
    <button
      @click="handleUninstall(getInstalledInstance(version.version)!.name)"
      class="flex-1 py-2 bg-rose-600/20 hover:bg-rose-600 text-rose-400 hover:text-white border border-rose-600/30 rounded-lg text-sm font-medium transition-all"
    >
      卸载
    </button>
  </div>
</div>
```

添加重启处理函数：

```typescript
const handleRestart = async (name: string) => {
  if (!confirm(`重启 ${name} 可能会影响依赖它的服务，是否继续？`)) return;
  
  try {
    const result = await invoke('restart_service', { name }) as string;
    alert(result);
    await loadInstalled();
  } catch (e) {
    alert(`重启失败: ${e}`);
  }
};
```

---

## Phase 4: 前端增强（预计 2-3 天）

### Task 4.1: 显示服务依赖关系

**文件**: `src/components/SoftwareCenter.vue`（扩展）

在版本卡片中添加网络连接信息展示：

```vue
<!-- 在已安装状态下方添加 -->
<div v-if="isInstalled(version.version)" class="mt-3 p-3 bg-slate-800/50 border border-slate-700 rounded-lg">
  <div class="text-xs text-slate-400 font-medium mb-2">🔗 网络连接</div>
  <div class="text-xs text-slate-500 space-y-1">
    <div v-if="selectedType === 'php' && hasService('mysql')">
      ✓ 可访问 MySQL: <code class="text-blue-300 bg-slate-900 px-1 rounded">mysql:3306</code>
    </div>
    <div v-if="selectedType === 'php' && hasService('redis')">
      ✓ 可访问 Redis: <code class="text-blue-300 bg-slate-900 px-1 rounded">redis:6379</code>
    </div>
    <div v-if="selectedType === 'nginx' && hasService('php')">
      ✓ 可访问 PHP: <code class="text-blue-300 bg-slate-900 px-1 rounded">php:9000</code>
    </div>
    <div v-if="selectedType === 'mysql'">
      ✓ 外部连接: <code class="text-blue-300 bg-slate-900 px-1 rounded">localhost:{{ getPort(version.version) }}</code>
    </div>
  </div>
</div>
```

添加辅助函数：

```typescript
const hasService = (serviceName: string): boolean => {
  return installedList.value.some(item => {
    const name = item.name.replace('ps-', '').split('-')[0];
    return name === serviceName;
  });
};

const getPort = (version: string): number | undefined => {
  const instance = getInstalledInstance(version);
  if (!instance) return undefined;
  // 从 port_mappings 中提取第一个端口
  const ports = Object.values(instance.spec.port_mappings);
  return ports[0];
};
```

---

### Task 4.2: 添加 docker-compose.yml 查看器

**文件**: `src/components/SoftwareCenter.vue`（扩展）

在页面顶部添加查看按钮：

```vue
<header class="mb-8">
  <div class="flex justify-between items-start">
    <div>
      <h1 class="text-3xl font-bold mb-2">软件管理中心</h1>
      <p class="text-slate-400">一键安装和管理开发环境软件</p>
    </div>
    <button
      @click="showComposeFile"
      class="px-4 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg text-sm font-medium transition flex items-center gap-2"
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
        <polyline points="14 2 14 8 20 8"></polyline>
      </svg>
      查看 docker-compose.yml
    </button>
  </div>
</header>
```

添加对话框组件：

```vue
<!-- 在模板底部添加 -->
<div
  v-if="showComposeModal"
  class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50"
  @click.self="showComposeModal = false"
>
  <div class="bg-slate-900 border border-slate-800 rounded-2xl p-6 max-w-3xl w-full mx-4 shadow-2xl">
    <div class="flex justify-between items-center mb-4">
      <h3 class="text-xl font-bold">docker-compose.yml</h3>
      <button @click="showComposeModal = false" class="text-slate-400 hover:text-white">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    </div>
    
    <div class="bg-black/40 p-4 rounded-lg font-mono text-xs text-green-400 max-h-96 overflow-y-auto whitespace-pre-wrap">
      {{ composeContent || '加载中...' }}
    </div>
    
    <div class="mt-4 flex gap-3">
      <button
        @click="copyComposeContent"
        class="flex-1 py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition"
      >
        复制内容
      </button>
      <button
        @click="showComposeModal = false"
        class="flex-1 py-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg font-medium transition"
      >
        关闭
      </button>
    </div>
  </div>
</div>
```

添加状态和处理函数：

```typescript
const showComposeModal = ref(false);
const composeContent = ref('');

const showComposeFile = async () => {
  showComposeModal.value = true;
  try {
    // 读取 docker-compose.yml 文件内容
    const { readTextFile } = await import('@tauri-apps/plugin-fs');
    composeContent.value = await readTextFile('docker-compose.yml');
  } catch (e) {
    composeContent.value = `读取失败: ${e}`;
  }
};

const copyComposeContent = async () => {
  try {
    await navigator.clipboard.writeText(composeContent.value);
    alert('已复制到剪贴板');
  } catch (e) {
    alert('复制失败');
  }
};
```

**注意**：需要添加 `@tauri-apps/plugin-fs` 依赖并在 `capabilities/default.json` 中授权文件读取权限。

---

### Task 4.3: 添加 Tauri 文件系统插件权限

**文件**: `src-tauri/capabilities/default.json`（修改）

在 `permissions` 数组中添加：

```json
{
  "identifier": "default",
  "description": "Default permissions",
  "local": true,
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:allow-save",
    "fs:allow-read-text-file"  // 新增
  ]
}
```

**文件**: `src-tauri/Cargo.toml`（确认依赖）

确保已添加：
```toml
tauri-plugin-fs = "2"
```

---

### Task 4.4: （可选）网络拓扑可视化

如果时间允许，可以使用简单的 Mermaid 图表或自定义 SVG 展示服务间的依赖关系。

**文件**: `src/components/NetworkTopology.vue`（可选新建）

使用 Vue + D3.js 或简单 CSS Grid 展示服务连接图。

---

## 测试与验证

### 端到端测试流程

1. **清空环境**：删除所有 `ps-*` 容器和 `docker-compose.yml`
2. **安装 MySQL 8.0**：
   - 验证容器创建成功
   - 验证 `docker-compose.yml` 生成
   - 验证 `data/mysql/` 目录创建
3. **安装 PHP 8.2**：
   - 验证容器加入网络
   - 验证 `docker-compose.yml` 中 PHP 的 `depends_on` 包含 mysql
   - 进入 PHP 容器测试 `ping mysql`
4. **安装 Redis 7.0**：
   - 验证三个服务都在 `docker-compose.yml` 中
   - 进入 PHP 容器测试 `ping redis`
5. **安装 Nginx 1.24**：
   - 验证 Nginx 的 `depends_on` 包含 php
   - 测试 Nginx 能否通过 `php:9000` 访问 PHP-FPM
6. **重启 MySQL**：
   - 点击重启按钮
   - 验证只有 MySQL 和依赖它的 PHP、Nginx 被重启
7. **卸载 Redis**：
   - 验证 `docker-compose.yml` 不再包含 redis
   - 验证 Redis 容器被删除
8. **查看 docker-compose.yml**：
   - 点击"查看"按钮
   - 验证内容正确显示
   - 测试复制功能

---

## 风险与注意事项

### 1. Bollard API 兼容性

网络相关 API 可能需要调整，参考 `bollard` 文档确认正确的类型和方法签名。

**应对方案**：先编写最小可运行示例，验证 API 可用性后再集成。

### 2. 端口冲突

`docker compose up -d` 可能因端口冲突失败。

**应对方案**：在 `apply_changes()` 前再次检测端口可用性，捕获错误并提供友好提示。

### 3. 向后兼容

用户可能已有运行的 `ps-*` 容器，直接集成可能导致冲突。

**应对方案**：
- 提供"迁移工具"按钮，一键将现有容器加入网络
- 首次运行时检测已有容器，提示用户是否迁移

### 4. 文件权限

`docker-compose.yml` 和 `data/` 目录的权限问题（特别是在 Linux/Mac）。

**应对方案**：使用适当的文件权限创建（0644 for files, 0755 for dirs）。

---

## 交付物清单

### 代码文件

- [ ] `src-tauri/src/engine/network_manager.rs`（新建）
- [ ] `src-tauri/src/engine/compose_manager.rs`（新建）
- [ ] `src-tauri/src/engine/software_manager.rs`（修改）
- [ ] `src-tauri/src/commands.rs`（新增迁移和重启命令）
- [ ] `src-tauri/src/lib.rs`（注册新命令）
- [ ] `src-tauri/Cargo.toml`（确认 serde_yaml 和 tauri-plugin-fs 依赖）
- [ ] `src-tauri/capabilities/default.json`（添加 fs 权限）
- [ ] `src/components/SoftwareCenter.vue`（添加依赖显示、重启按钮、Compose 查看器）

### 文档文件

- [ ] 更新 `docs/v1.1-docker-compose-integration-plan.md` 标记完成状态
- [ ] 在 `README.md` 中添加 Docker Compose 使用说明

### 自动生成文件

- [ ] `docker-compose.yml`（运行时生成）
- [ ] `data/mysql/`、`data/redis/`、`data/mongodb/`（运行时创建）

---

## 时间估算

| 阶段 | 任务数 | 预计天数 | 累计天数 |
|------|--------|----------|----------|
| Phase 1 | 3 | 1-2 | 2 |
| Phase 2 | 5 | 3-4 | 6 |
| Phase 3 | 3 | 1-2 | 8 |
| Phase 4 | 4 | 2-3 | 11 |
| **总计** | **15** | **7-11** | **11** |

建议分 2-3 次 Git Commit：
1. Phase 1-2 完成后：`feat: add Docker Compose integration core`
2. Phase 3 完成后：`feat: add smart restart optimization`
3. Phase 4 完成后：`feat: enhance UI with network info and compose viewer`

---

## 成功标准

1. ✅ 所有安装的容器自动加入 `php-stack-network`
2. ✅ PHP 可通过 `mysql:3306` 和 `redis:6379` 访问其他服务
3. ✅ `docker-compose.yml` 实时反映当前安装的服务
4. ✅ 数据持久化到 `data/` 目录
5. ✅ 智能重启只影响相关服务
6. ✅ 前端清晰展示服务依赖关系
7. ✅ 可查看和复制 `docker-compose.yml` 内容
8. ✅ 所有测试场景通过
