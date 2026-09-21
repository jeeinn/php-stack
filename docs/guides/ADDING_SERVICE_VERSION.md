# 如何添加新的服务版本

## 概述

PHP-Stack 的服务版本清单（`src-tauri/services/version_manifest.json`）与模板目录（`src-tauri/services/<名称><版本>/`）都是**纯文本、可直接编辑**的。添加新版本不需要改代码、不需要重新编译。

这是有意的设计：**即使本项目停止更新，你依然可以通过修改这两个地方，让软件支持上游新发布的版本**，继续用本软件完成启动、备份等核心操作。

> 如果你按本文操作后，界面上的下拉框仍是空的或看不到新版本，页面顶部会出现一条黄色提示条，告诉你清单文件加载失败并给出重试按钮——请先按提示排查，而不是怀疑配置没生效。

---

## 一、命名约定（必须先理解）

模板目录名统一为 **`服务名 + 版本号（去掉小数点）`**：

| 服务 | 版本 | 目录名 | 清单 ID |
|---|---|---|---|
| PHP | 8.5 | `php85` | `php85` |
| MySQL | 8.4 | `mysql84` | `mysql84` |
| Redis | 7.2 | `redis72` | `redis72` |
| Nginx | 1.28 | `nginx128` | `nginx128` |

这个目录名不是随便起的，它被程序按固定规则推导使用：

| 推导目标 | 规则 | 示例 |
|---|---|---|
| 环境变量前缀 | `service_dir.to_uppercase()` | `php85` → `PHP85_CONF_FILE` |
| Compose 服务名 / 容器名 | `ps-<service_dir>` | `ps-nginx128` |
| 目录映射 | 三处同名 | `services/`、`data/`、`logs/` 下都是 `nginx128` |

因此**目录名必须与清单里的 `service_dir` 完全一致**，三者（`services/`、`data/`、`logs/`）会保持同名，程序才能按规则找到配置并复制。

---

## 二、三步流程

### 步骤 1：在清单里添加条目

编辑 `src-tauri/services/version_manifest.json`，在对应服务类型下加一条，**键名即 ID**：

```jsonc
{
  "nginx": {
    "nginx129": {
      "display_name": "Nginx 1.29",
      "image_tag": "nginx:1.29-alpine",
      "service_dir": "nginx129",
      "default_port": 80,
      "show_port": true,
      "eol": false,
      "description": "Nginx 1.29"
    }
  }
}
```

字段说明：

| 字段 | 必填 | 说明 |
|---|---|---|
| `display_name` | 是 | 下拉框显示名 |
| `image_tag` | 是 | 完整镜像标签，需真实存在于镜像仓库 |
| `service_dir` | 是 | 模板目录名。**可与 ID 不同**，见步骤 2 的复用技巧 |
| `default_port` | 是 | 默认端口 |
| `show_port` | 是 | 是否在 UI 显示端口输入框（PHP-FPM 为 `false`，其余为 `true`） |
| `eol` | 是 | 是否已停止支持，仅影响显示（追加 ` (EOL)` 标记） |
| `description` | 否 | 补充说明 |

### 步骤 2：准备模板目录

有三种方式，按需选择：

#### 方式 A：复用已有目录（推荐，零新增文件）

如果新版本与已有版本的配置结构一致（大多数情况如此），直接把 `service_dir` 指向已有目录：

```jsonc
"nginx129": {
  "display_name": "Nginx 1.29",
  "image_tag": "nginx:1.29-alpine",
  "service_dir": "nginx128"   // ← 复用 1.28 的模板目录
}
```

**这是最快的方式**：不需要新建任何文件，新版本直接继承一套已验证可用的配置。

#### 方式 B：不建目录，依赖自动回退

如果你只加了清单条目但没建目录，程序会自动回退到该服务类型的默认模板目录：

| 服务类型 | 回退目录 |
|---|---|
| PHP | `php85` |
| MySQL | `mysql80` |
| Redis | `redis72` |
| Nginx | `nginx127` |

回退时日志会记录 `模板目录 services/xxx 不存在，使用 yyy 作为模板源`。功能可用，但**建议显式指定 `service_dir`**，避免回退目录被删除后失效。

#### 方式 C：新建独立目录（需要定制配置时）

在 `src-tauri/services/` 下新建目录，放入该服务类型需要的模板文件：

| 服务类型 | 必须包含（用于识别目录有效） | 通常还需要 |
|---|---|---|
| PHP | `Dockerfile` | `php.ini`、`php-fpm.conf` |
| MySQL | `mysql.cnf` | — |
| Redis | `redis.conf` | — |
| Nginx | `Dockerfile` | `nginx.conf`、`conf.d/` |

最简单可靠的做法：**复制一个同类型的现有目录再改**。例如添加 MySQL 9.0：

```bash
cp -r services/mysql84 services/mysql90
# 然后按需编辑 services/mysql90/mysql.cnf
```

> **重要**：模板文件只会在「应用配置」时**首次释放**到你的工作区。若目标文件已存在，程序会**跳过复制以保留你的修改**。想恢复默认模板，删除工作区里的对应文件后重新应用配置即可。

### 步骤 3：验证

1. **重新编译并重启应用**——这一步与模板文件不同，务必注意：

   | 内容 | 存放位置 | 读取方式 | 改动后 |
   |---|---|---|---|
   | 版本清单 | `src-tauri/services/version_manifest.json` | 编译时 `include_str!` 嵌入 | **必须重新构建**才生效 |
   | 模板文件 | 工作区 `services/<dir>/`（运行时释放） | 运行时读取 | 重启应用即生效 |

   也就是说，改了清单里的版本号后，仅重启应用**不会**生效，需要重新构建程序。
   而改工作区 `services/` 下的 `php.ini`、`redis.conf` 这类模板文件则不需要重新构建。

   > 如果你希望「不重新构建也能加版本」，需要让程序支持运行时读取外部清单，
   > 这属于代码改动，见 `docs/architecture/SERVICE_TEMPLATE_SOURCING.md`。
2. 进入「环境配置」页，确认新版本出现在下拉框中。
3. 选中并「应用配置」，检查工作区下是否正确生成了 `services/<service_dir>/` 与对应的 `data/`、`logs/` 目录。
4. 启动环境，确认容器 `ps-<service_dir>` 正常运行。

---

## 三、完整示例：添加 Nginx 1.29

```bash
# 1. 编辑清单，加入 nginx129（复用 nginx128 的模板目录）
#    src-tauri/services/version_manifest.json

# 2. 无需新建目录（已在清单中指定 service_dir: "nginx128"）

# 3. 重启应用 → 环境配置页 → 下拉框应出现 "Nginx 1.29 → nginx:1.29-alpine"
```

如果你希望 1.29 拥有独立配置：

```bash
cp -r src-tauri/services/nginx128 src-tauri/services/nginx129
# 编辑 nginx129/nginx.conf、nginx129/conf.d/ 等
# 清单中 service_dir 改为 "nginx129"
```

---

## 四、常见问题排查

| 现象 | 原因与处理 |
|---|---|
| 页面顶部出现「无法加载服务版本清单」 | 清单内置于程序中，并非工作区文件，因此不是你改坏了 JSON。持续失败通常意味着安装不完整，请重新安装后点击「重试」。 |
| 改了清单但 UI 没变化 | 清单是编译时嵌入的，必须**重新构建程序**才生效（见步骤 3）。只重启应用无效。 |
| 下拉框里看不到新加的版本 | 清单未重新加载。重启应用，或在提示条上点击「重试」。 |
| 应用配置后 `services/<dir>/` 是空的 | 模板目录缺少该服务类型的关键文件（见步骤 2 方式 C 的表格），回退到了默认目录。检查目录名与 `service_dir` 是否一致。 |
| 改了模板但工作区文件没变 | 正常行为：已存在的文件不会被覆盖。删除工作区中对应文件后重新「应用配置」。 |
| 容器名不是预期的 `ps-xxx` | 容器名由 `service_dir` 推导。检查清单里的 `service_dir` 字段。 |
| 环境变量名不符合预期 | 环境变量前缀由 `service_dir.to_uppercase()` 推导，如 `nginx129` → `NGINX129_*`。 |

---

## 五、相关文件位置

| 用途 | 路径 |
|---|---|
| 版本清单 | `src-tauri/services/version_manifest.json` |
| 模板目录 | `src-tauri/services/<名称><版本>/` |
| 清单加载（后端） | `src-tauri/src/engine/version_manifest.rs` |
| 模板目录解析与回退 | `src-tauri/src/engine/config_generator.rs`（`resolve_template_dir`） |
| 模板释放（存在则跳过） | `src-tauri/src/engine/config_generator.rs`（`copy_template_file`） |
| 前端版本下拉 | `src/components/EnvConfigPage.vue` |
| 版本与覆盖展示 | `src/components/SoftwareSettings.vue` |

> 模板同步与上游获取方案见 `docs/architecture/SERVICE_TEMPLATE_SOURCING.md`。
