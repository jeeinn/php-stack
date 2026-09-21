# 服务模板（services）获取方案

> 状态：v2（含 Phase 3 运行时按需提取方案）· 2026-09-04
> 范围：`src-tauri/services/` 模板体系 · 涉及版本清单、配置基线、构建定义三层
> 本文所有上游事实均在本文档产出时实测验证（见 §3 证据表）

---

## 1. 结论摘要

**核心判断：不要去找"官方模板"这个整体，因为它不存在。**

`services/` 下的文件其实是三类性质完全不同的东西，它们的"官方可得性"差异极大，必须拆开处理：

| 层 | 内容 | 官方可得？ | 策略 |
|---|---|---|---|
| L1 版本元数据 | `version_manifest.json` | ✅ 有权威源 | **全自动同步** |
| L2 配置基线 | `php.ini` / `mysql.cnf` / `redis.conf` / `nginx.conf` | ⚠️ 部分，需加工 | **半自动提取 + 本地 overlay** |
| L3 构建定义 | `Dockerfile` | ❌ 无可靠上游 | **继续手工，但先参数化去重** |

**推荐主线：把同步动作放在 CI，不放进 App。** 建立一个模板同步工作流，定时从上游拉取 → 生成 diff → 提 PR → 人工 review → 合并后随下个 App 版本发布快照。

> **术语澄清（重要）：** 这里说的"不放进 App"**不是要求 App 零联网**。本项目已有且必须保留的联网能力（Docker Registry 镜像源、APT/Composer/NPM 镜像源）属于**执行期联网**——由用户主动触发、用于拉取已发布的稳定产物、且有镜像源可加速兜底。本提案反对的是新增**定义期联网**——即在运行时去抓取"模板与版本元数据"这类不断漂移的上游定义。两者的失败影响完全不同：前者失败用户立刻感知并可通过换源恢复；后者失败会导致版本列表本身残缺，且**不在镜像源的覆盖范围内**（见 §6 实测）。

理由见 §6 关键决策。

---

## 2. 现状拆解

### 2.1 目录现状

```
src-tauri/services/
├── version_manifest.json        # L1：版本清单，include_str! 编译进二进制
├── php56/ php74/ php80/ php81/ php82/ php83/ php84/ php85/
│   └── Dockerfile, php.ini, php-fpm.conf
├── mysql57/ mysql80/ mysql84/    # mysql.cnf
├── redis62/ redis70/ redis72/ redis82/  # redis.conf
└── nginx124/ ~ nginx128/         # Dockerfile, nginx.conf, conf.d/
```

### 2.2 三个已存在的问题

**P0 — 配置文件来源不一致、无溯源。**
`redis.conf` 各版本体积差异悬殊：

| 文件 | 体积 | 说明 |
|---|---|---|
| `redis62/redis.conf` | 3.4 KB | 精简版 |
| `redis70/redis.conf` | 3.6 KB | 精简版 |
| `redis72/redis.conf` | **63.5 KB** | 疑似完整官方配置 |
| `redis82/redis.conf` | 3.9 KB | 且与 `README.md` 内容重复（3.9 KB，同尺寸） |

同样一类文件，四个版本四种形态，无法回答"这份配置从哪来、被改过什么"。`redis82/README.md` 与 `redis.conf` 字节数完全相同，疑似误复制。

**P1 — Dockerfile 高度重复。**
`php74`/`php80` 各 2680 字节，`php80`~`php85` 各 2652 字节。差异仅 `ARG PHP_BASE_IMAGE` 的默认值。新增一个 PHP 版本 = 复制目录 + 改一行 + 改 `version_manifest.json`，三处手工。

**P1 — 版本清单已滞后上游。** 见 §4。

---

## 3. 上游数据源验证结果

以下均为本次实测（2026-09-03），非推测。

### 3.1 L1 版本元数据

| 数据源 | 端点 | 结果 | 用途 |
|---|---|---|---|
| docker-library/official-images | `raw.githubusercontent.com/.../library/{php,mysql,redis,nginx}` | ✅ 全部 200 | **权威 tag 清单**，含 `GitRepo`/`GitCommit`/`Directory` |
| endoflife.date | `endoflife.date/api/{php,mysql,redis,nginx}.json` | ✅ 四服务全覆盖 | EOL 日期、latest、releaseDate |
| Docker Hub API v2 | `hub.docker.com/v2/repositories/library/php/tags` | ❌ 本环境不通（HTTP 000） | **不采用** |

**为什么用 official-images 而不是逐个 `docker-library/*` 仓库**：实测发现 Redis 官方镜像的构建仓库已从 `docker-library/redis` 迁移到 `redis/docker-library-redis`（前者目录只到 7.4，后者承载 8.x）。**构建仓库会迁移、会分裂，只有 `official-images` 的 `library/*` 是稳定的统一入口**——它只描述 tag 与产物的映射，不绑定单一构建仓库。

`library/*` 文件结构（实测样本）：

```
Maintainers: ...
GitRepo: https://github.com/redis/docker-library-redis.git

Tags: 8.10.1, 8.10, 8, 8.10.1-trixie, 8-trixie, latest, trixie
Architectures: amd64, arm64v8, ...
GitCommit: 3b10654333bfc145b9eeed55c4bf495a81ac8ec0
GitFetch: refs/tags/v8.10.1
Directory: debian
```

解析 `Tags` + `Directory` 即可推出 `{版本 → service_dir}` 映射，`GitCommit` 提供溯源锚点。

### 3.2 L2 配置基线（关键发现）

**docker-library 仓库里没有配置文件**——只有 `Dockerfile`、`docker-entrypoint.sh`、构建辅助脚本。实测 `docker-library/php/8.5/trixie/fpm/` 内容为 `Dockerfile` + 6 个 `docker-php-*` 脚本，无 `php.ini`；`docker-library/mysql/8.4/` 仅 `Dockerfile.oracle` + `docker-entrypoint.sh`。

**真正的"官方默认配置"在镜像内部。** 实测提取路径（`docker create` + `docker cp`，**不启动容器**，无副作用）：

| 服务 | 镜像内路径 | 实测结果 |
|---|---|---|
| PHP | `/usr/local/etc/php/php.ini-production` | ✅ 1974 行 / 73.9 KB |
| MySQL | `/etc/my.cnf` | ✅ 含 `!includedir /etc/mysql/conf.d/` |
| Nginx | `/etc/nginx/nginx.conf`、`/etc/nginx/conf.d/default.conf` | ✅ 644 / 1072 字节 |
| Redis | — | ❌ **官方镜像不含 redis.conf**（全盘搜索为空） |

Redis 需走源码仓库兜底：

```
raw.githubusercontent.com/redis/redis/{branch}/redis.conf
  7.2      → 107512 bytes
  8.2      → 111227 bytes
  unstable → 130773 bytes
```

这也解释了 §2.2 的 P0：项目里的 `redis72/redis.conf`（63.5 KB）既不是官方原版（107.5 KB），也不是其他版本的同款精简版，来源已不可考。

### 3.3 L3 构建定义

`docker-library/php` 等仓库的 `Dockerfile` 是**上游的构建源码**，不是给用户挂载的成品——它负责编译 PHP、下载源码、校验签名。而本项目的 `Dockerfile` 做的是另一件事：PUID/PGID 映射、扩展安装、镜像源注入、GitHub proxy。**这层差异正是本产品的价值所在**（dnmp 类工具的核心），不应也无法从上游获得。

---

## 4. 当前版本滞后情况

| 服务 | 项目最新 | 上游最新（实测） | 滞后 |
|---|---|---|---|
| PHP | `php85` (8.5) | **8.6-rc / 8.6.0beta2** | 1 个大版本 |
| MySQL | `mysql84` (8.4) | **9.7.2**（另有 8.4 线） | 缺 9.x 整条线 |
| Redis | `redis82` (8.2) | **8.10.1** | 4 个小版本 |
| Nginx | `nginx128` (1.28) | **1.31.5** | 3 个小版本 |

EOL 数据同样过期风险高——`version_manifest.json` 里 `eol` 字段为手填布尔值，例如 `php81` 标注"安全支持至 2025-12"，而当前已是 2026-09。手工维护必然漂移。

---

## 5. 方案设计

### 5.1 总体架构

```
上游                     CI（定时/手动）                    产物
────────────────────────────────────────────────────────────────
official-images    ─┐
  library/*         │                                  version_manifest.json
                    ├──►  sync 脚本 ──► diff ──► PR ──►  (+ provenance 字段)
endoflife.date     ─┘                                   ↑ 人工 review
  api/*.json                                            后合并

官方镜像            ─┐
  docker create+cp  ├──►  extract 脚本 ──────────────►  baselines/<svc>/<ver>/
redis 源码仓库      ─┘                                   + provenance.json

本地 overlay 仓库   ────►  合并（CI 内）──────────────►  services/<svc>/
  php.ini.d/*.conf       基线 ⊕ overlay
```

### 5.2 L1：版本清单同步

新增同步脚本（建议 `scripts/sync-versions.mjs` 或 Rust 侧独立 bin）：

1. 拉取 `library/{php,mysql,redis,nginx}`，解析 `Tags` + `Directory`
2. 过滤出与本项目形态匹配的变体：
   - PHP → `-fpm` 结尾（排除 cli/apache/zts/alpine）
   - MySQL → `8.x`（排除 innovation 线，或单列）
   - Redis → 主版本 tag
   - Nginx → `1.xx`（排除 perl/otel/alpine 变体）
3. 查 `endoflife.date` 补 `eol` 与 `description`
4. 与现有 `version_manifest.json` 做 diff，输出新增/删除/EOL 变更
5. **不自动写文件**——生成 PR，人工确认

`version_manifest.json` 建议新增 `provenance` 字段：

```json
{
  "php85": {
    "display_name": "PHP 8.5",
    "image_tag": "php:8.5-fpm",
    "service_dir": "php85",
    "eol": false,
    "_provenance": {
      "source": "docker-library/official-images@library/php",
      "upstream_commit": "e990b2ed...",
      "eol_source": "endoflife.date/api/php.json",
      "synced_at": "2026-09-03T14:30:00Z"
    }
  }
}
```

`_provenance` 前缀下划线，前端序列化时忽略，不影响现有 TS 类型（`src/types/env-config.ts`）。

### 5.3 L2：配置基线提取 + overlay

**提取器**（Rust 命令 `extract_official_baseline`，或 CI 脚本）：

```bash
# 不启动容器，零副作用
docker create --name tmp_<id> php:8.4-fpm
docker cp tmp_<id>:/usr/local/etc/php/php.ini-production ./php.ini
docker rm tmp_<id>
```

Redis 例外，走 `https://raw.githubusercontent.com/redis/redis/<branch>/redis.conf`。

**落地形态**——关键：不要直接把 1974 行的官方 `php.ini` 丢给用户。现有 `php84/php.ini`（10 行精简版）本质上已经是"overlay"，只是没被显式承认。改为：

```
baselines/php/8.4/php.ini            # 官方原版，只读，带 provenance.json
overlays/php/php.ini.d/10-php-stack.conf   # 本项目推荐值
```

释放时合并：

```
最终 php.ini = 官方基线（1974 行，全部注释）
             + 末尾追加 overlay 段（memory_limit / timezone / upload_max_filesize ...）
```

这样既拿到官方完整性（用户能查到任意指令的默认值与注释），又保留项目推荐值，且 overlay 独立可审计。

**P0 优先修 redis**：用 `redis/redis` 对应分支的官方 `redis.conf` 重生成四个版本，消除 63.5 KB 异常文件与 `README.md` 误复制。

### 5.4 L3：Dockerfile 参数化

现状 6 份 PHP Dockerfile 仅 `ARG PHP_BASE_IMAGE` 默认值不同。收敛为单模板：

```
templates/php/Dockerfile.template    # ARG PHP_BASE_IMAGE 无默认值或占位
```

构建时由 `config_generator.rs` 注入 `build.args.PHP_BASE_IMAGE`（该机制**已存在**，见 `config_generator.rs:326`），无需改动 compose 生成逻辑。

收益：新增 PHP 8.6 = 加一条 `version_manifest.json` 记录，不再复制目录。

---

## 6. 关键决策：同步放 CI，不放进 App

| 维度 | App 内联网同步 | CI 同步 + 内置快照（推荐） |
|---|---|---|
| 离线可用 | ❌ 断网即功能降级 | ✅ 与当前行为完全一致 |
| Tauri 权限 | 需新增网络 capability、扩大攻击面 | ✅ 零改动 |
| 网络失败处理 | 需处理代理/超时/限流/部分失败 | ✅ CI 侧重试即可 |
| 用户环境差异 | 内网用户大面积失败（本项目用户正是镜像源不通的群体） | ✅ 无关 |
| 更新节奏 | 跟随 App 版本 | ✅ 解耦，可随时发 |
| 变更可审计 | 用户端静默变更 | ✅ PR review |

### 6.1 关于"镜像源功能"的必要性与本提案的边界

镜像源切换功能是本项目的**核心能力，必须保留**，本提案不涉及它的任何改动。关键在于：它和"模板同步"需要的网络能力**不是同一类**。

项目 `services/mirror_config.json` 当前仅覆盖四类源：`docker_registry`、`apt`、`composer`、`npm`。这四类解决的都是**"拉取已发布的稳定产物"**（镜像层、deb 包、composer 包、npm 包），且每一类都有国内镜像站可加速。

而模板同步要访问的 `raw.githubusercontent.com` 与 `hub.docker.com/v2` **不在上述任何一类的覆盖范围内**，项目也没有提供对应的 GitHub 加速能力。实测（2026-09-03，同一网络环境）：

| 端点 | 用途 | 实测结果 |
|---|---|---|
| `mirrors.aliyun.com/debian/ls-lR.gz` | APT 源（镜像源已覆盖） | ✅ `200` |
| `raw.githubusercontent.com/redis/redis/8.2/redis.conf` | 配置基线 | ❌ `000`（连接失败） |
| `hub.docker.com/v2/` | 版本元数据 | ❌ `000`（连接失败） |
| `endoflife.date/api/*.json` | EOL 数据 | ⚠️ 时通时断 |

同一时刻，镜像站通、上游定义站不通。这直接说明：**镜像源加速能力无法外推到模板同步场景**。若把同步放进 App，用户端既没有加速手段、也没有降级路径，失败率只会比 CI 更高。

### 6.2 结论

因此决策不是"App 不该联网"，而是：**定义期的数据获取应发生在有重试、有 review、有稳定出口的一侧（CI），而非在网络条件不可控的用户端。** App 继续保留全部执行期联网能力（镜像源、依赖下载），并把模板与版本清单作为随版本发布的快照内置。

---

## 7. 红线（不可违反）

1. **永不覆盖用户已修改的文件。** 现有 `copy_template_file` 的"目标存在则跳过"语义（`config_generator.rs:493-495`）必须保留。基线只用于**新建**，不能用于升级已有环境。
2. **同步产物不自动合入。** 必须经 PR + 人工 review。上游 tag 可能出现 `rc`/`beta`（如 PHP 8.6.0beta2），自动合入会把预发布版推给用户。
3. **不引入 Docker Hub API 依赖。** 实测不通且有限流。版本源只用 `official-images` 的 raw 文件。
4. **Dockerfile 不从上游复制。** 上游 Dockerfile 是构建源码，与本项目职责不同。
5. **不破坏 `{name}{version}` 命名约定。** 目录名、`env_prefix`、容器名 `ps-{service_dir}` 三者的推导关系必须保持（见 §8.1）。任何"更优雅"的抽象若要求用户改代码才能加版本，一律否决。
6. **不删除用户手写的清单条目。** CI 同步只做"新增 + 标记 EOL"，不做"删除"。用户自己添加的版本（如内部私有镜像）不得被同步逻辑清除。

---

## 8. 一等约束：`{name}{version}` 命名约定必须保留

### 8.1 约定为何是一等公民

`services/` 下的目录名（`php85`、`mysql84`、`redis72`、`nginx128`）不是随意命名，而是贯穿全项目的**隐式契约**。它同时被以下位置消费：

| 位置 | 用法 | 说明 |
|---|---|---|
| `version_manifest.json` → `service_dir` | 清单条目指向模板目录 | 用户自添加版本的入口 |
| `config_generator.rs:189/317` | `env_prefix = service_dir.to_uppercase()` | 直接生成 `PHP85_CONF_FILE` 等变量名 |
| `config_generator.rs:321-411` | compose 服务名、容器名 `ps-{service_dir}`、build context | 容器识别依赖 `ps-` 前缀 |
| 目录映射 | `./services/{dir}`、`./data/{dir}`、`./logs/{dir}` | 三者必须同名 |
| `resolve_template_dir` (`:531`) | 按 `service_dir` 查找模板目录 | 找不到时回退默认目录 |

**这份约定的价值在于"项目停止维护后用户仍可自救"**：用户只需在 `version_manifest.json` 加一条、在 `services/` 建一个同名目录，程序就能按固定规则查到并复制，无需改代码。这个设计意图必须保留——本提案的任何改动都不得破坏它。

因此：**不引入与目录名解耦的新抽象层**（如"模板包 ID"或哈希目录）。新增版本继续遵循 `name + version`（去点）规则。

### 8.2 但 `service_dir` 已提供足够的解耦点

`version_manifest.json` 中 `service_dir` 与 `id` **已经是两个独立字段**（当前恰好同名）。这意味着新增版本**不必新建模板目录**：

```jsonc
// 新增 PHP 8.6：复用 php85 模板目录（Dockerfile 参数化后仅 ARG 不同）
"php86": {
  "display_name": "PHP 8.6",
  "image_tag": "php:8.6-fpm",
  "service_dir": "php85"   // ← 指向已有目录，零新增文件
}
```

配合 `resolve_template_dir` 已有的回退逻辑（`config_generator.rs:531-551`，找不到精确目录时用 `php85`/`mysql80`/`redis72`/`nginx127`），**"加版本"退化为"改一行 JSON"**。这正是命名约定想要的效果，且与自动化同步天然兼容：CI 同步只需改清单，不必凭空造目录。

⚠️ 注意：回退目录当前是硬编码的（`php85`/`mysql80`/`redis72`/`nginx127`），若这些目录被删除或改名，回退会失效。建议改为"该服务类型下版本号最高的目录"，或在清单中显式声明 `fallback_dir`。

### 8.3 发现的问题：前端硬编码兜底削弱了"用户自维护"

`src/components/EnvConfigPage.vue:180-203` 在 `get_version_mappings` 调用失败时，会 fallback 到一份**硬编码的版本列表**。该列表已与当前清单脱节（含 `php56`…`php84`，但**缺 `php85`**；nginx 只到 1.27，**缺 1.28**）。

后果：用户按约定加好 `nginx129` 后，若后端调用异常，UI 会显示这份陈旧列表，让用户误以为"改配置没生效"。**这与 §8.1 的设计意图直接冲突。**

建议：删除硬编码列表，改为显示明确的错误态 + 重试按钮（或在 `services/version_manifest.json` 解析失败时给出可操作的提示）。软件"不装了也能看懂"的前提，是错误信息要指出真实原因，而不是用陈旧数据掩盖故障。

> 补充：`src/components/SoftwareSettings.vue:232` 已正确展示 `version.service_dir`，用户能直观看到目录名，这个设计应保持。

---

## 9. 落地路线

| 阶段 | 内容 | 状态 |
|---|---|---|
| **Phase 0** | ① 删除 `EnvConfigPage.vue` 硬编码兜底，改为错误态 + 重试（§8.3）<br>② 编写 `docs`：`如何添加新版本`（三步：加清单条目 → 建同名目录/复用 → 验证） | ✅ 完成（c97929c 之前） |
| **Phase 1** | ① 用官方源码重生成 4 份 `redis.conf`（修 P0）<br>② PHP Dockerfile 收敛为参数化模板<br>③ `version_manifest.json` 加 `_provenance` 字段 | ✅ 完成（609ffbe） |
| **Phase 2** | CI 版本同步脚本（official-images + endoflife.date），输出 diff PR | ✅ 完成（c97929c） |
| **Phase 3** | **运行时按需配置提取**（apply 阶段从官方镜像 `docker create` + `cp` 提取） | 🔄 进行中（本文档定稿时） |
| **Phase 4**（可选） | App 内"检查模板更新"按钮——仅提示有新版本、引导去下载新版，不直接联网改文件 | 待评估 |

**Phase 0 优先的理由**：它不改动任何数据结构，只修掉"用户改了配置却看不到效果"这个最伤信任的问题，并把"如何加版本"从口口相传变成文档。这一步完成后，即使后续自动化一直不做，用户自维护的路径也是完整且可验证的。

### 9.1 Phase 3 设计方向变更

**v1（已被否决）**："配置基线提取器"作为离线 CI 工具，开发者跑一次把 13 个镜像的基线提出来存进仓库。用户应用时直接复制静态文件。

**v2（采用）**："运行时按需提取"——用户 apply 时，程序从该版本对应的官方镜像中提取默认配置，**不再维护 13 份离线基线**。

### 9.2 Phase 3 v1 → v2 的关键反转

v1 的"提一次用很久"是错觉——基线会随上游镜像漂移（PHP 8.5 → 8.6 改动、`default_authentication_plugin` 跨大版本行为变化等），离线基线必然过期。

更隐蔽的问题：**冷门版本（如 MySQL 5.6）回退到主流模板（mysql80）会拿到错误配置**。8.0 的 `default_authentication_plugin=caching_sha2_password` 在 5.6 里直接报错。

v2 把"提取"从"开发者维护负担"变成"用户使用时的一次性自动动作"：
- 用户加 `mysql56` 到 manifest + 加 `services/mysql56/`（或复用 `service_dir: "mysql80"`）
- apply 时，程序检查 `services/mysql56/mysql.cnf` 是否存在 → 不存在 → 检查本地有 `mysql:5.6` 镜像 → 有则 `docker create`+`cp` / 无则汇总弹窗让用户确认拉取 → 拉取 → 提取
- **该版本配置永远与官方镜像一致**，无需开发者预生成

---

## 10. Phase 3：运行时按需配置提取（v2 详细设计）

### 10.1 触发时机

集成到 `ConfigGenerator::apply` → `generate_service_dirs`（`config_generator.rs:565`）的**复制逻辑之前**，对每个 service 串行处理。

不独立成单独 Tauri command（避免用户先点 apply 再触发拉取的二次操作），合并到 apply 流程的"服务目录准备"阶段。

### 10.2 拉取决策表

```
对每个 service:
  ① 目标 workspace 配置文件已存在（用户已改过）
     → 跳过（保留用户修改，不覆盖）✅
  ② 内置模板目录有该 service_dir 的对应配置
     → 模板直接复制（零成本，现状行为）✅
  ③ 都没有（需要从镜像提取）
     → 检查本地是否有 image_tag 镜像
     → 有 → docker create + cp + rm 容器（**不删镜像**）
     → 无 → 加入"待拉取列表"
  ④ 待拉取列表非空
     → 弹窗汇总（"需要拉取：mysql:5.6 (689MB)、php:5.6-fpm (110MB)，共 799MB，确认？"）
     → 用户取消 → 回滚到可配置状态（不写 compose / 不动 workspace）
     → 用户确认 → 串行 pull → 回到 ③ 继续
  ⑤ 提取失败（任何环节）
     → 降级到现有 fallback 模板 + warning 日志
     → apply 不阻断（容器启动后用镜像内置默认值）
```

### 10.3 镜像生命周期：拉一次用两次，不主动 rmi

**关键决策**：提取完镜像**不删除**。

- 提取动作：需要 `docker cp`（镜像层在本地）
- 启动动作：用户后续点"一键启动"时，docker compose up 复用本地缓存（**秒启**，避免重新 pull）
- "拉一次用两次"——零带宽浪费
- 镜像生命周期完全由用户管理（`docker image prune` 是用户自己的事）

这与项目"执行期联网保留在 App"的架构决策一致（见 §6.1）——`docker pull` 本就是执行期联网。

### 10.4 配置映射（硬编码在 Rust 代码里）

> **不放进 manifest 的原因**：这 4 条映射**跨版本基本稳定**，放进 manifest 会让用户改 JSON 的难度上升（多一堆易错字段）。只提取"用户可能想编辑的配置文件"，Dockerfile 仍走项目自研（PUID/PGID/镜像源注入），不碰上游。

| 服务 | 镜像内路径 | 落盘名 | 备注 |
|---|---|---|---|
| PHP | `/usr/local/etc/php/php.ini-production` | `php.ini` | 5.6 是 `/usr/local/etc/php/php.ini`（无 `-production`） |
| MySQL | `/etc/my.cnf` | `mysql.cnf` | 5.6/5.7 是 `/etc/mysql/my.cnf` |
| Redis | `/usr/local/etc/redis/redis.conf` | `redis.conf` | 8.x 路径相同 |
| Nginx | `/etc/nginx/nginx.conf` | `nginx.conf` | alpine/debian 一致 |

**PHP 5.6 特殊处理**：5.6 镜像内只有 `/usr/local/etc/php/php.ini`（无 `-production` / `-development` 区分）。代码内按 PHP 主版本分支：`5.x → /usr/local/etc/php/php.ini`，`7.x+ → /usr/local/etc/php/php.ini-production`。

**MySQL 5.6/5.7 特殊处理**：镜像内 `/etc/my.cnf` 可能不存在，只有 `/etc/mysql/my.cnf`。代码内按 MySQL 主版本分支：`5.x → /etc/mysql/my.cnf`，`8.x+ → /etc/my.cnf`。

### 10.5 失败降级（关键，不能拖垮 apply）

- `docker daemon` 未起 / pull 超时 / 镜像不存在 → 静默降级到现有 fallback 模板逻辑
- apply 阶段记录 warning 日志（前端可见）
- 用户下次删掉 workspace 里的配置文件、重新 apply，仍有机会重试

**为什么能降级**：官方 MySQL/Nginx/Redis 镜像本身就有内置默认配置，docker cp 不出来也能跑。PHP 8.x 镜像启动时使用 `php.ini-production` 的默认值，仅当用户想修改时才需要落到 workspace。**提取失败不致命**。

### 10.6 镜像源复用

提取时的 `docker pull` 走 `mirror_config.json` 的 `docker_registry` 配置（阿里云等）——**不新造轮子**，直接复用 `docker pull` 的镜像源能力。

### 10.7 UI 交互：拉取确认弹窗

前端 `EnvConfigPage.vue` 的 apply 流程变更：

1. **检查阶段**：apply 时先调 `check_image_presence(services)` → 后端返回 `Vec<ServiceImageStatus>`，标注每个 service 的镜像是否本地已存在
2. **汇总阶段**：前端聚合出"待拉取列表"（含 `image_tag` + 估算大小），弹窗让用户确认
3. **拉取阶段**：用户确认后逐个调 `extract_service_config(...)` 触发 pull + create + cp
4. **降级阶段**：任何 service 提取失败 → 后端返回 warning，前端 toast 提示"已降级使用 fallback 模板"，apply 继续成功

`?` 帮助弹窗（版本下拉旁）：
- 内容 = `docs/guides/ADDING_SERVICE_VERSION.md` 精简版
- 含 MySQL 5.6 完整样例 JSON（典型冷门版本）
- 强调："apply 时程序会自动从官方镜像提取该版本的正确配置"

### 10.8 红线（不可违反）

1. **永不覆盖用户已修改的配置文件。** 复用现有 `copy_template_file` 的"目标存在则跳过"语义（`config_generator.rs:496-505`）。
2. **不主动 rmi 镜像。** 镜像生命周期由用户管理（`docker image prune`）。
3. **降级不阻断 apply。** 提取失败 → 静默回退到 fallback 模板 + warning 日志。
4. **不增加新的 Tauri 权限 capability。** 复用现有 `docker pull`/`docker create`/`docker cp`/`docker rm` 进程调用通道。
5. **不破坏 `{name}{version}` 命名约定。** 提取配置仍落到 `services/{service_dir}/` 下（沿用现有目录结构）。

### 10.9 落地步骤

1. **C1**（本文档）：架构定稿
2. **C2**（后端）：
   - 新建 `src-tauri/src/engine/config_extractor.rs` —— 配置提取器核心
   - `engine/mod.rs` 注册模块
   - `config_generator.rs::generate_service_dirs` 集成调用（在 `copy_template_file` 失败时降级为提取尝试）
   - `commands/baseline.rs`（或合并到 `env_config.rs`）—— 暴露 `check_image_presence` + `extract_service_config` Tauri commands
   - `lib.rs` 注册新 commands
   - `#[cfg(test)]` 单元测试：配置映射、降级逻辑、参数解析
3. **C3**（前端）：
   - 新建 `VersionHelpModal.vue`（`?` 弹窗）
   - 新建 `ImagePullConfirmModal.vue`（拉取确认弹窗）
   - `EnvConfigPage.vue` 集成两弹窗 + apply 流程接入
   - `src/i18n/locales/zh-CN.json` + `en.json` 同步新增 key
4. **C4**（测试 + 验证）：cargo test 88+/88+、vue-tsc 零错误、npm test 60/60

### 10.10 跟原 §5.3（v1 设计）的差异

v1 设计的"overlay 合并"机制（基线 + overlay 段）被否决。理由：

- **用户场景**：90% 用户不动配置（用镜像默认值），10% 用户改一两个参数（直接编辑配置文件）
- **overlay 抽象的代价**：引入新文件结构（`php.ini.d/*.conf`）、新合并算法、新边界场景。**为 10% 的用户加了 100% 的复杂度**
- **v2 简化**：直接 `docker cp` 出原版，用户改就改整体文件，**没有"基线 vs 用户改"的边界**——更直观

如果未来真有"系统级默认 + 用户级覆盖"的需求，再回头做 overlay（v3）。现阶段 v2 已覆盖 100% 的核心需求。

---

## 附：实测命令备查

```bash
# 版本源
curl -s https://raw.githubusercontent.com/docker-library/official-images/master/library/php
curl -s https://endoflife.date/api/php.json

# 配置基线（不启动容器）
docker create --name tmp php:8.4-fpm
docker cp tmp:/usr/local/etc/php/php.ini-production ./php.ini-production
docker rm tmp

# Redis 兜底
curl -s https://raw.githubusercontent.com/redis/redis/7.2/redis.conf
```
