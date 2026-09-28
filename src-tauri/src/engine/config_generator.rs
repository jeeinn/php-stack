use chrono::Local;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::FileOptions;

use super::config_extractor::{ConfigExtractor, ExtractOutcome};
use super::env_parser::EnvFile;
use super::mirror_config_manager::UserMirrorConfig;
use super::service_catalog::{
    normalize_service_kind, GeneratorKind, ServiceCatalog, ServiceDescriptor,
};
use super::site_manager::{self, SiteEntry};
use super::user_config;
use super::user_override_manager::UserOverrideManager;
use super::version_manifest::{VersionEntry, VersionManifest};
use crate::app_log;

/// 解析服务镜像 tag：override/custom → manifest → `{kind}:{version}`
pub(crate) fn resolve_service_image_tag(
    override_manager: &UserOverrideManager,
    manifest: &VersionManifest,
    kind: &str,
    version: &str,
) -> String {
    override_manager
        .get_merged_entry(kind, version)
        .or_else(|| manifest.get_entry(kind, version).cloned())
        .map(|e| e.image_tag)
        .unwrap_or_else(|| format!("{kind}:{version}"))
}

fn serialize_service_kind<S>(kind: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&normalize_service_kind(kind))
}

fn deserialize_service_kind<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Ok(normalize_service_kind(&raw))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// 服务 kind（小写，如 `php` / `mysql`）。反序列化接受 `PHP` / `MySQL` 等历史写法。
    #[serde(
        serialize_with = "serialize_service_kind",
        deserialize_with = "deserialize_service_kind"
    )]
    pub service_type: String,
    pub version: String,
    pub host_port: u16,
    pub extensions: Option<Vec<String>>, // Only for PHP
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    pub services: Vec<ServiceEntry>,
    pub timezone: String,
    pub mysql_root_password: Option<String>, // MySQL root密码（可选）
    /// 启用 Nginx 时的站点清单。空列表 = 不挂代码卷。
    #[serde(default)]
    pub sites: Vec<SiteEntry>,
}

pub struct ConfigGenerator;

/// 配置文件来源（用于日志与未来的 UI 提示）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    /// 用户已存在该文件，未做改动
    UserKept,
    /// 从内置模板目录复制
    TemplateCopied,
    /// 从官方镜像运行时提取（Phase 3 新增）
    ExtractedFromImage,
}

/// 检测宿主机当前用户的 UID/GID
///
/// - Linux: 精确检测当前用户 ID（唯一需要精确映射的平台）
/// - Windows/macOS: 返回默认值 1000，依赖 Docker Desktop 的权限转换层
fn detect_host_uid_gid() -> Result<(u32, u32), String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let uid_output = Command::new("id")
            .arg("-u")
            .output()
            .map_err(|e| format!("Failed to get UID: {}", e))?;
        let uid = String::from_utf8_lossy(&uid_output.stdout)
            .trim()
            .parse::<u32>()
            .map_err(|e| format!("Invalid UID: {}", e))?;

        let gid_output = Command::new("id")
            .arg("-g")
            .output()
            .map_err(|e| format!("Failed to get GID: {}", e))?;
        let gid = String::from_utf8_lossy(&gid_output.stdout)
            .trim()
            .parse::<u32>()
            .map_err(|e| format!("Invalid GID: {}", e))?;

        Ok((uid, gid))
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Windows/macOS 下 Docker Desktop 自动处理权限转换
        // 使用默认值保持配置一致性
        Ok((1000, 1000))
    }
}

/// Backup state enum for two-phase commit
enum BackupState {
    NothingToBackup,
    Ready {
        timestamp: String,
        items: Vec<String>,
    },
}

impl ConfigGenerator {
    fn catalog_for(project_root: Option<&Path>) -> ServiceCatalog {
        match project_root {
            Some(root) => ServiceCatalog::merged(root),
            None => ServiceCatalog::builtin(),
        }
    }

    fn service_kind(service: &ServiceEntry) -> String {
        normalize_service_kind(&service.service_type)
    }

    /// 三级查找：override/custom → manifest → 合成占位（image_tag=id）
    pub(crate) fn lookup_version_entry(
        override_manager: &UserOverrideManager,
        manifest: &VersionManifest,
        kind: &str,
        id: &str,
    ) -> VersionEntry {
        override_manager
            .get_merged_entry(kind, id)
            .unwrap_or_else(|| {
                manifest
                    .get_entry(kind, id)
                    .cloned()
                    .unwrap_or_else(|| VersionEntry {
                        display_name: id.to_string(),
                        image_tag: id.to_string(),
                        service_dir: id.to_string(),
                        default_port: 0,
                        show_port: false,
                        eol: false,
                        description: None,
                    })
            })
    }

    /// Validate config: port conflicts, unknown versions, required_min / multi_instance.
    ///
    /// `project_root` 用于合并自定义服务目录与 UserOverride；为 `None` 时仅用内置 catalog/清单。
    pub fn validate(config: &EnvConfig, project_root: Option<&Path>) -> Result<(), String> {
        let catalog = Self::catalog_for(project_root);
        let manifest = VersionManifest::new();
        let override_manager = project_root.map(UserOverrideManager::new);
        let mut port_services: HashMap<u16, Vec<String>> = HashMap::new();
        let mut kind_counts: HashMap<String, usize> = HashMap::new();
        let mut seen_service_dirs: HashSet<String> = HashSet::new();

        for service in &config.services {
            let kind = Self::service_kind(service);
            if catalog.get(&kind).is_none() {
                return Err(format!("Unknown service type: {kind}"));
            }

            if service.host_port == 0 {
                return Err(format!(
                    "host_port must be > 0 for service {}",
                    service.version
                ));
            }

            let resolved = match &override_manager {
                Some(mgr) => mgr
                    .get_merged_entry(&kind, &service.version)
                    .or_else(|| manifest.get_entry(&kind, &service.version).cloned()),
                None => manifest.get_entry(&kind, &service.version).cloned(),
            };
            let Some(entry) = resolved else {
                return Err(format!("Unknown version id: {}", service.version));
            };

            // compose 服务键与 .env 前缀都落在 service_dir 上；重复会互相覆盖
            if !seen_service_dirs.insert(entry.service_dir.clone()) {
                return Err(format!(
                    "Duplicate service_dir: {} (service version: {})",
                    entry.service_dir, service.version
                ));
            }

            *kind_counts.entry(kind.clone()).or_default() += 1;

            port_services
                .entry(service.host_port)
                .or_default()
                .push(entry.display_name);
        }

        for desc in catalog.list() {
            let count = kind_counts.get(&desc.id).copied().unwrap_or(0);
            if (count as u32) < desc.required_min {
                return Err(format!(
                    "Service '{}' requires at least {} instance(s), got {}",
                    desc.id, desc.required_min, count
                ));
            }
            if !desc.multi_instance && count > 1 {
                return Err(format!(
                    "Service '{}' does not allow multiple instances, got {}",
                    desc.id, count
                ));
            }
        }

        for (port, services) in &port_services {
            if services.len() > 1 {
                return Err(format!(
                    "Port conflict: port {} used by: {}",
                    port,
                    services.join(", ")
                ));
            }
        }

        let mut sites = config.sites.clone();
        site_manager::normalize_sites(&mut sites);
        if !sites.is_empty() {
            let php_dirs = Self::service_dirs(config, &manifest, "php");
            let nginx_dirs = Self::service_dirs(config, &manifest, "nginx");
            site_manager::validate_sites(&sites, &php_dirs, &nginx_dirs)?;
        }

        Ok(())
    }

    fn service_dirs(config: &EnvConfig, manifest: &VersionManifest, kind: &str) -> Vec<String> {
        let kind = normalize_service_kind(kind);
        config
            .services
            .iter()
            .filter(|service| Self::service_kind(service) == kind)
            .map(|service| {
                manifest
                    .get_entry(&kind, &service.version)
                    .map(|entry| entry.service_dir.clone())
                    .unwrap_or_else(|| service.version.clone())
            })
            .collect()
    }

    /// Generate .env file content from EnvConfig.
    /// If existing_env is provided, preserve user custom variables.
    /// Also merges mirror configuration from `.user-config/mirror_config.json`.
    ///
    /// Note: `ServiceEntry.version` is now a manifest ID (e.g., "php82", "mysql84").
    /// `project_root` is the user's workspace directory where `.user-config/` resides.
    pub fn generate_env(
        config: &EnvConfig,
        existing_env: Option<&EnvFile>,
        project_root: &Path,
    ) -> EnvFile {
        let mut env = if let Some(existing) = existing_env {
            existing.clone()
        } else {
            EnvFile { lines: Vec::new() }
        };

        let catalog = ServiceCatalog::merged(project_root);
        let manifest = VersionManifest::new();
        let override_manager = UserOverrideManager::new(project_root);

        // 站点宿主机路径写入 SITE_*；始终清掉历史 SOURCE_DIR。无站点则不写代码路径键。
        let mut sites = config.sites.clone();
        site_manager::normalize_sites(&mut sites);
        site_manager::remove_stale_site_keys(&mut env, project_root, &sites);
        env.remove(site_manager::LEGACY_SOURCE_DIR_KEY);
        for site in &sites {
            env.set(&site.env_key, &site.host_path);
        }
        env.set("TZ", &config.timezone);
        env.set("DATA_DIR", "./data");

        // Detect and set PUID/PGID for file permissions
        // On Linux: precise detection; On Windows/macOS: defaults to 1000
        let (uid, gid) = detect_host_uid_gid().unwrap_or((1000, 1000));
        env.set("PUID", &uid.to_string());
        env.set("PGID", &gid.to_string());

        for service in &config.services {
            let id = &service.version;
            let kind = Self::service_kind(service);
            let Some(desc) = catalog.get(&kind) else {
                app_log!(
                    warn,
                    "engine::config_generator",
                    "skip env for unknown service kind: {kind}"
                );
                continue;
            };

            let entry = Self::lookup_version_entry(&override_manager, &manifest, &kind, id);
            let env_prefix = entry.service_dir.to_uppercase();
            let service_dir = &entry.service_dir;

            match desc.generator {
                GeneratorKind::Php => {
                    env.set(&format!("{env_prefix}_VERSION"), &entry.image_tag);
                    env.set(
                        &format!("{env_prefix}_HOST_PORT"),
                        &service.host_port.to_string(),
                    );
                    if let Some(exts) = &service.extensions {
                        env.set(&format!("{env_prefix}_EXTENSIONS"), &exts.join(","));
                    }
                    env.set(
                        &format!("{env_prefix}_PHP_CONF_FILE"),
                        &format!("./services/{service_dir}/php.ini"),
                    );
                    env.set(
                        &format!("{env_prefix}_FPM_CONF_FILE"),
                        &format!("./services/{service_dir}/php-fpm.conf"),
                    );
                    env.set(
                        &format!("{env_prefix}_LOG_DIR"),
                        &format!("./logs/{service_dir}"),
                    );
                }
                GeneratorKind::Nginx => {
                    env.set(&format!("{env_prefix}_VERSION"), &entry.image_tag);
                    env.set(
                        &format!("{env_prefix}_{}", desc.host_port_env_suffix),
                        &service.host_port.to_string(),
                    );
                    env.set(
                        &format!("{env_prefix}_BUILD_CONTEXT"),
                        &format!("./services/{service_dir}"),
                    );
                    env.set(
                        &format!("{env_prefix}_CONF_FILE"),
                        &format!("./services/{service_dir}/nginx.conf"),
                    );
                    env.set(
                        &format!("{env_prefix}_CONFD_DIR"),
                        &format!("./services/{service_dir}/conf.d"),
                    );
                    env.set("NGINX_LOG_DIR", "./logs/nginx");
                }
                GeneratorKind::Image => {
                    Self::emit_image_env(&mut env, config, desc, &entry, service);
                }
            }
        }

        Self::remove_stale_managed_env_keys(&mut env, config, &override_manager, &manifest);

        // Merge mirror configuration from .user-config/mirror_config.json
        if let Ok(user_mirror_config) = UserMirrorConfig::load(project_root) {
            // APT Mirror
            if let Some(apt_cat) = user_mirror_config.get_category("apt") {
                if apt_cat.enabled && !apt_cat.source.is_empty() {
                    env.set("APT_MIRROR", &apt_cat.source);
                }
            }

            // Composer Mirror
            if let Some(composer_cat) = user_mirror_config.get_category("composer") {
                if composer_cat.enabled && !composer_cat.source.is_empty() {
                    env.set("COMPOSER_MIRROR", &composer_cat.source);
                }
            }

            // NPM Mirror
            if let Some(npm_cat) = user_mirror_config.get_category("npm") {
                if npm_cat.enabled && !npm_cat.source.is_empty() {
                    env.set("NPM_MIRROR", &npm_cat.source);
                }
            }

            // GitHub Proxy
            if let Some(github_cat) = user_mirror_config.get_category("github_proxy") {
                if github_cat.enabled && !github_cat.source.is_empty() {
                    env.set("GITHUB_PROXY", &github_cat.source);
                }
            }
        }

        env
    }

    /// 移除已不在当前 config 中的托管 .env 键（保留用户自定义键）
    ///
    /// 删除需同时满足三个条件，避免误伤用户手写变量：
    /// 1. 键以某个托管后缀结尾；
    /// 2. 前缀是**已知服务前缀**——出现在版本清单 / 用户覆盖 / 当前 config 中的
    ///    `service_dir` 大写形式；
    /// 3. 该前缀不在当前 config 的服务列表里。
    ///
    /// 只认已知前缀，是为了不把 `APP_VERSION` / `APP_LOG_DIR` 这类恰好撞上托管
    /// 后缀的用户变量当成过期键删掉。代价是：已从清单中移除的旧版本（如 `MYSQL56_*`）
    /// 会残留，但 `parse_env_to_services` 只按已知 `service_dir` 反查，残留键不会被
    /// 回读成服务，属无害噪音。
    fn remove_stale_managed_env_keys(
        env: &mut EnvFile,
        config: &EnvConfig,
        override_manager: &UserOverrideManager,
        manifest: &VersionManifest,
    ) {
        // 长后缀优先，避免 `_HOST_PORT` 误匹配 `_HTTP_HOST_PORT`
        const MANAGED_SUFFIXES: &[&str] = &[
            "_HTTP_HOST_PORT",
            "_PHP_CONF_FILE",
            "_FPM_CONF_FILE",
            "_BUILD_CONTEXT",
            "_EXTENSIONS",
            "_HOST_PORT",
            "_CONF_FILE",
            "_CONFD_DIR",
            "_DATA_DIR",
            "_LOG_DIR",
            "_VERSION",
        ];

        let mut keep_prefixes: HashSet<String> = HashSet::new();
        for service in &config.services {
            let kind = Self::service_kind(service);
            let entry =
                Self::lookup_version_entry(override_manager, manifest, &kind, &service.version);
            keep_prefixes.insert(entry.service_dir.to_uppercase());
        }

        // 已知服务前缀：清单全部版本 + 用户覆盖/自定义 + 当前 config
        let mut known_prefixes: HashSet<String> = HashSet::new();
        let mut kinds: HashSet<String> = manifest.service_kinds().into_iter().collect();
        kinds.extend(override_manager.service_kinds());
        for kind in kinds {
            for item in override_manager.list_merged_entries(&kind) {
                known_prefixes.insert(item.entry.service_dir.to_uppercase());
            }
        }
        known_prefixes.extend(keep_prefixes.iter().cloned());

        let keys: Vec<String> = env.to_map().into_keys().collect();
        for key in keys {
            for suffix in MANAGED_SUFFIXES {
                if let Some(prefix) = key.strip_suffix(suffix) {
                    if !prefix.is_empty()
                        && known_prefixes.contains(prefix)
                        && !keep_prefixes.contains(prefix)
                    {
                        env.remove(&key);
                    }
                    break;
                }
            }
        }

        let has_mysql = config
            .services
            .iter()
            .any(|s| Self::service_kind(s) == "mysql");
        if !has_mysql {
            env.remove("MYSQL_ROOT_PASSWORD");
        }
        let has_nginx = config
            .services
            .iter()
            .any(|s| Self::service_kind(s) == "nginx");
        if !has_nginx {
            env.remove("NGINX_LOG_DIR");
        }
    }

    /// 通用 Image 生成器：按 catalog volumes / extra_env 写 .env
    fn emit_image_env(
        env: &mut EnvFile,
        config: &EnvConfig,
        desc: &ServiceDescriptor,
        entry: &VersionEntry,
        service: &ServiceEntry,
    ) {
        let env_prefix = entry.service_dir.to_uppercase();
        let service_dir = &entry.service_dir;

        env.set(&format!("{env_prefix}_VERSION"), &entry.image_tag);
        env.set(
            &format!("{env_prefix}_{}", desc.host_port_env_suffix),
            &service.host_port.to_string(),
        );

        if let Some(vol) = &desc.volumes.conf {
            let conf_name = desc.conf_key_file.as_deref().unwrap_or("config");
            env.set(
                &format!("{env_prefix}_{}", vol.env_suffix),
                &format!("./services/{service_dir}/{conf_name}"),
            );
        }
        if let Some(vol) = &desc.volumes.data {
            env.set(
                &format!("{env_prefix}_{}", vol.env_suffix),
                &format!("./data/{service_dir}"),
            );
        }
        if let Some(vol) = &desc.volumes.log {
            env.set(
                &format!("{env_prefix}_{}", vol.env_suffix),
                &format!("./logs/{service_dir}"),
            );
        }

        for extra in &desc.extra_env {
            let value = match extra.from.as_str() {
                "mysql_root_password" => config
                    .mysql_root_password
                    .as_deref()
                    .or(extra.default.as_deref())
                    .unwrap_or("root"),
                _ => extra.default.as_deref().unwrap_or(""),
            };
            env.set(&extra.name, value);
        }
    }

    /// Generate docker-compose.yml content using ${VAR} interpolation.
    ///
    /// Note: `ServiceEntry.version` is now a manifest ID (e.g., "php82", "mysql80").
    /// 路由按 catalog `generator`：Php / Nginx 走专用块，Image 走通用块。
    pub fn generate_compose(config: &EnvConfig, project_root: &Path) -> String {
        let mut sites = config.sites.clone();
        site_manager::normalize_sites(&mut sites);
        let catalog = ServiceCatalog::merged(project_root);
        let override_manager = UserOverrideManager::new(project_root);
        let mut lines: Vec<String> = Vec::new();
        // Note: 'version' attribute is obsolete in modern Docker Compose, omit it
        lines.push("name: php-stack".to_string());
        lines.push("services:".to_string());

        // 单实例短主机名：仅对 catalog 声明了 short_alias 的 kind，且 count==1 时挂别名。
        let mut kind_counts: HashMap<String, usize> = HashMap::new();
        for service in &config.services {
            let kind = Self::service_kind(service);
            if catalog
                .get(&kind)
                .and_then(|d| d.short_alias.as_ref())
                .is_some()
            {
                *kind_counts.entry(kind).or_default() += 1;
            }
        }
        let mut alias_assigned: HashMap<String, bool> = HashMap::new();

        for service in &config.services {
            let id = &service.version;
            let kind = Self::service_kind(service);
            let Some(desc) = catalog.get(&kind) else {
                app_log!(
                    warn,
                    "engine::config_generator",
                    "skip compose for unknown service kind: {kind}"
                );
                continue;
            };

            let service_dir = override_manager
                .get_merged_entry(&kind, id)
                .map(|entry| entry.service_dir)
                .unwrap_or_else(|| id.clone());
            let env_prefix = service_dir.to_uppercase();

            let aliases: Vec<&str> = if let Some(alias) = desc.short_alias.as_deref() {
                let count = kind_counts.get(&kind).copied().unwrap_or(0);
                let assigned = alias_assigned.entry(kind.clone()).or_default();
                if count == 1 && !*assigned {
                    *assigned = true;
                    vec![alias]
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            match desc.generator {
                GeneratorKind::Php => {
                    lines.push(format!("  {service_dir}:"));
                    lines.push("    build:".to_string());
                    lines.push(format!("      context: ./services/{service_dir}"));
                    lines.push("      args:".to_string());
                    // Pass the full image tag to Dockerfile's PHP_BASE_IMAGE ARG
                    lines.push(format!(
                        "        PHP_BASE_IMAGE: \"${{{env_prefix}_VERSION}}\""
                    ));
                    lines.push(format!(
                        "        PHP_EXTENSIONS: \"${{{env_prefix}_EXTENSIONS}}\""
                    ));
                    lines.push("        TZ: \"${TZ}\"".to_string());
                    // 镜像源配置（Debian APT 加速，适用于所有 PHP 版本）
                    lines.push(
                        "        DEBIAN_MIRROR_DOMAIN: \"${APT_MIRROR:-deb.debian.org}\""
                            .to_string(),
                    );
                    lines.push(
                        "        COMPOSER_MIRROR: \"${COMPOSER_MIRROR:-https://packagist.org}\""
                            .to_string(),
                    );
                    lines.push("        GITHUB_PROXY: \"${GITHUB_PROXY:-}\"".to_string());
                    // File permissions configuration
                    lines.push("        PUID: \"${PUID:-1000}\"".to_string());
                    lines.push("        PGID: \"${PGID:-1000}\"".to_string());
                    lines.push(format!("    container_name: ps-{service_dir}"));
                    lines.push("    expose:".to_string());
                    lines.push(format!("      - {}", desc.container_port));
                    lines.push("    volumes:".to_string());
                    lines.extend(site_manager::source_volume_lines(
                        &sites,
                        site_manager::MountRole::Php,
                        &service_dir,
                    ));
                    lines.push(format!(
                        "      - ${{{env_prefix}_PHP_CONF_FILE}}:/usr/local/etc/php/php.ini"
                    ));
                    lines.push(format!(
                        "      - ${{{env_prefix}_FPM_CONF_FILE}}:/usr/local/etc/php-fpm.d/www.conf"
                    ));
                    lines.push(format!("      - ${{{env_prefix}_LOG_DIR}}:/var/log/php"));
                    lines.push("    restart: always".to_string());
                    // 运行时注入 TZ：覆盖 Dockerfile 构建时 bake 进镜像的 ENV，
                    // 这样改 .env 里的时区后只需 recreate，不必重建镜像。
                    lines.push("    environment:".to_string());
                    lines.push("      TZ: \"${TZ}\"".to_string());
                    // PHP 服务名本身已是 php82 等，无需额外短别名
                    Self::push_compose_network(&mut lines, &[]);
                    lines.push(String::new());
                }
                GeneratorKind::Nginx => {
                    lines.push(format!("  {service_dir}:"));
                    lines.push("    build:".to_string());
                    lines.push(format!("      context: ${{{env_prefix}_BUILD_CONTEXT}}"));
                    lines.push("      args:".to_string());
                    // Pass the full image tag to Dockerfile's NGINX_BASE_IMAGE ARG
                    lines.push(format!(
                        "        NGINX_BASE_IMAGE: \"${{{env_prefix}_VERSION}}\""
                    ));
                    // File permissions configuration
                    lines.push("        PUID: \"${PUID:-1000}\"".to_string());
                    lines.push("        PGID: \"${PGID:-1000}\"".to_string());
                    lines.push(format!("    container_name: ps-{service_dir}"));
                    lines.push("    ports:".to_string());
                    lines.push(format!(
                        "      - \"${{{env_prefix}_{}}}:{}\"",
                        desc.host_port_env_suffix, desc.container_port
                    ));
                    lines.push("    volumes:".to_string());
                    lines.extend(site_manager::source_volume_lines(
                        &sites,
                        site_manager::MountRole::Nginx,
                        &service_dir,
                    ));
                    lines.push(format!(
                        "      - ${{{env_prefix}_CONF_FILE}}:/etc/nginx/nginx.conf"
                    ));
                    lines.push(format!(
                        "      - ${{{env_prefix}_CONFD_DIR}}:/etc/nginx/conf.d"
                    ));
                    lines.push("      - ${NGINX_LOG_DIR}:/var/log/nginx".to_string());
                    lines.push("    restart: always".to_string());
                    // Nginx 官方镜像不会 bake TZ；必须运行时注入，否则 error/access 日志落 UTC
                    lines.push("    environment:".to_string());
                    lines.push("      TZ: \"${TZ}\"".to_string());
                    Self::push_compose_network(&mut lines, &aliases);
                    lines.push(String::new());
                }
                GeneratorKind::Image => {
                    Self::emit_image_compose(&mut lines, desc, &service_dir, &env_prefix, &aliases);
                }
            }
        }

        lines.push("networks:".to_string());
        lines.push("  php-stack-network:".to_string());
        lines.push("    driver: bridge".to_string());

        lines.join("\n")
    }

    /// 通用 Image 生成器：按 catalog volumes / entrypoint / compose_environment 写 compose 块
    fn emit_image_compose(
        lines: &mut Vec<String>,
        desc: &ServiceDescriptor,
        service_dir: &str,
        env_prefix: &str,
        aliases: &[&str],
    ) {
        lines.push(format!("  {service_dir}:"));
        lines.push(format!("    image: ${{{env_prefix}_VERSION}}"));
        lines.push(format!("    container_name: ps-{service_dir}"));
        lines.push("    ports:".to_string());
        lines.push(format!(
            "      - \"${{{env_prefix}_{}}}:{}\"",
            desc.host_port_env_suffix, desc.container_port
        ));

        let has_volumes = desc.volumes.conf.is_some()
            || desc.volumes.data.is_some()
            || desc.volumes.log.is_some();
        if has_volumes {
            lines.push("    volumes:".to_string());
            for vol in [&desc.volumes.conf, &desc.volumes.data, &desc.volumes.log]
                .into_iter()
                .flatten()
            {
                let mode = if vol.read_only { ":ro" } else { ":rw" };
                lines.push(format!(
                    "      - ${{{env_prefix}_{}}}:{}{}",
                    vol.env_suffix, vol.container_path, mode
                ));
            }
        }

        lines.push("    restart: always".to_string());

        if let Some(entrypoint) = &desc.entrypoint {
            let items: Vec<String> = entrypoint.iter().map(|s| format!("\"{s}\"")).collect();
            lines.push(format!("    entrypoint: [{}]", items.join(", ")));
        }

        if !desc.compose_environment.is_empty() {
            lines.push("    environment:".to_string());
            for env_name in &desc.compose_environment {
                lines.push(format!("      {env_name}: \"${{{env_name}}}\""));
            }
        }

        Self::push_compose_network(lines, aliases);
        lines.push(String::new());
    }

    /// 写入 compose 的 `networks` 段；有别名时用 map 形式挂 `aliases`（单实例短主机名）。
    fn push_compose_network(lines: &mut Vec<String>, aliases: &[&str]) {
        lines.push("    networks:".to_string());
        if aliases.is_empty() {
            lines.push("      - php-stack-network".to_string());
            return;
        }
        lines.push("      php-stack-network:".to_string());
        lines.push("        aliases:".to_string());
        for alias in aliases {
            lines.push(format!("          - {alias}"));
        }
    }

    /// 获取服务模板目录（services/ 模板所在基础目录）的候选列表。
    ///
    /// 查找顺序：
    /// 1. exe 同级目录下的 services/ —— 安装版布局：NSIS/MSI 安装器会将
    ///    tauri.conf.json 中 bundle.resources（services/**）释放到安装目录（exe 旁）；
    ///    本地 `tauri build` 产物同样会将资源复制到 src-tauri/target/<profile>/services/。
    /// 2. src-tauri/services/ —— 开发模式（exe 位于 src-tauri/target/<profile>/ 下）；
    ///    也覆盖 `--no-bundle` 构建后直接运行 target/release/app.exe 的场景。
    /// 3. src-tauri/services/（再上溯一级）—— 覆盖 cargo test 等测试二进制
    ///    （位于 src-tauri/target/<profile>/deps/ 下）的场景。
    fn template_base_candidates() -> Vec<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                // 1. 安装版 / tauri build 产物布局（exe 同级 services/）
                candidates.push(exe_dir.join("services"));
                // 2. 开发布局：exe_dir 为 src-tauri/target/<profile>/，上两级即 src-tauri/
                if let Some(src_tauri) = exe_dir.parent().and_then(|p| p.parent()) {
                    candidates.push(src_tauri.join("services"));
                }
                // 3. 测试二进制布局：exe_dir 为 src-tauri/target/<profile>/deps/
                if let Some(src_tauri) = exe_dir
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                {
                    candidates.push(src_tauri.join("services"));
                }
            }
        }
        // 4. 编译期 crate 根（src-tauri/services）。CARGO_TARGET_DIR 被重定向时
        //    按 exe 上溯找不到源码树，测试/开发仍能定位模板。
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("services"));
        candidates
    }

    /// 在所有候选目录中定位模板文件，返回第一个命中的完整路径。
    fn locate_template_file(template_name: &str) -> Result<PathBuf, String> {
        let candidates = Self::template_base_candidates();
        let searched: Vec<String> = candidates
            .iter()
            .map(|base| base.join(template_name).display().to_string())
            .collect();

        candidates
            .into_iter()
            .map(|base| base.join(template_name))
            .find(|p| p.exists())
            .ok_or_else(|| {
                format!(
                    "template not found: {template_name} (searched: {}). Check services/ in the install dir, or run from src-tauri/target/<profile>/",
                    searched.join(", ")
                )
            })
    }

    /// Copy template file from the services template directory to the project
    /// services directory (user workspace).
    ///
    /// 释放语义：模板文件在「应用配置」时释放到用户 workspace 下的 services/ 目录；
    /// **目标文件已存在时一律跳过** —— 用户对已释放配置/模板的修改不会被覆盖。
    /// 需要恢复默认模板时，删除对应文件后重新应用配置即可。
    fn copy_template_file(template_name: &str, dest_path: &Path) -> Result<(), String> {
        // 目标已存在：保留用户文件，不覆盖
        if dest_path.exists() {
            app_log!(
                info,
                "engine::config_generator",
                "Dest exists, skipping template copy: {}",
                dest_path.display()
            );
            return Ok(());
        }

        let template_path = Self::locate_template_file(template_name)?;

        // Create destination directory if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("failed to create dir: {e}"))?;
        }

        // Copy file (destination is guaranteed not to exist at this point)
        std::fs::copy(&template_path, dest_path).map_err(|e| {
            format!(
                "failed to copy {} to {}: {e}",
                template_path.display(),
                dest_path.display()
            )
        })?;

        Ok(())
    }

    /// 释放配置文件，多层降级（Phase 3）：
    /// 1. 目标已存在 → 保留用户修改（`UserKept`）
    /// 2. `primary_template_dir` 命中 → 模板复制（`TemplateCopied`）—— 精确目录场景
    /// 3. 镜像提取 → `docker create` + `cp` 提取（`ExtractedFromImage`）—— fallback 场景或精确模板失败
    /// 4. `fallback_template_dir` 命中 → 模板复制（仅 fallback 场景下用，**保底**）
    /// 5. 全部失败 → 返回 `Err`
    ///
    /// `primary_template_dir` 通常是精确目录（与 service_dir 同名），`fallback_template_dir`
    /// 是 `resolve_template_dir` 兜底出来的同服务类型主流版本目录。**fallback 场景下
    /// 优先用镜像提取拿到正确版本配置**——避免 fallback 模板（如 mysql80 的 my.cnf）
    /// 错配给冷门版本（mysql:5.6）。
    ///
    /// **不主动 rmi 镜像**——复用本地缓存，让用户后续启动时秒启。
    #[allow(clippy::too_many_arguments)]
    fn ensure_config_with_extract_fallback(
        primary_template_dir: Option<&str>,
        fallback_template_dir: Option<&str>,
        dest_dir: &Path,
        dest_filename: &str,
        service_kind: &str,
        service_dir: &str,
        image_tag: &str,
        dest_root: &Path,
    ) -> Result<ConfigSource, String> {
        let dest_path = dest_dir.join(dest_filename);

        // 1. 目标已存在：用户已改过，保留
        if dest_path.exists() {
            return Ok(ConfigSource::UserKept);
        }

        // 2. 尝试精确目录模板（如果提供）
        if let Some(tdir) = primary_template_dir {
            let tname = format!("{tdir}/{dest_filename}");
            if Self::copy_template_file(&tname, &dest_path).is_ok() {
                return Ok(ConfigSource::TemplateCopied);
            }
            app_log!(
                warn,
                "engine::config_generator",
                "Built-in template missing ({}), extracting from image {}",
                tname,
                image_tag
            );
        }

        // 3. 调用 ConfigExtractor 从官方镜像提取
        let extract_outcome =
            ConfigExtractor::extract_config(service_kind, service_dir, image_tag, dest_root);
        match extract_outcome {
            ExtractOutcome::Extracted { dest, bytes } => {
                app_log!(
                    info,
                    "engine::config_generator",
                    "Extracted config from image: {} ({} bytes)",
                    dest,
                    bytes
                );
                return Ok(ConfigSource::ExtractedFromImage);
            }
            ExtractOutcome::SkippedExists => return Ok(ConfigSource::UserKept),
            ExtractOutcome::Failed { reason } => {
                app_log!(
                    warn,
                    "engine::config_generator",
                    "Image extract failed: {}",
                    reason
                );
                // 继续走 fallback 模板
            }
        }

        // 4. fallback 模板（保底）
        if let Some(tdir) = fallback_template_dir {
            let tname = format!("{tdir}/{dest_filename}");
            if Self::copy_template_file(&tname, &dest_path).is_ok() {
                app_log!(
                    warn,
                    "engine::config_generator",
                    "Using fallback template (may not match target version): {}",
                    tname
                );
                return Ok(ConfigSource::TemplateCopied);
            }
        }

        // 5. 全部失败
        Err(format!(
            "failed to release config: no template, extract failed, fallback missing (service: {service_dir}, image: {image_tag}, file: {dest_filename})"
        ))
    }

    /// Resolve the template source directory for a given service kind.
    /// Checks if the exact `service_dir` template exists; if not, falls back to catalog default.
    /// Returns `(template_dir, is_fallback)`.
    fn resolve_template_dir(
        kind: &str,
        service_dir: &str,
        catalog: &ServiceCatalog,
    ) -> (String, bool) {
        let desc = catalog.get(kind);
        let key_file = desc
            .and_then(|d| d.conf_key_file.as_deref())
            .unwrap_or("Dockerfile");
        let fallback = desc
            .and_then(|d| d.fallback_template_dir.clone())
            .unwrap_or_else(|| service_dir.to_string());

        // Check if the exact service_dir template exists in any candidate base dir
        let exact_found = Self::template_base_candidates()
            .iter()
            .any(|base| base.join(service_dir).join(key_file).exists());
        if exact_found {
            return (service_dir.to_string(), false);
        }

        (fallback, true)
    }

    /// Create services/, data/, logs/ directory structure.
    ///
    /// Note: `ServiceEntry.version` is now a manifest ID (e.g., "php82", "mysql80").
    /// Template selection is based on checking if the exact `service_dir` template directory
    /// exists; if not, catalog `fallback_template_dir` is used.
    pub fn generate_service_dirs(config: &EnvConfig, root: &Path) -> Result<(), String> {
        // Create top-level directories
        std::fs::create_dir_all(root.join("services"))
            .map_err(|e| format!("failed to create services/ dir: {e}"))?;
        std::fs::create_dir_all(root.join("data"))
            .map_err(|e| format!("failed to create data/ dir: {e}"))?;
        std::fs::create_dir_all(root.join("logs"))
            .map_err(|e| format!("failed to create logs/ dir: {e}"))?;

        let catalog = ServiceCatalog::merged(root);
        let override_manager = UserOverrideManager::new(root);

        for service in &config.services {
            let id = &service.version;
            let kind = Self::service_kind(service);
            let Some(desc) = catalog.get(&kind) else {
                app_log!(
                    warn,
                    "engine::config_generator",
                    "skip service dirs for unknown service kind: {kind}"
                );
                continue;
            };

            let merged = override_manager.get_merged_entry(&kind, id);
            let service_dir_name = merged
                .as_ref()
                .map(|e| e.service_dir.clone())
                .unwrap_or_else(|| id.clone());
            let image_tag = merged
                .as_ref()
                .map(|e| e.image_tag.clone())
                .unwrap_or_else(|| format!("{kind}:{id}"));

            let (template_dir, is_fallback) =
                Self::resolve_template_dir(&kind, &service_dir_name, &catalog);
            if is_fallback {
                app_log!(
                    info,
                    "engine::config_generator",
                    "No template dir services/{}, using {}",
                    service_dir_name,
                    template_dir
                );
            }

            // 精确/fallback 模板目录分配（Phase 3）：
            // - 精确目录：primary = Some(精确目录), fallback = None（避免误用其他版本模板）
            // - fallback 目录：primary = None（直接走镜像提取）, fallback = Some(fallback 目录) 作保底
            let (primary_tpl_dir, fallback_tpl_dir): (Option<&str>, Option<&str>) = if is_fallback {
                (None, Some(template_dir.as_str()))
            } else {
                (Some(template_dir.as_str()), None)
            };

            match desc.generator {
                GeneratorKind::Php => {
                    let service_dir = root.join(format!("services/{service_dir_name}"));
                    std::fs::create_dir_all(&service_dir).map_err(|e| {
                        format!("failed to create services/{service_dir_name}/ dir: {e}")
                    })?;

                    // Copy Dockerfile from template (项目自研，不从镜像提取)
                    Self::copy_template_file(
                        &format!("{template_dir}/Dockerfile"),
                        &service_dir.join("Dockerfile"),
                    )?;

                    // Copy php.ini via multi-layer fallback (Phase 3: 内置模板 → 镜像提取)
                    Self::ensure_config_with_extract_fallback(
                        primary_tpl_dir,
                        fallback_tpl_dir,
                        &service_dir,
                        "php.ini",
                        &kind,
                        &service_dir_name,
                        &image_tag,
                        root,
                    )?;

                    // Copy php-fpm.conf from template (项目自研，不从镜像提取)
                    Self::copy_template_file(
                        &format!("{template_dir}/php-fpm.conf"),
                        &service_dir.join("php-fpm.conf"),
                    )?;

                    // Create log directory
                    std::fs::create_dir_all(root.join(format!("logs/{service_dir_name}")))
                        .map_err(|e| {
                            format!("failed to create logs/{service_dir_name}/ dir: {e}")
                        })?;
                }
                GeneratorKind::Nginx => {
                    let service_dir = root.join(format!("services/{service_dir_name}"));
                    std::fs::create_dir_all(&service_dir).map_err(|e| {
                        format!("failed to create services/{service_dir_name}/ dir: {e}")
                    })?;
                    std::fs::create_dir_all(
                        root.join(format!("services/{service_dir_name}/conf.d")),
                    )
                    .map_err(|e| {
                        format!("failed to create services/{service_dir_name}/conf.d/ dir: {e}")
                    })?;

                    // Copy Dockerfile from template (项目自研，不从镜像提取)
                    Self::copy_template_file(
                        &format!("{template_dir}/Dockerfile"),
                        &service_dir.join("Dockerfile"),
                    )?;

                    // Copy nginx.conf via multi-layer fallback (Phase 3: 内置模板 → 镜像提取)
                    Self::ensure_config_with_extract_fallback(
                        primary_tpl_dir,
                        fallback_tpl_dir,
                        &service_dir,
                        "nginx.conf",
                        &kind,
                        &service_dir_name,
                        &image_tag,
                        root,
                    )?;

                    // Copy default.conf from template (项目自研，不从镜像提取)
                    Self::copy_template_file(
                        &format!("{template_dir}/conf.d/default.conf"),
                        &service_dir.join("conf.d/default.conf"),
                    )?;

                    // Create log directory
                    std::fs::create_dir_all(root.join("logs/nginx"))
                        .map_err(|e| format!("failed to create logs/nginx/ dir: {e}"))?;
                }
                GeneratorKind::Image => {
                    // 按 volumes 创建 conf/data/log 目录
                    if desc.volumes.conf.is_some() || desc.conf_key_file.is_some() {
                        std::fs::create_dir_all(root.join(format!("services/{service_dir_name}")))
                            .map_err(|e| {
                                format!("failed to create services/{service_dir_name}/ dir: {e}")
                            })?;
                    }
                    if desc.volumes.data.is_some() {
                        std::fs::create_dir_all(root.join(format!("data/{service_dir_name}")))
                            .map_err(|e| {
                                format!("failed to create data/{service_dir_name}/ dir: {e}")
                            })?;
                    }
                    if desc.volumes.log.is_some() {
                        std::fs::create_dir_all(root.join(format!("logs/{service_dir_name}")))
                            .map_err(|e| {
                                format!("failed to create logs/{service_dir_name}/ dir: {e}")
                            })?;
                    }

                    // 有 conf_key_file 且 (extract 或 volumes.conf) 时释放配置；否则跳过
                    if let Some(conf_file) = &desc.conf_key_file {
                        if desc.extract.is_some() || desc.volumes.conf.is_some() {
                            let service_dir = root.join(format!("services/{service_dir_name}"));
                            std::fs::create_dir_all(&service_dir).map_err(|e| {
                                format!("failed to create services/{service_dir_name}/ dir: {e}")
                            })?;
                            Self::ensure_config_with_extract_fallback(
                                primary_tpl_dir,
                                fallback_tpl_dir,
                                &service_dir,
                                conf_file,
                                &kind,
                                &service_dir_name,
                                &image_tag,
                                root,
                            )?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Phase 1: Pre-check backup feasibility
    /// Returns BackupState or error if pre-check fails
    fn precheck_backup(project_root: &Path) -> Result<BackupState, String> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

        // List of files/directories to backup
        let items_to_backup = vec![".env", "docker-compose.yml", "services"];
        let mut existing_items = Vec::new();

        // Check which items exist
        for item in &items_to_backup {
            let path = project_root.join(item);
            if path.exists() {
                existing_items.push((*item).to_string());
            }
        }

        if existing_items.is_empty() {
            return Ok(BackupState::NothingToBackup);
        }

        // Pre-check: verify backup zip file doesn't exist (avoid overwriting old backups)
        let backup_zip_name = format!("config_backup_{timestamp}.zip");
        let backup_zip_path = project_root.join(&backup_zip_name);

        if backup_zip_path.exists() {
            return Err(format!(
                "backup file already exists, delete it and retry: {backup_zip_name}"
            ));
        }

        Ok(BackupState::Ready {
            timestamp,
            items: existing_items,
        })
    }

    /// Phase 2: Execute backup with atomic rollback
    /// Creates a ZIP archive containing all config files
    fn execute_backup(state: BackupState, project_root: &Path) -> Result<Vec<String>, String> {
        match state {
            BackupState::NothingToBackup => Ok(vec![]),
            BackupState::Ready { timestamp, items } => {
                let backup_zip_name = format!("config_backup_{timestamp}.zip");
                let backup_zip_path = project_root.join(&backup_zip_name);

                app_log!(
                    info,
                    "engine::config_generator",
                    "Creating config backup: {}",
                    backup_zip_name
                );

                // Create ZIP file
                let file = std::fs::File::create(&backup_zip_path)
                    .map_err(|e| format!("failed to create backup file: {e}"))?;
                let mut zip = zip::ZipWriter::new(file);
                let zip_options = FileOptions::<()>::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                let mut backed_up_count = 0;

                // Add each item to the ZIP
                for item in &items {
                    let item_path = project_root.join(item);

                    if item_path.is_file() {
                        // Add single file
                        match std::fs::read(&item_path) {
                            Ok(content) => {
                                zip.start_file(item, zip_options)
                                    .map_err(|e| format!("failed to add file to zip: {e}"))?;
                                zip.write_all(&content)
                                    .map_err(|e| format!("failed to write zip entry: {e}"))?;
                                app_log!(
                                    info,
                                    "engine::config_generator",
                                    "Added to backup: {}",
                                    item
                                );
                                backed_up_count += 1;
                            }
                            Err(e) => {
                                app_log!(
                                    error,
                                    "engine::config_generator",
                                    "Failed to read {}: {}",
                                    item,
                                    e
                                );
                                // Continue with other files, don't fail entire backup
                            }
                        }
                    } else if item_path.is_dir() {
                        // Add directory recursively
                        match Self::add_dir_to_zip_recursive(
                            &mut zip,
                            &item_path,
                            item,
                            zip_options,
                        ) {
                            Ok(count) => {
                                app_log!(
                                    info,
                                    "engine::config_generator",
                                    "Added dir {} ({} files)",
                                    item,
                                    count
                                );
                                backed_up_count += count;
                            }
                            Err(e) => {
                                app_log!(
                                    error,
                                    "engine::config_generator",
                                    "Failed to add dir {}: {}",
                                    item,
                                    e
                                );
                                // Continue with other items
                            }
                        }
                    }
                }

                // Add user custom configuration files (same as backup_engine.rs)
                let user_config_files = [
                    user_config::MIRROR_CONFIG,
                    user_config::VERSION_OVERRIDES,
                    user_config::SITES,
                ];

                for file_name in user_config_files {
                    let config_path = user_config::path(project_root, file_name);
                    let zip_name = user_config::relative(file_name);
                    if config_path.exists() {
                        match std::fs::read(&config_path) {
                            Ok(content) => {
                                zip.start_file(&zip_name, zip_options).map_err(|e| {
                                    format!("failed to add user config to zip: {e}")
                                })?;
                                zip.write_all(&content)
                                    .map_err(|e| format!("failed to write user config: {e}"))?;
                                app_log!(
                                    info,
                                    "engine::config_generator",
                                    "Added user config: {}",
                                    zip_name
                                );
                                backed_up_count += 1;
                            }
                            Err(e) => {
                                app_log!(
                                    warn,
                                    "engine::config_generator",
                                    "Failed to read user config {}: {}",
                                    zip_name,
                                    e
                                );
                                // Continue with other config files
                            }
                        }
                    }
                }

                // Finish ZIP file
                zip.finish()
                    .map_err(|e| format!("failed to finish zip: {e}"))?;

                if backed_up_count == 0 {
                    // No files were successfully added, delete the empty ZIP
                    let _ = std::fs::remove_file(&backup_zip_path);
                    app_log!(
                        warn,
                        "engine::config_generator",
                        "No files backed up, removed empty zip"
                    );
                    return Ok(vec![]);
                }

                app_log!(
                    info,
                    "engine::config_generator",
                    "Backup done: {} ({} items)",
                    backup_zip_name,
                    items.len()
                );
                Ok(vec![backup_zip_name])
            }
        }
    }

    /// Recursively add directory contents to ZIP
    fn add_dir_to_zip_recursive(
        zip: &mut zip::ZipWriter<std::fs::File>,
        dir_path: &Path,
        zip_base_path: &str,
        options: FileOptions<()>,
    ) -> Result<usize, String> {
        let mut file_count = 0;

        for entry in std::fs::read_dir(dir_path).map_err(|e| format!("failed to read dir: {e}"))? {
            let entry = entry.map_err(|e| format!("failed to read dir entry: {e}"))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name() {
                    let zip_path = format!("{}/{}", zip_base_path, file_name.to_string_lossy());
                    match std::fs::read(&path) {
                        Ok(content) => {
                            zip.start_file(&zip_path, options)
                                .map_err(|e| format!("failed to add file to zip: {e}"))?;
                            zip.write_all(&content)
                                .map_err(|e| format!("failed to write zip entry: {e}"))?;
                            file_count += 1;
                        }
                        Err(e) => {
                            app_log!(
                                warn,
                                "engine::config_generator",
                                "Skipping file {:?}: {}",
                                path,
                                e
                            );
                        }
                    }
                }
            } else if path.is_dir() {
                if let Some(dir_name) = path.file_name() {
                    let sub_zip_path = format!("{}/{}", zip_base_path, dir_name.to_string_lossy());
                    let sub_count =
                        Self::add_dir_to_zip_recursive(zip, &path, &sub_zip_path, options)?;
                    file_count += sub_count;
                }
            }
        }

        Ok(file_count)
    }

    /// Backup existing configuration files by creating a ZIP archive.
    /// Format: config_backup_YYYYMMDD_HHMMSS.zip
    /// Contains: .env, docker-compose.yml, services/, .user-config/
    pub fn backup_existing_config(project_root: &Path) -> Result<Vec<String>, String> {
        app_log!(info, "engine::config_generator", "Prechecking backup...");

        // Phase 1: Pre-check
        let backup_state = Self::precheck_backup(project_root)?;

        // Phase 2: Execute with rollback
        app_log!(info, "engine::config_generator", "Running backup...");
        Self::execute_backup(backup_state, project_root)
    }

    /// Apply config: write .env, docker-compose.yml, create directories.
    /// If enable_backup is true, backup existing config files before overwriting.
    pub async fn apply(
        config: &EnvConfig,
        project_root: &Path,
        enable_backup: bool,
    ) -> Result<Vec<String>, String> {
        // Validate first
        Self::validate(config, Some(project_root))?;

        // Backup existing config if requested
        let mut backed_up_files = Vec::new();
        if enable_backup {
            backed_up_files = Self::backup_existing_config(project_root)?;
        }

        // Generate and write .env（保留已有自定义变量，并清理过期托管键）
        let env_path = project_root.join(".env");
        let existing_env = if env_path.exists() {
            match std::fs::read_to_string(&env_path) {
                Ok(content) => match EnvFile::parse(&content) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        app_log!(
                            warn,
                            "engine::config_generator",
                            "failed to parse existing .env, regenerating fresh: {e}"
                        );
                        None
                    }
                },
                Err(e) => {
                    app_log!(
                        warn,
                        "engine::config_generator",
                        "failed to read existing .env: {e}"
                    );
                    None
                }
            }
        } else {
            None
        };
        let env_file = Self::generate_env(config, existing_env.as_ref(), project_root);
        std::fs::write(&env_path, env_file.format())
            .map_err(|e| format!("failed to write .env file: {e}"))?;

        // Generate and write docker-compose.yml
        let compose = Self::generate_compose(config, project_root);
        std::fs::write(project_root.join("docker-compose.yml"), compose)
            .map_err(|e| format!("failed to write docker-compose.yml: {e}"))?;

        // Create directory structure
        Self::generate_service_dirs(config, project_root)?;

        let mut sites = config.sites.clone();
        site_manager::normalize_sites(&mut sites);
        site_manager::save_sites(project_root, &sites)?;
        site_manager::sync_managed_confs(project_root, &sites)?;

        // Generate .npmrc file in workspace path if NPM mirror is configured
        let npm_mirror = env_file.get("NPM_MIRROR").unwrap_or("");
        if !npm_mirror.is_empty() && npm_mirror != "https://registry.npmjs.org" {
            // 从 workspace.json 获取工作区路径
            let workspace_path = if let Some(workspace_config) =
                crate::engine::workspace_manager::WorkspaceManager::load_workspace()?
            {
                PathBuf::from(workspace_config.workspace_path)
            } else {
                // 如果没有配置 workspace，使用项目根目录作为后备
                project_root.to_path_buf()
            };

            let npmrc_content = format!("registry={npm_mirror}\n");
            let npmrc_path = workspace_path.join(".npmrc");
            std::fs::write(&npmrc_path, npmrc_content)
                .map_err(|e| format!("failed to write .npmrc file: {e}"))?;
        }

        Ok(backed_up_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_basic_config() -> EnvConfig {
        EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9000,
                    extensions: Some(vec!["pdo_mysql".to_string(), "gd".to_string()]),
                },
                ServiceEntry {
                    service_type: "mysql".to_string(),
                    version: "mysql80".to_string(),
                    host_port: 3306,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "redis".to_string(),
                    version: "redis70".to_string(),
                    host_port: 6379,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "nginx".to_string(),
                    version: "nginx125".to_string(),
                    host_port: 80,
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        }
    }

    #[test]
    fn test_locate_template_file_finds_php_template() {
        // 回归：模板解析候选链应能在开发/测试环境（测试二进制位于 target/<profile>/deps/）
        // 定位 src-tauri/services/ 下的模板
        let path = ConfigGenerator::locate_template_file("php85/Dockerfile")
            .expect("应能定位 php85/Dockerfile 模板");
        assert!(path.ends_with("php85/Dockerfile"), "实际路径: {path:?}");
    }

    #[test]
    fn test_copy_template_file_skips_existing_dest() {
        // 回归：目标文件已存在时必须跳过（保留用户修改），不得用模板覆盖
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let dest = dir.path().join("php.ini");
        std::fs::write(&dest, "; user customized\n").unwrap();

        ConfigGenerator::copy_template_file("php85/php.ini", &dest)
            .expect("目标已存在时 copy_template_file 应直接成功");

        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(
            content, "; user customized\n",
            "已存在的用户文件不应被模板覆盖"
        );
    }

    #[test]
    fn test_copy_template_file_copies_when_missing() {
        // 回归：目标文件不存在时应正常释放模板
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let dest = dir.path().join("Dockerfile");

        ConfigGenerator::copy_template_file("php85/Dockerfile", &dest).expect("模板释放应成功");

        assert!(dest.exists(), "目标文件应被释放创建");
        assert!(!std::fs::read_to_string(&dest).unwrap().is_empty());
    }

    #[test]
    fn test_validate_no_conflict() {
        let config = make_basic_config();
        assert!(ConfigGenerator::validate(&config, None).is_ok());
    }

    #[test]
    fn test_validate_port_conflict() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9000,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "mysql".to_string(),
                    version: "mysql80".to_string(),
                    host_port: 3306,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "redis".to_string(),
                    version: "redis70".to_string(),
                    host_port: 3306, // conflict!
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };
        let result = ConfigGenerator::validate(&config, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Port conflict"));
        assert!(err.contains("3306"));
        // display_name from manifest: "MySQL 8.0" and "Redis 7.0"
        assert!(err.contains("MySQL 8.0"));
        assert!(err.contains("Redis 7.0"));
    }

    #[test]
    fn test_generate_env_basic() {
        let config = make_basic_config();
        let temp_dir = std::env::temp_dir();
        let env = ConfigGenerator::generate_env(&config, None, &temp_dir);
        let map = env.to_map();

        assert!(!map.contains_key("SOURCE_DIR"));
        assert_eq!(map.get("TZ").unwrap(), "Asia/Shanghai");
        assert_eq!(map.get("DATA_DIR").unwrap(), "./data");
        // PHP VERSION now contains full image tag (e.g., php:8.2-fpm)
        assert_eq!(map.get("PHP82_VERSION").unwrap(), "php:8.2-fpm");
        assert_eq!(map.get("PHP82_HOST_PORT").unwrap(), "9000");
        assert_eq!(map.get("PHP82_EXTENSIONS").unwrap(), "pdo_mysql,gd");
        assert_eq!(
            map.get("PHP82_PHP_CONF_FILE").unwrap(),
            "./services/php82/php.ini"
        );
        assert_eq!(
            map.get("PHP82_FPM_CONF_FILE").unwrap(),
            "./services/php82/php-fpm.conf"
        );
        assert_eq!(map.get("PHP82_LOG_DIR").unwrap(), "./logs/php82");
        // MySQL 8.0 uses full image tag "mysql:8.0" from version_manifest.json
        assert_eq!(map.get("MYSQL80_VERSION").unwrap(), "mysql:8.0");
        assert_eq!(map.get("MYSQL80_HOST_PORT").unwrap(), "3306");
        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "root");
        // Redis 7.0 uses full image tag "redis:7.0-alpine" from version_manifest.json
        assert_eq!(map.get("REDIS70_VERSION").unwrap(), "redis:7.0-alpine");
        assert_eq!(map.get("REDIS70_HOST_PORT").unwrap(), "6379");
        // Nginx 1.25 uses full image tag "nginx:1.25-alpine" from version_manifest.json
        assert_eq!(map.get("NGINX125_VERSION").unwrap(), "nginx:1.25-alpine");
        assert_eq!(map.get("NGINX125_HTTP_HOST_PORT").unwrap(), "80");
    }

    #[test]
    fn test_generate_env_preserves_custom_vars() {
        let existing_content = "# My custom config\nCUSTOM_VAR=hello\nSOURCE_DIR=./old";
        let existing_env = EnvFile::parse(existing_content).unwrap();

        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "nginx".to_string(),
                version: "nginx125".to_string(),
                host_port: 80,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let env =
            ConfigGenerator::generate_env(&config, Some(&existing_env), &std::env::temp_dir());
        let map = env.to_map();

        // Custom variable preserved
        assert_eq!(map.get("CUSTOM_VAR").unwrap(), "hello");
        // 历史 SOURCE_DIR 应被清除（无站点 = 不写代码路径）
        assert!(!map.contains_key("SOURCE_DIR"));
        // New managed variable added (uses full image tag from version_manifest.json)
        assert_eq!(map.get("NGINX125_VERSION").unwrap(), "nginx:1.25-alpine");
    }

    #[test]
    fn test_generate_env_removes_stale_managed_keys() {
        // 已有 CUSTOM_FOO + MYSQL80_*；config 仅 PHP → 保留 CUSTOM_FOO，丢弃 MYSQL80_* 与 MYSQL_ROOT_PASSWORD
        let existing_content = "\
CUSTOM_FOO=bar
MYSQL80_VERSION=mysql:8.0
MYSQL80_HOST_PORT=3306
MYSQL80_CONF_FILE=./services/mysql80/my.cnf
MYSQL80_DATA_DIR=./data/mysql80
MYSQL80_LOG_DIR=./logs/mysql80
MYSQL_ROOT_PASSWORD=secret
SOURCE_DIR=./old
";
        let existing_env = EnvFile::parse(existing_content).unwrap();

        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "php".to_string(),
                version: "php82".to_string(),
                host_port: 9000,
                extensions: Some(vec!["pdo_mysql".to_string()]),
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let env =
            ConfigGenerator::generate_env(&config, Some(&existing_env), &std::env::temp_dir());
        let map = env.to_map();

        assert_eq!(map.get("CUSTOM_FOO").unwrap(), "bar");
        assert!(map.contains_key("PHP82_VERSION"));
        assert!(!map.contains_key("MYSQL80_VERSION"));
        assert!(!map.contains_key("MYSQL80_HOST_PORT"));
        assert!(!map.contains_key("MYSQL80_CONF_FILE"));
        assert!(!map.contains_key("MYSQL80_DATA_DIR"));
        assert!(!map.contains_key("MYSQL80_LOG_DIR"));
        assert!(!map.contains_key("MYSQL_ROOT_PASSWORD"));
        assert!(!map.contains_key("NGINX_LOG_DIR"));
        assert!(!map.contains_key("SOURCE_DIR"));
    }

    #[test]
    fn test_generate_env_keeps_user_vars_with_managed_suffixes() {
        // 用户手写变量恰好以托管后缀结尾：必须保留（非已知服务前缀）
        let existing_content = "\
APP_VERSION=1.2.3
APP_LOG_DIR=/var/app
APP_DATA_DIR=/var/data
APP_HOST_PORT=8080
CUSTOM_FOO=bar
";
        let existing_env = EnvFile::parse(existing_content).unwrap();

        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "php".to_string(),
                version: "php82".to_string(),
                host_port: 9000,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let env =
            ConfigGenerator::generate_env(&config, Some(&existing_env), &std::env::temp_dir());
        let map = env.to_map();

        assert_eq!(map.get("APP_VERSION").unwrap(), "1.2.3");
        assert_eq!(map.get("APP_LOG_DIR").unwrap(), "/var/app");
        assert_eq!(map.get("APP_DATA_DIR").unwrap(), "/var/data");
        assert_eq!(map.get("APP_HOST_PORT").unwrap(), "8080");
        assert_eq!(map.get("CUSTOM_FOO").unwrap(), "bar");
        // 当前服务的托管键照常写入
        assert!(map.contains_key("PHP82_VERSION"));
    }

    #[test]
    fn test_generate_env_removes_stale_keys_for_custom_kind() {
        // 自定义 kind 只存在于 version_overrides：其 service_dir 仍属「已知前缀」，
        // 从 config 移除后托管键要清掉；同时用户变量不受影响。
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join(".user-config");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("version_overrides.json"),
            r#"{
              "mongodb": {
                "mongodbdefault": {
                  "entry_kind": "custom",
                  "display_name": "MongoDB (mongodbdefault)",
                  "image_tag": "mongo:7",
                  "service_dir": "mongodbdefault",
                  "default_port": 27017,
                  "show_port": true,
                  "eol": false
                }
              }
            }"#,
        )
        .unwrap();

        let existing_content = "\
MONGODBDEFAULT_VERSION=mongo:7
MONGODBDEFAULT_HOST_PORT=27017
MONGODBDEFAULT_DATA_DIR=./data/mongodbdefault
APP_VERSION=9
";
        let existing_env = EnvFile::parse(existing_content).unwrap();

        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "php".to_string(),
                version: "php82".to_string(),
                host_port: 9000,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let env = ConfigGenerator::generate_env(&config, Some(&existing_env), temp.path());
        let map = env.to_map();

        assert!(!map.contains_key("MONGODBDEFAULT_VERSION"));
        assert!(!map.contains_key("MONGODBDEFAULT_HOST_PORT"));
        assert!(!map.contains_key("MONGODBDEFAULT_DATA_DIR"));
        assert_eq!(map.get("APP_VERSION").unwrap(), "9");
        assert!(map.contains_key("PHP82_VERSION"));
    }

    #[test]
    fn test_validate_rejects_unknown_version() {
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "php".to_string(),
                version: "php_does_not_exist".to_string(),
                host_port: 9000,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };
        let err = ConfigGenerator::validate(&config, None).unwrap_err();
        assert!(err.contains("Unknown version id"));
        assert!(err.contains("php_does_not_exist"));
    }

    #[test]
    fn test_validate_rejects_missing_required_php() {
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "nginx".to_string(),
                version: "nginx125".to_string(),
                host_port: 80,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };
        let err = ConfigGenerator::validate(&config, None).unwrap_err();
        assert!(err.contains("requires at least"));
        assert!(err.contains("php"));
    }

    #[test]
    fn test_validate_rejects_host_port_zero() {
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "php".to_string(),
                version: "php82".to_string(),
                host_port: 0,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };
        let err = ConfigGenerator::validate(&config, None).unwrap_err();
        assert!(err.contains("host_port must be > 0"));
    }

    #[test]
    fn test_validate_rejects_duplicate_service_dir() {
        // PHP 允许多实例，但同一 version/service_dir 重复会让 compose 键互相覆盖
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9000,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9001,
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };
        let err = ConfigGenerator::validate(&config, None).unwrap_err();
        assert!(err.contains("Duplicate service_dir"));
        assert!(err.contains("php82"));
    }

    #[test]
    fn test_resolve_service_image_tag_uses_override() {
        use super::resolve_service_image_tag;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join(".user-config");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("version_overrides.json"),
            r#"{
              "php": {
                "php82": {
                  "entry_kind": "override",
                  "image_tag": "myregistry/php:8.2-custom"
                }
              }
            }"#,
        )
        .unwrap();

        let manager = UserOverrideManager::new(temp.path());
        let manifest = VersionManifest::new();
        let tag = resolve_service_image_tag(&manager, &manifest, "php", "php82");
        assert_eq!(tag, "myregistry/php:8.2-custom");

        let fallback = resolve_service_image_tag(&manager, &manifest, "mongodb", "mongo7");
        assert_eq!(fallback, "mongodb:mongo7");
    }

    #[test]
    fn test_generate_env_multiple_php() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php74".to_string(),
                    host_port: 9074,
                    extensions: Some(vec!["pdo_mysql".to_string()]),
                },
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9082,
                    extensions: Some(vec!["gd".to_string(), "curl".to_string()]),
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let env = ConfigGenerator::generate_env(&config, None, &std::env::temp_dir());
        let map = env.to_map();

        // PHP 7.4 vars (full image tag)
        assert_eq!(map.get("PHP74_VERSION").unwrap(), "php:7.4-fpm");
        assert_eq!(map.get("PHP74_HOST_PORT").unwrap(), "9074");
        assert_eq!(map.get("PHP74_EXTENSIONS").unwrap(), "pdo_mysql");

        // PHP 8.2 vars (full image tag)
        assert_eq!(map.get("PHP82_VERSION").unwrap(), "php:8.2-fpm");
        assert_eq!(map.get("PHP82_HOST_PORT").unwrap(), "9082");
        assert_eq!(map.get("PHP82_EXTENSIONS").unwrap(), "gd,curl");
    }

    #[test]
    fn test_generate_compose_uses_interpolation() {
        let config = make_basic_config();
        let compose = ConfigGenerator::generate_compose(&config, &std::env::temp_dir());

        // Should contain ${VAR} interpolation, not hardcoded values
        assert!(compose.contains("${MYSQL80_VERSION}"));
        assert!(compose.contains("${MYSQL80_HOST_PORT}"));
        assert!(compose.contains("${REDIS70_VERSION}"));
        assert!(compose.contains("${REDIS70_HOST_PORT}"));
        assert!(compose.contains("${NGINX125_HTTP_HOST_PORT}"));
        assert!(!compose.contains("${SOURCE_DIR}"));
        assert!(!compose.contains(":/www/:rw"));
        assert!(compose.contains("${PHP82_EXTENSIONS}"));
        assert!(compose.contains("${PHP82_PHP_CONF_FILE}"));
        assert!(compose.contains("${TZ}"));

        // 各服务均应运行时注入 TZ（Nginx/Redis 此前缺失，日志会落 UTC）
        assert!(
            compose.contains("nginx125:") && compose.contains("TZ: \"${TZ}\""),
            "nginx 服务应注入运行时 TZ"
        );
        let nginx_block = compose
            .split("nginx125:")
            .nth(1)
            .and_then(|s| s.split("\nnetworks:").next())
            .unwrap_or("");
        assert!(
            nginx_block.contains("environment:") && nginx_block.contains("TZ: \"${TZ}\""),
            "nginx 应有 environment.TZ，实际段落:\n{nginx_block}"
        );
        let redis_block = compose
            .split("redis70:")
            .nth(1)
            .and_then(|s| s.split("\n  nginx").next())
            .unwrap_or("");
        assert!(
            redis_block.contains("environment:") && redis_block.contains("TZ: \"${TZ}\""),
            "redis 应有 environment.TZ，实际段落:\n{redis_block}"
        );

        // 单实例时挂短主机名别名，便于应用连 redis/mysql
        assert!(
            redis_block.contains("aliases:") && redis_block.contains("- redis"),
            "单 Redis 应有 alias redis，实际段落:\n{redis_block}"
        );
        let mysql_block = compose
            .split("mysql80:")
            .nth(1)
            .and_then(|s| s.split("\n  redis").next())
            .unwrap_or("");
        assert!(
            mysql_block.contains("aliases:") && mysql_block.contains("- mysql"),
            "单 MySQL 应有 alias mysql，实际段落:\n{mysql_block}"
        );
        assert!(
            nginx_block.contains("aliases:") && nginx_block.contains("- nginx"),
            "单 Nginx 应有 alias nginx，实际段落:\n{nginx_block}"
        );

        // Should NOT contain hardcoded values for versions/ports
        assert!(!compose.contains("image: mysql:8.0"));
        assert!(!compose.contains("\"3306:3306\""));
    }

    #[test]
    fn test_generate_compose_emits_site_volumes_without_source_dir() {
        use crate::engine::site_manager::SiteEntry;

        let mut config = make_basic_config();
        config.sites = vec![
            SiteEntry {
                id: "cmp".into(),
                server_name: "cmp.localhost".into(),
                host_path: "E:/projects/fm-cmp".into(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".into(),
                php_service: "php82".into(),
                public_dir: "www".into(),
            },
            SiteEntry {
                id: "agent".into(),
                server_name: "agent.localhost".into(),
                host_path: "E:/projects/fm-agent".into(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".into(),
                php_service: "php82".into(),
                public_dir: "www".into(),
            },
        ];

        let tmp = tempfile::tempdir().unwrap();
        let env = ConfigGenerator::generate_env(&config, None, tmp.path());
        let map = env.to_map();
        assert_eq!(
            map.get("SITE_CMP").map(String::as_str),
            Some("E:/projects/fm-cmp")
        );
        assert_eq!(
            map.get("SITE_AGENT").map(String::as_str),
            Some("E:/projects/fm-agent")
        );
        assert!(!map.contains_key("SOURCE_DIR"));

        let compose = ConfigGenerator::generate_compose(&config, tmp.path());
        assert!(compose.contains("${SITE_CMP}:/sites/cmp/:rw"));
        assert!(compose.contains("${SITE_AGENT}:/sites/agent/:rw"));
        assert!(!compose.contains("${SOURCE_DIR}"));
        assert!(!compose.contains(":/www/:rw"));
    }

    #[test]
    fn test_generate_compose_skips_short_alias_when_multi_redis() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "redis".to_string(),
                    version: "redis62".to_string(),
                    host_port: 6379,
                    extensions: None,
                },
                ServiceEntry {
                    service_type: "redis".to_string(),
                    version: "redis70".to_string(),
                    host_port: 6380,
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };
        let compose = ConfigGenerator::generate_compose(&config, &std::env::temp_dir());
        // 多 Redis 时不能抢同一个 DNS 名 redis
        assert!(
            !compose.contains("- redis\n") && !compose.contains("- redis\r\n"),
            "多 Redis 时不应生成 alias redis，实际:\n{compose}"
        );
        assert!(compose.contains("  redis62:"));
        assert!(compose.contains("  redis70:"));
    }

    #[test]
    fn test_generate_compose_multiple_php() {
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php74".to_string(),
                    host_port: 9074,
                    extensions: Some(vec!["pdo_mysql".to_string()]),
                },
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9082,
                    extensions: Some(vec!["gd".to_string()]),
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let compose = ConfigGenerator::generate_compose(&config, &std::env::temp_dir());

        // Should have 2 PHP service definitions
        assert!(compose.contains("container_name: ps-php74"));
        assert!(compose.contains("container_name: ps-php82"));

        // Each should have its own service block
        assert!(compose.contains("  php74:"));
        assert!(compose.contains("  php82:"));

        // Each should reference its own variables
        assert!(compose.contains("${PHP74_EXTENSIONS}"));
        assert!(compose.contains("${PHP82_EXTENSIONS}"));
    }

    #[test]
    fn test_generate_env_mysql_root_password() {
        // 测试自定义MySQL root密码
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "mysql".to_string(),
                version: "mysql80".to_string(),
                host_port: 3306,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: Some("mypassword123".to_string()),
            sites: vec![],
        };

        let env = ConfigGenerator::generate_env(&config, None, &std::env::temp_dir());
        let map = env.to_map();

        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "mypassword123");
    }

    #[test]
    fn test_generate_env_mysql_default_password() {
        // 测试默认MySQL root密码（未设置时）
        let config = EnvConfig {
            services: vec![ServiceEntry {
                service_type: "mysql".to_string(),
                version: "mysql80".to_string(),
                host_port: 3306,
                extensions: None,
            }],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: None,
            sites: vec![],
        };

        let env = ConfigGenerator::generate_env(&config, None, &std::env::temp_dir());
        let map = env.to_map();

        assert_eq!(map.get("MYSQL_ROOT_PASSWORD").unwrap(), "root");
    }

    #[test]
    fn test_service_entry_serde_accepts_legacy_pascal_case() {
        let json = r#"{
            "service_type": "PHP",
            "version": "php82",
            "host_port": 9000,
            "extensions": null
        }"#;
        let entry: ServiceEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.service_type, "php");

        let json_mysql = r#"{
            "service_type": "MySQL",
            "version": "mysql80",
            "host_port": 3306
        }"#;
        let entry: ServiceEntry = serde_json::from_str(json_mysql).unwrap();
        assert_eq!(entry.service_type, "mysql");
    }

    #[test]
    fn test_generate_env_php_nginx_only_omits_mysql() {
        // 仅 PHP + Nginx 时不应写出 MYSQL_* / mysql 服务相关键
        let config = EnvConfig {
            services: vec![
                ServiceEntry {
                    service_type: "php".to_string(),
                    version: "php82".to_string(),
                    host_port: 9000,
                    extensions: Some(vec!["pdo_mysql".to_string()]),
                },
                ServiceEntry {
                    service_type: "nginx".to_string(),
                    version: "nginx125".to_string(),
                    host_port: 80,
                    extensions: None,
                },
            ],
            timezone: "Asia/Shanghai".to_string(),
            mysql_root_password: Some("should-not-appear".to_string()),
            sites: vec![],
        };

        let env = ConfigGenerator::generate_env(&config, None, &std::env::temp_dir());
        let map = env.to_map();
        let compose = ConfigGenerator::generate_compose(&config, &std::env::temp_dir());

        assert!(
            !map.keys().any(|k| k.starts_with("MYSQL")),
            "env 不应含 MYSQL_*: {map:?}"
        );
        assert!(!map.contains_key("MYSQL_ROOT_PASSWORD"));
        assert!(
            !compose.contains("mysql"),
            "compose 不应含 mysql 服务:\n{compose}"
        );
        assert!(compose.contains("  php82:"));
        assert!(compose.contains("  nginx125:"));
    }
}
