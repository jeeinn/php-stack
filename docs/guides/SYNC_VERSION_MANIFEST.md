# 版本清单同步工作流

> **TL;DR**：本地/CI 跑 `npm run sync:manifest`，自动从上游拉版本元数据并写回 `version_manifest.json`；新条目会自动复用已有模板目录，不建新目录。

## 为什么需要这个脚本

`src-tauri/services/version_manifest.json` 之前是手工维护的，结果严重滞后：

- MySQL 缺整条 9.x（最新 9.7.2，LTS 到 2034）
- Redis 缺 8.4 / 8.6 / 8.8 / 8.10（最新 8.10.1）
- Nginx 缺 1.29 / 1.30 / 1.31（最新 1.31.5）
- 多个版本已 EOL 但 manifest 的 `eol` 字段未更新（php81、nginx128、mysql80、redis70 等）

本脚本在**仓库侧**（开发者本地或 CI）定时跑，把"哪些版本算 stable"的决策外包给 [endoflife.date](https://endoflife.date)（权威 EOL 数据源），再在 [docker-library/official-images](https://github.com/docker-library/official-images) 里查每个 cycle 对应的 latest tag。

> **不**放进 App 运行时：本项目用户是网络受限群体（内置镜像源功能就是为此存在），让核心功能依赖外网与产品定位冲突。App 保持完全离线使用内置清单，本脚本只在仓库侧触发。

## 用法

```bash
# 默认（拉数据 + 打印报告，不写文件）
node scripts/sync-version-manifest.mjs

# CI 用：有差异则 exit 1（阻断合并）
npm run check:manifest

# 写回 version_manifest.json
npm run sync:manifest

# 离线模式：跳过网络，使用 .workbuddy/sync-cache/ 的缓存
node scripts/sync-version-manifest.mjs --offline --apply
```

四个标志位：

| 标志 | 作用 |
|---|---|
| `--check` | CI 模式：有差异时 exit 1，否则 exit 0 |
| `--apply` | 写回 `version_manifest.json`（仅新增条目；不动已有） |
| `--offline` | 复用 `.workbuddy/sync-cache/` 缓存，不联网（供断网环境调试） |

## 报告解读

脚本输出三段 markdown 表：

### 1. 上游新增版本（manifest 缺失）

eol api 列了但 manifest 还没有的 cycle。**`--apply` 模式会自动写入**——新增条目的 `service_dir` 自动复用 cycle 内已有目录（如 `redis84` → `service_dir: "redis82"`），不新建目录。

EOL 列里 `false` 表示仍在支持；`2029-12-31` 这种日期表示支持到期日；`⚠️ true` 表示上游已终止所有支持，写入会自动标 `eol: true`。

### 2. 现有条目 image_tag 落后

manifest 的 `image_tag` 用了 short form（如 `php:8.5-fpm`），上游已发了 patch 升级（如 `8.5.10-fpm`）。**`--apply` 模式不会自动改这条**——patch 升级可能引入兼容性变化，必须人工 review。

### 3. EOL 状态变化

manifest 里 `eol: false` 但 upstream 已 EOL 的条目。**`--apply` 模式不会自动改这条**——是否标 EOL 涉及业务判断（公司可能买商业支持）。

## 新增条目的 service_dir 复用规则

这是项目的隐式约定（详见 [`docs/architecture/SERVICE_TEMPLATE_SOURCING.md`](../architecture/SERVICE_TEMPLATE_SOURCING.md)）：

> 加新版本 = 改一行 JSON + 复用已有模板目录，**不改代码**。

`scripts/sync-version-manifest.mjs` 严格遵循这条：

- 如果 cycle 内已有任意条目（如 `redis62`、`redis72`、`redis82`），新条目（如 `redis84`、`redis810`）的 `service_dir` 自动复用其中第一个
- 如果 cycle 是新增的全新服务（如用户加了 `mariadb`），则会失败（因为没有任何已有目录可参考），需要用户手工指定 `service_dir`

## 工作流（推荐）

```bash
# 1. 在本地拉数据，看 diff
npm run sync:manifest  # 这步会用 --apply 写回

# 2. 检查 git diff
git diff src-tauri/services/version_manifest.json

# 3. 对需要更新的 patch 升级（如 8.4 → 8.4.11），手动修改 manifest
#    对已 EOL 条目，根据业务决定是否标 eol: true

# 4. 提交
git add src-tauri/services/version_manifest.json
git commit -m "chore(manifest): 同步上游版本元数据"
```

## CI 集成建议

```yaml
# .github/workflows/sync-manifest.yml（建议示例，未实施）
name: check-version-manifest
on:
  schedule:
    - cron: '0 6 * * 1'  # 每周一 6 点跑一次
  workflow_dispatch:        # 也支持手动触发

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm run check:manifest
      # 有差异时 exit 1，可选：自动创建 PR
      # - run: npm run sync:manifest
      # - uses: peter-evans/create-pull-request@v6
```

## 设计红线（不可破坏）

1. **永不删除 manifest 已有条目**——用户手工维护的可能被覆盖，`--apply` 只增不改。
2. **新增条目复用已有 `service_dir`**——不自动建目录（避开 `config_generator.rs` 的隐式约定）。
3. **patch 升级不自动**——`--apply` 只处理新增；现有 image_tag 落后需人工确认。
4. **EOL 标定不自动**——是否把 `eol` 翻 true 涉及商业判断，必须人工 review。
5. **`--apply` 模式下 fetch 失败必须报错退出**——禁止写半截不一致的 manifest。