# 2026-09-02 模板路径解析修复与构建脚本简化

## 问题现象

直接运行 `out/windows/release/php-stack.exe`（由旧版 `scripts/build.ps1` 从
`src-tauri/target/release/app.exe` 搬运而来）后，应用环境配置报错：

```
❌ 配置应用失败: 模板文件不存在: E:\study\php-stack\out\windows\release\services\php85/Dockerfile
```

## 根因分析

- 生产模式代码通过 `std::env::current_exe()` 的同级目录拼 `services/` 查找模板；
- `tauri.conf.json` 的 `bundle.resources = ["services/**/*"]` 会在打包安装器
  （NSIS/MSI）时将模板释放到安装目录（exe 旁），也会复制到
  `src-tauri/target/<profile>/services/`；
- 但旧版 `build.ps1` 只把 `app.exe` 单独搬运到 `out/windows/release/`，
  没有携带 `services/` 模板目录 → 被搬运的 exe 找不到模板。

验证方式：检查 NSIS 暂存脚本 `target/release/nsis/x64/installer.nsi` 与
MSI 的 `target/release/wix/x64/main.wxs`，均确认资源安装布局为
`$INSTDIR\services\...`（exe 同级），生产模式解析逻辑本身没有问题。

## 修复方案（设计决策）

**不再搬运产物到 out/ 目录**，产物保持在 tauri 源目录
（`src-tauri/target/<profile>/`），模板生命周期如下：

1. 源头：`src-tauri/services/`（开发维护处）；
2. 分发：`tauri build` 复制到 `target/<profile>/services/`；安装包将
   `services/` 释放到安装目录（exe 旁）；
3. 释放：程序「应用配置」时从模板源复制到用户 workspace 的 `services/` 目录；
4. 归属：释放后的文件归用户所有，用户可自行修复/修改。

### 代码改动（src-tauri/src/engine/config_generator.rs）

1. **统一模板解析**（不再区分 debug/release 两套路径）：
   新增 `template_base_candidates()` / `locate_template_file()`，按顺序查找：
   - exe 同级 `services/`（安装版 / tauri build 产物布局）
   - `src-tauri/services/`（开发模式，或 `--no-bundle` 后直接运行
     `target/release/app.exe` 的兜底）
   - 再上溯一级的 `src-tauri/services/`（覆盖 cargo test 测试二进制位于
     `target/<profile>/deps/` 的场景）

   报错信息会列出所有已查找路径，便于排查。

2. **释放语义变更**：`copy_template_file()` 由「内容不同则覆盖」改为
   **「目标文件已存在即跳过」**。原因：模板释放到 workspace 后归用户所有，
   重新应用配置不应覆盖用户的修改；需要恢复默认模板时，删除对应文件后
   重新应用配置即可。

### 构建脚本改动（scripts/build.ps1）

- 移除 `out/windows/<profile>/` 暂存逻辑（不再搬运 exe / 安装包）；
- 构建完成后直接打印产物路径：
  - 可执行文件：`src-tauri/target/<profile>/app.exe`
  - 服务模板：`src-tauri/target/<profile>/services/`（若 `--no-bundle`
    未复制资源则给出警告并说明运行时回退机制）
  - 安装包：`src-tauri/target/<profile>/bundle/{msi,nsis}/...`

## 兼容性说明

- 旧的 `out/windows/release/php-stack.exe` 仍为坏产物（无同级 services/），
  可直接删除 `out/` 目录；正确运行方式为
  `src-tauri/target/release/app.exe` 或安装包安装后的程序。
- 既有 workspace 中已释放的模板文件不再被覆盖，属于预期行为变更。
