//! 运行时配置基线提取器（Phase 3）
//!
//! # 设计目标
//!
//! 在用户 apply 配置时，从该版本对应的官方镜像中**按需提取**默认配置文件
//! （如 `php.ini-production` / `my.cnf` / `redis.conf` / `nginx.conf`），落到
//! 用户 workspace 的 `services/{service_dir}/` 下。
//!
//! 取代了 v1 设计里的"离线预生成 13 份基线"思路——后者存在两个根本缺陷：
//! 1. **基线会过期**（上游镜像默认配置会随版本漂移）
//! 2. **冷门版本回退到主流模板是错的**（MySQL 5.6 拿到 8.0 的 my.cnf 会报错）
//!
//! # 触发时机
//!
//! 由 `ConfigGenerator::generate_service_dirs` 在现有模板复制失败时调用。
//! 不独立成单独 Tauri command 前置步骤——避免"先点 apply 再触发拉取"的二次操作。
//!
//! # 镜像生命周期
//!
//! **不主动 rmi 镜像**。用户后续点"一键启动"时，docker compose up 复用本地缓存，
//! 避免重新 pull。"拉一次用两次"零带宽浪费。
//!
//! # 失败降级
//!
//! - `docker daemon` 未起 / pull 超时 / 镜像不存在 → 返回 `Failed`
//! - 上层调用方（`ConfigGenerator`）收到 `Failed` 时降级到现有 fallback 模板
//! - **不阻断 apply**——官方 MySQL/Nginx/Redis 镜像本身有内置默认配置，
//!   启动时用镜像默认值也能跑
//!
//! # 配置映射（硬编码）
//!
//! 不放进 `version_manifest.json` 的原因：4 条映射跨版本基本稳定，放进 manifest
//! 会让用户改 JSON 的难度上升（多一堆易错字段）。Dockerfile 仍走项目自研
//! （PUID/PGID/镜像源注入），不碰上游。

use std::path::{Path, PathBuf};
use std::process::Command;

use super::version_manifest::ServiceType as VmServiceType;

use crate::app_log;

/// 镜像本地存在性状态（前端弹窗用）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ImageStatus {
    /// 本地已有该镜像
    Present {
        tag: String,
        /// `docker images` 输出的 SIZE 字段（人类可读），用于弹窗展示
        size: Option<String>,
    },
    /// 本地没有该镜像
    Missing { tag: String },
}

/// 提取结果
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum ExtractOutcome {
    /// 成功提取到 workspace
    Extracted { dest: String, bytes: u64 },
    /// 目标文件已存在（用户已改过），不覆盖
    SkippedExists,
    /// 提取失败（daemon 未起 / 镜像不存在 / 路径不对等）
    Failed { reason: String },
}

/// 配置基线提取器
pub struct ConfigExtractor;

impl ConfigExtractor {
    /// 检查本地是否已有指定 tag 的镜像
    ///
    /// 实现：`docker images --format "{{.Repository}}:{{.Tag}} {{.Size}}"`
    /// 命中条件：输出行精确匹配 `{image_tag}` 或 `library/{image_tag}`
    /// （Docker Hub 默认 namespace 是 `library/`，官方镜像两种形式都常见）
    pub fn check_image_present(image_tag: &str) -> ImageStatus {
        let output = Command::new("docker")
            .args(["images", "--format", "{{.Repository}}:{{.Tag}} {{.Size}}"])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout);
                for line in s.lines() {
                    let line = line.trim();
                    // 格式: "<repo>:<tag> <size>"，splitn(2, ' ') 拆出 size
                    if let Some((repo_tag, size)) = line.split_once(' ') {
                        if repo_tag == image_tag || repo_tag == format!("library/{image_tag}") {
                            return ImageStatus::Present {
                                tag: image_tag.to_string(),
                                size: Some(size.to_string()),
                            };
                        }
                    }
                }
                ImageStatus::Missing {
                    tag: image_tag.to_string(),
                }
            }
            Ok(_) | Err(_) => {
                // daemon 未起 / 命令失败 → 按"不存在"处理（前端会汇总到待拉取列表）
                ImageStatus::Missing {
                    tag: image_tag.to_string(),
                }
            }
        }
    }

    /// 批量检查
    pub fn check_images_batch(image_tags: &[String]) -> Vec<ImageStatus> {
        image_tags
            .iter()
            .map(|t| Self::check_image_present(t))
            .collect()
    }

    /// 拉取镜像
    ///
    /// 失败时返回 Err。调用方需要决定如何降级（汇总到 UI 弹窗让用户决定是否继续）。
    pub fn pull_image(image_tag: &str) -> Result<(), String> {
        app_log!(
            info,
            "engine::config_extractor",
            "开始拉取镜像: {}",
            image_tag
        );
        let status = Command::new("docker")
            .args(["pull", image_tag])
            .status()
            .map_err(|e| format!("docker pull 启动失败: {e}"))?;
        if !status.success() {
            return Err(format!("docker pull {image_tag} 失败（exit code: {:?}）", status.code()));
        }
        app_log!(
            info,
            "engine::config_extractor",
            "镜像拉取完成: {}",
            image_tag
        );
        Ok(())
    }

    /// 从已存在的镜像中提取配置到 workspace
    ///
    /// **不负责拉取镜像**——调用方必须先确认镜像本地存在（`check_image_present`）
    /// 或先调用 `pull_image`。**不主动 rmi 镜像**。
    ///
    /// 流程：
    /// 1. 查目标 `services/{dir}/{name}` 是否已存在 → 存在则 `SkippedExists`
    /// 2. `docker create --name {tmp} {image_tag}`（不启动）
    /// 3. `docker cp {tmp}:{src_path} {dest_path}`（按 `extract_paths` 决定的路径）
    /// 4. `docker rm {tmp}`（始终执行，包括失败时）
    /// 5. 返回结果
    pub fn extract_config(
        service_type: &VmServiceType,
        service_dir: &str,
        image_tag: &str,
        dest_root: &Path,
    ) -> ExtractOutcome {
        // 1. 决定镜像内路径 + 落盘名
        let (src_paths, dest_name) = match Self::extract_paths(service_type, image_tag) {
            Ok(v) => v,
            Err(reason) => return ExtractOutcome::Failed { reason },
        };

        // 2. 目标已存在？跳过
        let dest_path = dest_root.join(format!("services/{service_dir}/{dest_name}"));
        if dest_path.exists() {
            app_log!(
                info,
                "engine::config_extractor",
                "目标配置已存在，跳过提取（保留用户修改）: {}",
                dest_path.display()
            );
            return ExtractOutcome::SkippedExists;
        }

        // 3. docker create（不启动）
        // 容器名用时间戳 + 进程 ID 组合，避免并发冲突
        let container_name = format!(
            "php_stack_extract_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );

        let create_status = Command::new("docker")
            .args(["create", "--name", &container_name, image_tag])
            .status();

        match create_status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                let _ = Self::cleanup_container(&container_name);
                return ExtractOutcome::Failed {
                    reason: format!("docker create 失败（exit code: {:?}）", s.code()),
                };
            }
            Err(e) => {
                return ExtractOutcome::Failed {
                    reason: format!("docker create 启动失败: {e}"),
                };
            }
        }

        // 4. docker cp 提取（按 src_paths 顺序尝试，第一个成功即可）
        let mut extracted_bytes: u64 = 0;
        let mut last_err: Option<String> = None;
        for src in &src_paths {
            if let Some(parent) = dest_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    last_err = Some(format!("创建目标目录失败: {e}"));
                    continue;
                }
            }
            let cp_status = Command::new("docker")
                .args([
                    "cp",
                    &format!("{container_name}:{src}"),
                    dest_path.to_str().unwrap_or(""),
                ])
                .status();
            match cp_status {
                Ok(s) if s.success() => {
                    if let Ok(meta) = std::fs::metadata(&dest_path) {
                        extracted_bytes = meta.len();
                    }
                    break;
                }
                Ok(s) => {
                    last_err = Some(format!(
                        "docker cp {container_name}:{src} 失败（exit code: {:?}）",
                        s.code()
                    ));
                }
                Err(e) => {
                    last_err = Some(format!("docker cp 启动失败: {e}"));
                }
            }
        }

        // 5. 清理容器（始终执行）
        let _ = Self::cleanup_container(&container_name);

        // 6. 结果判定
        if extracted_bytes == 0 {
            let reason = last_err.unwrap_or_else(|| "未提取到任何配置".to_string());
            app_log!(
                warn,
                "engine::config_extractor",
                "配置提取失败: {} ({})",
                service_dir,
                reason
            );
            return ExtractOutcome::Failed { reason };
        }
        app_log!(
            info,
            "engine::config_extractor",
            "配置提取成功: {} ← {} ({} bytes)",
            dest_path.display(),
            image_tag,
            extracted_bytes
        );
        ExtractOutcome::Extracted {
            dest: dest_path.display().to_string(),
            bytes: extracted_bytes,
        }
    }

    /// 清理临时容器（容错，不返回错误）
    fn cleanup_container(name: &str) -> Result<(), ()> {
        Command::new("docker")
            .args(["rm", name])
            .status()
            .map(|_| ())
            .map_err(|_| ())
    }

    /// 决定镜像内配置路径 + workspace 落盘名
    ///
    /// 公开便于测试。**主版本号是分支依据**（5/7/8 等跨大版本差异巨大）。
    pub fn extract_paths(
        service_type: &VmServiceType,
        image_tag: &str,
    ) -> Result<(Vec<String>, String), String> {
        let major = Self::major_version_from_tag(image_tag);
        match service_type {
            VmServiceType::Php => {
                // 5.x → /usr/local/etc/php/php.ini（无 -production 变体）
                // 7.x+ → /usr/local/etc/php/php.ini-production
                let src = if major == 5 {
                    "/usr/local/etc/php/php.ini".to_string()
                } else {
                    "/usr/local/etc/php/php.ini-production".to_string()
                };
                Ok((vec![src], "php.ini".to_string()))
            }
            VmServiceType::Mysql => {
                // 5.x → /etc/mysql/my.cnf（容器内唯一位置）
                // 8.x+ → /etc/my.cnf（含 !includedir /etc/mysql/conf.d/）
                let src = if major == 5 {
                    "/etc/mysql/my.cnf".to_string()
                } else {
                    "/etc/my.cnf".to_string()
                };
                Ok((vec![src], "mysql.cnf".to_string()))
            }
            VmServiceType::Redis => {
                // 全部版本统一路径
                Ok((
                    vec!["/usr/local/etc/redis/redis.conf".to_string()],
                    "redis.conf".to_string(),
                ))
            }
            VmServiceType::Nginx => {
                Ok((
                    vec!["/etc/nginx/nginx.conf".to_string()],
                    "nginx.conf".to_string(),
                ))
            }
        }
    }

    /// 从 image_tag 提取主版本号
    ///
    /// 例：`php:5.6-fpm` → 5, `mysql:8.4` → 8, `redis:7.2-alpine` → 7, `nginx:1.27-alpine` → 1
    /// **不要求**主版本号真实存在——`0` 表示"无法解析"（容错）。
    pub fn major_version_from_tag(tag: &str) -> u32 {
        // 取冒号后的部分，split 数字部分
        let after_colon = tag.split(':').nth(1).unwrap_or("");
        let first = after_colon
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap_or("");
        first.parse().unwrap_or(0)
    }

    /// 计算 `services/{service_dir}/{dest_name}` 的目标路径（公开便于测试）
    pub fn dest_path(dest_root: &Path, service_dir: &str, dest_name: &str) -> PathBuf {
        dest_root.join(format!("services/{service_dir}/{dest_name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_major_version_from_tag() {
        // 标准格式
        assert_eq!(ConfigExtractor::major_version_from_tag("php:8.5-fpm"), 8);
        assert_eq!(ConfigExtractor::major_version_from_tag("php:7.4-fpm"), 7);
        assert_eq!(ConfigExtractor::major_version_from_tag("php:5.6-fpm"), 5);
        assert_eq!(ConfigExtractor::major_version_from_tag("mysql:8.4"), 8);
        assert_eq!(ConfigExtractor::major_version_from_tag("mysql:5.7"), 5);
        assert_eq!(ConfigExtractor::major_version_from_tag("redis:7.2-alpine"), 7);
        assert_eq!(ConfigExtractor::major_version_from_tag("redis:8.0"), 8);
        assert_eq!(ConfigExtractor::major_version_from_tag("nginx:1.27-alpine"), 1);
        // 边缘情况
        assert_eq!(ConfigExtractor::major_version_from_tag("library/php:8.5-fpm"), 8);
        assert_eq!(ConfigExtractor::major_version_from_tag("invalid"), 0);
        assert_eq!(ConfigExtractor::major_version_from_tag(""), 0);
    }

    #[test]
    fn test_extract_paths_php() {
        // 5.6 走老路径
        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Php, "php:5.6-fpm").unwrap();
        assert_eq!(srcs, vec!["/usr/local/etc/php/php.ini"]);
        assert_eq!(dest, "php.ini");

        // 7.x+ 走 -production
        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Php, "php:7.4-fpm").unwrap();
        assert_eq!(srcs, vec!["/usr/local/etc/php/php.ini-production"]);
        assert_eq!(dest, "php.ini");

        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Php, "php:8.5-fpm").unwrap();
        assert_eq!(srcs, vec!["/usr/local/etc/php/php.ini-production"]);
        assert_eq!(dest, "php.ini");
    }

    #[test]
    fn test_extract_paths_mysql() {
        // 5.x 走 /etc/mysql/my.cnf
        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Mysql, "mysql:5.7").unwrap();
        assert_eq!(srcs, vec!["/etc/mysql/my.cnf"]);
        assert_eq!(dest, "mysql.cnf");

        // 8.x 走 /etc/my.cnf
        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Mysql, "mysql:8.4").unwrap();
        assert_eq!(srcs, vec!["/etc/my.cnf"]);
        assert_eq!(dest, "mysql.cnf");
    }

    #[test]
    fn test_extract_paths_redis() {
        // 全部版本统一
        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Redis, "redis:7.2-alpine").unwrap();
        assert_eq!(srcs, vec!["/usr/local/etc/redis/redis.conf"]);
        assert_eq!(dest, "redis.conf");

        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Redis, "redis:8.2-alpine").unwrap();
        assert_eq!(srcs, vec!["/usr/local/etc/redis/redis.conf"]);
        assert_eq!(dest, "redis.conf");
    }

    #[test]
    fn test_extract_paths_nginx() {
        let (srcs, dest) = ConfigExtractor::extract_paths(&VmServiceType::Nginx, "nginx:1.27-alpine").unwrap();
        assert_eq!(srcs, vec!["/etc/nginx/nginx.conf"]);
        assert_eq!(dest, "nginx.conf");
    }

    #[test]
    fn test_dest_path() {
        let root = Path::new("/tmp/test");
        let p = ConfigExtractor::dest_path(root, "mysql56", "mysql.cnf");
        assert_eq!(p, PathBuf::from("/tmp/test/services/mysql56/mysql.cnf"));
    }

    #[test]
    fn test_extract_config_skipped_exists() {
        // 目标已存在 → SkippedExists（不调 docker）
        use std::fs;
        let tmp = std::env::temp_dir().join(format!("php_stack_ext_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let svc_dir = "mysql56";
        let dest_dir = tmp.join(format!("services/{svc_dir}"));
        fs::create_dir_all(&dest_dir).unwrap();
        let dest_file = dest_dir.join("mysql.cnf");
        fs::write(&dest_file, "user customized content").unwrap();

        let result = ConfigExtractor::extract_config(
            &VmServiceType::Mysql,
            svc_dir,
            "mysql:5.6",
            &tmp,
        );
        match result {
            ExtractOutcome::SkippedExists => {
                // 验证文件未被覆盖
                let content = fs::read_to_string(&dest_file).unwrap();
                assert_eq!(content, "user customized content");
            }
            other => panic!("expected SkippedExists, got {:?}", other),
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    // ─── 前后端 serde 契约 ──────────────────────────────────────
    // 前端 `src/types/env-config.ts` 的 ImagePresence / ExtractResult 按字面量
    // 判断 `status` / `outcome` 字段。Rust 侧改 `rename_all` 或加 `rename`
    // 会静默破坏前端分支判断，这里把 JSON 形状锁死。
    //
    // ⚠️ 注意 `SkippedExists` → "skippedexists"（lowercase 不加下划线），
    //    若将来改成 snake_case 必须同步改前端类型。

    #[test]
    fn test_image_status_serde_contract() {
        let present = ImageStatus::Present {
            tag: "mysql:8.4".to_string(),
            size: Some("1.08GB".to_string()),
        };
        let json: serde_json::Value = serde_json::to_value(&present).unwrap();
        assert_eq!(json["status"], "present", "Present 的 tag 字段应为 present");
        assert_eq!(json["tag"], "mysql:8.4");
        assert_eq!(json["size"], "1.08GB");

        let missing = ImageStatus::Missing {
            tag: "redis:8.2-alpine".to_string(),
        };
        let json: serde_json::Value = serde_json::to_value(&missing).unwrap();
        assert_eq!(json["status"], "missing", "Missing 的 tag 字段应为 missing");
        assert_eq!(json["tag"], "redis:8.2-alpine");
        // Missing 不带 size —— 前端按可选字段处理，出现即类型漂移
        assert!(
            json.get("size").is_none(),
            "Missing 不应携带 size 字段，否则前端类型需同步"
        );
    }

    #[test]
    fn test_image_status_present_omits_size_when_none() {
        let present = ImageStatus::Present {
            tag: "nginx:1.28-alpine".to_string(),
            size: None,
        };
        let json: serde_json::Value = serde_json::to_value(&present).unwrap();
        assert_eq!(json["status"], "present");
        // size 为 null 时序列化为 null，前端声明 `size?: string | null` 可兼容
        assert!(json["size"].is_null(), "size=None 应序列化为 null");
    }

    #[test]
    fn test_extract_outcome_serde_contract() {
        let extracted = ExtractOutcome::Extracted {
            dest: "services/php85/php.ini".to_string(),
            bytes: 72431,
        };
        let json: serde_json::Value = serde_json::to_value(&extracted).unwrap();
        assert_eq!(json["outcome"], "extracted");
        assert_eq!(json["dest"], "services/php85/php.ini");
        assert_eq!(json["bytes"], 72431);

        let skipped = ExtractOutcome::SkippedExists;
        let json: serde_json::Value = serde_json::to_value(&skipped).unwrap();
        assert_eq!(
            json["outcome"], "skippedexists",
            "SkippedExists 序列化为 skippedexists（无下划线），前端类型依赖此字面量"
        );

        let failed = ExtractOutcome::Failed {
            reason: "docker daemon not running".to_string(),
        };
        let json: serde_json::Value = serde_json::to_value(&failed).unwrap();
        assert_eq!(json["outcome"], "failed");
        assert_eq!(json["reason"], "docker daemon not running");
    }
}
