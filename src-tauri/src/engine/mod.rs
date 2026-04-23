pub mod mirror_config;      // 镜像源配置（向后兼容）
pub mod env_parser;          // Env 解析器/格式化器
pub mod backup_manifest;     // 备份清单数据模型与序列化
pub mod config_generator;   // 可视化配置生成器
pub mod mirror_manager;     // 统一镜像源管理器
pub mod backup_engine;      // 增强备份引擎
pub mod restore_engine;     // 恢复引擎
pub mod version_manifest;   // 版本清单管理器
pub mod user_override_manager; // 用户版本覆盖管理器
pub mod mirror_config_manager; // 用户镜像源配置管理器
pub mod workspace_manager;     // 工作目录管理器
