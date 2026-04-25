# PHP-Stack 文档中心

> **版本**: v0.1.0  
> **最后更新**: 2026-04-25

本文档中心整理了 PHP-Stack 项目的所有技术文档，按类别组织以便于查找和维护。

---

## 📚 文档分类

### 🏗️ 架构文档 (architecture/)

系统设计和架构相关文档：

- [ARCHITECTURE.md](architecture/ARCHITECTURE.md) - 系统架构文档（核心）
  - 系统架构图
  - 核心工作流程
  - 模块详细说明
  - 数据流图
  - 关键技术决策

### 💻 实现文档 (implementation/)

功能实现和技术细节文档：

- [IMPLEMENTATION_SUMMARY.md](implementation/IMPLEMENTATION_SUMMARY.md) - v0.1.0 实现总结报告
  - 已完成功能清单
  - 代码统计与测试覆盖
  - 架构改进说明
  
- [VERSION_SCOPE.md](implementation/VERSION_SCOPE.md) - 版本定位说明
  - v0.1.0 包含/不包含的功能
  - 设计理念与用户场景
  - 常见问题解答

- [VERSION_SELECTION_OPTIMIZATION.md](implementation/VERSION_SELECTION_OPTIMIZATION.md) - 版本选择界面优化说明
  - 优化目标与实现
  - 用户体验改进

### 📖 使用指南 (guides/)

用户和开发者指南：

- [MIRROR_GUIDE.md](guides/MIRROR_GUIDE.md) - 镜像源配置使用指南
  - 配置项说明
  - 作用时机
  - 配置步骤
  - 常见问题

- [QUICK_REFERENCE.md](guides/QUICK_REFERENCE.md) - 快速参考指南
  - 核心功能速览
  - API 命令参考
  - 开发规范速查
  - 常见问题

- [TESTING_GUIDE.md](guides/TESTING_GUIDE.md) - 测试规范指南 ⭐️新增
  - Rust后端测试规范
  - Vue前端测试规范
  - 测试编写示例
  - 最佳实践

- [TEST_RESTRUCTURE.md](guides/TEST_RESTRUCTURE.md) - 测试结构重构说明 ⭐️新增
  - 重构前后对比
  - 目录结构说明
  - 使用示例

- [TESTING_QUICK_REF.md](guides/TESTING_QUICK_REF.md) - 测试快速参考 ⭐️新增
  - 常用命令
  - 代码模板
  - 覆盖目标

### 📜 历史文档 (history/)

项目开发过程中的历史记录、问题修复和优化记录：

#### 开发日志
- [Dev.log.md](history/Dev.log.md) - 开发日志

#### 代码清理
- [CLEANUP_PLAN.md](history/CLEANUP_PLAN.md) - 代码清理计划（已执行完成）

#### 镜像配置重构
- [MIRROR_CONFIG_REFACTOR_SUMMARY.md](history/MIRROR_CONFIG_REFACTOR_SUMMARY.md) - 镜像源管理重构总结（合并版）

#### 问题修复归档
- [FIXES_ARCHIVE_SUMMARY.md](history/FIXES_ARCHIVE_SUMMARY.md) - 问题修复历史归档摘要
- FIX_*.md 系列 - 详细问题修复报告（供深度参考）
  - [FIX_CONFIG_GENERATOR_PATH.md](history/FIX_CONFIG_GENERATOR_PATH.md)
  - [FIX_USER_OVERRIDE_NOT_APPLIED.md](history/FIX_USER_OVERRIDE_NOT_APPLIED.md)
  - [FIX_VERSION_KEY_MATCHING.md](history/FIX_VERSION_KEY_MATCHING.md)
  - [FIX_ENV_CONFIG_AUTO_SELECT.md](history/FIX_ENV_CONFIG_AUTO_SELECT.md)

#### 其他历史记录
- VERIFY_*.md 系列 - 验证报告
- REFACTOR_*.md 系列 - 重构记录
- USER_OVERRIDE_*.md - 用户覆盖功能文档
- VERSION_*.md - 版本相关文档

#### AI 辅助开发文档（按日期归档）
- [2026-04-25_REALTIME_LOG_DISPLAY_OPTIMIZATION.md](history/2026-04-25_REALTIME_LOG_DISPLAY_OPTIMIZATION.md) - Docker Compose 实时日志显示优化
- [2026-04-25_DOCKER_COMPOSE_COMPATIBILITY_FIX.md](history/2026-04-25_DOCKER_COMPOSE_COMPATIBILITY_FIX.md) - Docker Compose 跨平台兼容性修复
- [2026-04-16_Docker_Compose_集成实施.md](history/2026-04-16_Docker_Compose_集成实施.md) - Docker Compose 集成实施方案
- [2026-04-17_requirements.md](history/2026-04-17_requirements.md) - 环境配置与备份需求文档
- [2026-04-17_design.md](history/2026-04-17_design.md) - 系统设计方案
- [2026-04-17_tasks.md](history/2026-04-17_tasks.md) - 开发任务清单

> 💡 **提示**: 历史文档主要用于了解项目开发过程和问题解决思路，日常开发可参考架构文档和使用指南。

---

## 🎯 快速导航

### 新用户入门
1. 阅读根目录 [README.md](../README.md) 了解项目概况
2. 查看 [guides/MIRROR_GUIDE.md](guides/MIRROR_GUIDE.md) 学习镜像源配置
3. 参考 [guides/QUICK_REFERENCE.md](guides/QUICK_REFERENCE.md) 快速上手
4. 阅读 [guides/TESTING_QUICK_REF.md](guides/TESTING_QUICK_REF.md) 了解测试命令

### 开发者必读
1. [architecture/ARCHITECTURE.md](architecture/ARCHITECTURE.md) - 理解系统架构
2. [implementation/IMPLEMENTATION_SUMMARY.md](implementation/IMPLEMENTATION_SUMMARY.md) - 了解实现细节
3. [guides/TESTING_GUIDE.md](guides/TESTING_GUIDE.md) - 掌握测试规范 ⭐️新增
4. 根目录 [AGENTS.md](../AGENTS.md) - AI Agent 开发指南

### 版本信息
- [implementation/VERSION_SCOPE.md](implementation/VERSION_SCOPE.md) - 版本定位和功能范围
- 根目录 [CHANGELOG.md](../CHANGELOG.md) - 版本变更历史

---

## 📝 文档维护规范

### 文档分类原则

| 文档类型 | 存放位置 | 示例 |
|---------|---------|------|
| 架构设计 | `doc/architecture/` | 系统架构图、模块设计 |
| 实现总结 | `doc/implementation/` | 功能实现报告、优化说明 |
| 使用指南 | `doc/guides/` | 用户手册、快速参考 |
| 历史记录 | `doc/history/` | 问题修复、重构记录、开发日志 |
| 项目说明 | 根目录 | README.md, CHANGELOG.md |
| 开发指南 | 根目录 | AGENTS.md |

### 文档命名规范

- 使用大写字母和下划线：`FEATURE_NAME.md`
- 架构文档：简洁明了，如 `ARCHITECTURE.md`
- 实现文档：包含版本或功能名，如 `IMPLEMENTATION_SUMMARY.md`
- 历史文档：保留原始名称，便于追溯

### 文档更新流程

1. **新增文档**：根据内容类型放入对应目录
2. **更新文档**：修改后更新"最后更新"日期
3. **过时文档**：移至 `doc/history/` 并标注状态
4. **重复文档**：合并内容后删除冗余版本
5. **更新索引**：在本文档中添加/更新链接

---

## 🔗 相关链接

- [项目主页](../README.md)
- [AI Agent 指南](../AGENTS.md)
- [变更日志](../CHANGELOG.md)
- [GitHub 仓库](https://github.com/your-repo/php-stack)

---

**维护者**: PHP-Stack Team  
**文档版本**: 1.0
