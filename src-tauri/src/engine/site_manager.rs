//! 精简站点清单。
//!
//! 宿主机路径只写入 `.env`。`.user_sites.json` 与生成的 Nginx conf 只保留容器路径，
//! 这样备份换机器时只需重写 `.env` 里的盘符。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::env_parser::EnvFile;
use crate::app_log;

pub const SITES_FILE_NAME: &str = ".user_sites.json";
pub const MANAGED_MARKER_PREFIX: &str = "# php-stack:managed site=";
pub const PRIMARY_ENV_KEY: &str = "SOURCE_DIR";
pub const PRIMARY_CONTAINER_PATH: &str = "/www";
pub const KIND_RELATIVE: &str = "workspace-relative";
pub const KIND_ABSOLUTE: &str = "absolute";

/// 界面与 EnvConfig 使用的站点（含宿主机路径）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteEntry {
    pub id: String,
    pub server_name: String,
    /// 挂进容器的项目根（宿主机路径）。PHP 能读到其中的依赖。
    pub host_path: String,
    pub env_key: String,
    pub container_path: String,
    pub nginx_service: String,
    pub php_service: String,
    /// 相对挂载目录的对外子目录，如 `public`、`www`。空表示挂载目录本身就是网站根。
    #[serde(default)]
    pub public_dir: String,
}

/// 写入 `.user_sites.json` 的站点，不含宿主机绝对路径。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SiteRecord {
    id: String,
    server_name: String,
    env_key: String,
    container_path: String,
    nginx_service: String,
    php_service: String,
    /// 相对挂载目录，不含盘符，可随站点定义跨机器带走。
    #[serde(default)]
    public_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct UserSitesFile {
    sites: Vec<SiteRecord>,
}

/// 备份清单里的站点快照（含当时的宿主机路径，供恢复时重映射）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestSite {
    pub id: String,
    pub env_key: String,
    pub container_path: String,
    pub host_path: String,
    pub kind: String,
    pub server_name: String,
    /// 相对挂载目录的对外子目录。旧备份没有该字段时为空。
    #[serde(default)]
    pub public_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountRole {
    Php,
    Nginx,
}

/// 把反斜杠收成正斜杠，避免 Compose 把 `\` 当成转义。
pub fn normalize_host_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

/// Windows 盘符、UNC 与 Unix 绝对路径。不使用 `Path::is_absolute`，
/// 这样在 Linux 上也能认出备份里的 `D:/code`。
pub fn is_absolute_host_path(path: &str) -> bool {
    let path = normalize_host_path(path);
    if path.starts_with("//") || path.starts_with('/') {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn path_kind(host_path: &str) -> &'static str {
    if is_absolute_host_path(host_path) {
        KIND_ABSOLUTE
    } else {
        KIND_RELATIVE
    }
}

pub fn sanitize_site_id(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "main".to_string()
    } else {
        cleaned
    }
}

/// 第一条站点固定 `SOURCE_DIR` → `/www`，其余为 `SITE_{ID}` → `/sites/{id}`。
pub fn normalize_sites(sites: &mut [SiteEntry]) {
    for (index, site) in sites.iter_mut().enumerate() {
        site.id = sanitize_site_id(&site.id);
        site.host_path = repair_host_path(&site.host_path);
        site.public_dir = normalize_public_dir(&site.public_dir);
        site.server_name = site.server_name.trim().to_string();
        if index == 0 {
            site.env_key = PRIMARY_ENV_KEY.to_string();
            site.container_path = PRIMARY_CONTAINER_PATH.to_string();
        } else {
            let slug: String = site
                .id
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_uppercase();
            let slug = if slug.is_empty() {
                format!("SITE{index}")
            } else {
                slug
            };
            site.env_key = format!("SITE_{slug}");
            site.container_path = format!("/sites/{}", site.id);
        }
    }
}

pub fn validate_sites(
    sites: &[SiteEntry],
    php_dirs: &[String],
    nginx_dirs: &[String],
) -> Result<(), String> {
    if sites.is_empty() {
        return Ok(());
    }
    if nginx_dirs.is_empty() {
        return Err("sites require at least one Nginx service".to_string());
    }
    if php_dirs.is_empty() {
        return Err("sites require at least one PHP service".to_string());
    }

    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_keys = std::collections::HashSet::new();
    for site in sites {
        if !seen_ids.insert(site.id.clone()) {
            return Err(format!("duplicate site id: {}", site.id));
        }
        if !seen_keys.insert(site.env_key.clone()) {
            return Err(format!("duplicate site env key: {}", site.env_key));
        }
        if site.server_name.is_empty()
            || site
                .server_name
                .chars()
                .any(|c| c.is_whitespace() || c == ';' || c == '{' || c == '}')
        {
            return Err(format!(
                "invalid server_name for site {}: {}",
                site.id, site.server_name
            ));
        }
        if site.host_path.is_empty() {
            return Err(format!("site {} is missing a mount directory", site.id));
        }
        validate_public_dir(&site.id, &site.public_dir)?;
        if !nginx_dirs.iter().any(|dir| dir == &site.nginx_service) {
            return Err(format!(
                "site {} nginx service {} is not in the current config",
                site.id, site.nginx_service
            ));
        }
        if !php_dirs.iter().any(|dir| dir == &site.php_service) {
            return Err(format!(
                "site {} php service {} is not in the current config",
                site.id, site.php_service
            ));
        }
        if !is_safe_service_dir(&site.php_service) || !is_safe_service_dir(&site.nginx_service) {
            return Err(format!("site {} has an invalid service name", site.id));
        }
    }
    Ok(())
}

fn is_safe_service_dir(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn render_site_conf(site: &SiteEntry) -> String {
    format!(
        "{prefix}{id}\nserver {{\n    listen 80;\n    server_name {server};\n\n    root {root};\n    index index.php index.html;\n\n    location / {{\n        try_files $uri $uri/ /index.php?$query_string;\n    }}\n\n    location ~ \\.php$ {{\n        fastcgi_pass ps-{php}:9000;\n        fastcgi_index index.php;\n        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n        include fastcgi_params;\n    }}\n}}\n",
        prefix = MANAGED_MARKER_PREFIX,
        id = site.id,
        server = site.server_name,
        root = nginx_root(site),
        php = site.php_service,
    )
}

pub fn managed_site_id(content: &str) -> Option<String> {
    let line = content.lines().next()?.trim();
    let id = line.strip_prefix(MANAGED_MARKER_PREFIX)?.trim();
    if id.is_empty() || !is_safe_service_dir(id) {
        None
    } else {
        Some(id.to_string())
    }
}

/// 把 `./E:/...` 这种误加的相对前缀去掉。真正的 `./www` 不受影响。
pub fn repair_host_path(path: &str) -> String {
    let path = normalize_host_path(path);
    let trimmed = path.strip_prefix("./").unwrap_or(&path);
    if trimmed != path && is_absolute_host_path(trimmed) {
        trimmed.to_string()
    } else {
        path
    }
}

/// 对外目录只允许相对挂载目录的子路径。空、`.` 表示挂载目录本身就是网站根。
pub fn normalize_public_dir(raw: &str) -> String {
    let path = normalize_host_path(raw);
    let path = path.trim_matches('/');
    let path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty() || path == "." {
        String::new()
    } else {
        path.to_string()
    }
}

pub fn validate_public_dir(site_id: &str, public_dir: &str) -> Result<(), String> {
    let public = normalize_public_dir(public_dir);
    if public.is_empty() {
        return Ok(());
    }
    let invalid = is_absolute_host_path(&public)
        || public.contains(':')
        || public
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..");
    if invalid {
        return Err(format!(
            "site {site_id} public directory must be a relative path inside the mount, such as public or www"
        ));
    }
    Ok(())
}

/// Nginx `root`：挂载点本身，或挂载点下的对外子目录。卷始终挂在 `container_path`。
pub fn nginx_root(site: &SiteEntry) -> String {
    let base = site.container_path.trim_end_matches('/').to_string();
    let public = normalize_public_dir(&site.public_dir);
    if public.is_empty() {
        base
    } else {
        format!("{base}/{public}")
    }
}

/// 选中的目录若在工作区内，返回 `./相对路径`；跨盘或工作区外保留绝对路径（正斜杠）。
pub fn normalize_mount_against_workspace(absolute_path: &str, project_root: &Path) -> String {
    let forward = repair_host_path(absolute_path);
    let abs = PathBuf::from(absolute_path);
    match pathdiff::diff_paths(&abs, project_root) {
        Some(relative) => {
            let rel = relative.to_string_lossy().replace('\\', "/");
            if rel.is_empty() || rel == "." {
                "./".to_string()
            } else if rel == ".." || rel.starts_with("../") || is_absolute_host_path(&rel) {
                // Windows 上 pathdiff 对其他盘符会返回已是绝对路径的 Some，不能再加 `./`。
                forward
            } else {
                format!("./{rel}")
            }
        }
        None => forward,
    }
}

/// 把用户点选的对外目录收成相对挂载目录的子路径。同一目录返回空字符串。
pub fn public_subdir(mount_path: &str, selected_public: &str) -> Result<String, String> {
    let mount = PathBuf::from(mount_path);
    let public = PathBuf::from(selected_public);
    let mount_norm = normalize_host_path(mount_path)
        .trim_end_matches('/')
        .to_string();
    let public_norm = normalize_host_path(selected_public)
        .trim_end_matches('/')
        .to_string();
    if public_norm == mount_norm {
        return Ok(String::new());
    }
    match pathdiff::diff_paths(&public, &mount) {
        Some(rel) => {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel.is_empty() || rel == "." {
                return Ok(String::new());
            }
            if rel == ".." || rel.starts_with("../") || is_absolute_host_path(&rel) {
                return Err("public directory must be inside the mount directory".to_string());
            }
            validate_public_dir("site", &rel)?;
            Ok(normalize_public_dir(&rel))
        }
        None => Err("public directory must be inside the mount directory".to_string()),
    }
}

pub fn resolve_host_path(project_root: &Path, host_path: &str) -> PathBuf {
    if is_absolute_host_path(host_path) {
        PathBuf::from(host_path)
    } else {
        let rel = normalize_host_path(host_path);
        let rel = rel.trim_start_matches("./");
        project_root.join(rel)
    }
}

pub fn source_volume_lines(sites: &[SiteEntry], role: MountRole, service_dir: &str) -> Vec<String> {
    if sites.is_empty() {
        return vec!["      - ${SOURCE_DIR}:/www/:rw".to_string()];
    }
    sites
        .iter()
        .filter(|site| match role {
            MountRole::Php => site.php_service == service_dir,
            MountRole::Nginx => site.nginx_service == service_dir,
        })
        .map(|site| {
            let container = if site.container_path.ends_with('/') {
                site.container_path.clone()
            } else {
                format!("{}/", site.container_path)
            };
            format!("      - ${{{}}}:{container}:rw", site.env_key)
        })
        .collect()
}

pub fn load_sites_with_hosts(project_root: &Path, env: &EnvFile) -> Vec<SiteEntry> {
    let Ok(records) = read_records(project_root) else {
        return Vec::new();
    };
    records
        .into_iter()
        .map(|record| {
            let host_path = env
                .get(&record.env_key)
                .map(normalize_host_path)
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| {
                    if record.env_key == PRIMARY_ENV_KEY {
                        "./www".to_string()
                    } else {
                        String::new()
                    }
                });
            SiteEntry {
                host_path: repair_host_path(&host_path),
                id: record.id,
                server_name: record.server_name,
                env_key: record.env_key,
                container_path: record.container_path,
                nginx_service: record.nginx_service,
                php_service: record.php_service,
                public_dir: record.public_dir,
            }
        })
        .collect()
}

pub fn save_sites(project_root: &Path, sites: &[SiteEntry]) -> Result<(), String> {
    let path = project_root.join(SITES_FILE_NAME);
    if sites.is_empty() {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("failed to remove {SITES_FILE_NAME}: {e}"))?;
        }
        return Ok(());
    }
    let file = UserSitesFile {
        sites: sites.iter().map(SiteEntry::to_record).collect(),
    };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| format!("failed to serialize {SITES_FILE_NAME}: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("failed to write {SITES_FILE_NAME}: {e}"))
}

impl SiteEntry {
    fn to_record(&self) -> SiteRecord {
        SiteRecord {
            id: self.id.clone(),
            server_name: self.server_name.clone(),
            env_key: self.env_key.clone(),
            container_path: self.container_path.clone(),
            nginx_service: self.nginx_service.clone(),
            php_service: self.php_service.clone(),
            public_dir: normalize_public_dir(&self.public_dir),
        }
    }
}

fn read_records(project_root: &Path) -> Result<Vec<SiteRecord>, String> {
    let path = project_root.join(SITES_FILE_NAME);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text =
        fs::read_to_string(&path).map_err(|e| format!("failed to read {SITES_FILE_NAME}: {e}"))?;
    let file: UserSitesFile = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse {SITES_FILE_NAME}: {e}"))?;
    Ok(file.sites)
}

/// 去掉上一版站点写入、这次不再使用的 `SITE_*` 键。`SOURCE_DIR` 由调用方重写。
pub fn remove_stale_site_keys(env: &mut EnvFile, project_root: &Path, sites: &[SiteEntry]) {
    let Ok(previous) = read_records(project_root) else {
        return;
    };
    for record in previous {
        if record.env_key == PRIMARY_ENV_KEY {
            continue;
        }
        if !sites.iter().any(|site| site.env_key == record.env_key) {
            env.remove(&record.env_key);
        }
    }
}

pub fn collect_manifest_sites(project_root: &Path) -> Vec<ManifestSite> {
    let env = read_env(project_root);
    let records = read_records(project_root).unwrap_or_default();
    if records.is_empty() {
        let host = env
            .as_ref()
            .and_then(|file| file.get(PRIMARY_ENV_KEY))
            .map(normalize_host_path)
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "./www".to_string());
        return vec![synthetic_site(&host)];
    }
    records
        .into_iter()
        .map(|record| {
            let host = env
                .as_ref()
                .and_then(|file| file.get(&record.env_key))
                .map(normalize_host_path)
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| "./www".to_string());
            ManifestSite {
                id: record.id,
                env_key: record.env_key,
                container_path: record.container_path,
                host_path: repair_host_path(&host),
                kind: path_kind(&repair_host_path(&host)).to_string(),
                server_name: record.server_name,
                public_dir: record.public_dir,
            }
        })
        .collect()
}

pub fn synthetic_site(host_path: &str) -> ManifestSite {
    let host_path = normalize_host_path(host_path);
    let host_path = if host_path.is_empty() {
        "./www".to_string()
    } else {
        host_path
    };
    ManifestSite {
        id: "main".to_string(),
        env_key: PRIMARY_ENV_KEY.to_string(),
        container_path: PRIMARY_CONTAINER_PATH.to_string(),
        host_path: repair_host_path(&host_path),
        kind: path_kind(&repair_host_path(&host_path)).to_string(),
        server_name: "localhost".to_string(),
        public_dir: String::new(),
    }
}

fn read_env(project_root: &Path) -> Option<EnvFile> {
    let text = fs::read_to_string(project_root.join(".env")).ok()?;
    EnvFile::parse(&text).ok()
}

/// 写入托管 conf，并删除已不在清单中的托管文件。无标记的用户 conf 保持原样。
pub fn sync_managed_confs(project_root: &Path, sites: &[SiteEntry]) -> Result<(), String> {
    remove_stale_managed_confs(project_root, sites)?;
    for site in sites {
        write_managed_conf(project_root, site)?;
    }
    Ok(())
}

fn write_managed_conf(project_root: &Path, site: &SiteEntry) -> Result<(), String> {
    if !is_safe_service_dir(&site.nginx_service) || !is_safe_service_dir(&site.id) {
        return Err(format!("refusing to write conf for site {}", site.id));
    }
    let dir = project_root
        .join("services")
        .join(&site.nginx_service)
        .join("conf.d");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    let dest = dir.join(format!("{}.conf", site.id));
    if dest.exists() {
        let existing = fs::read_to_string(&dest)
            .map_err(|e| format!("failed to read {}: {e}", dest.display()))?;
        match managed_site_id(&existing) {
            Some(id) if id == site.id => {}
            Some(_) => {
                app_log!(
                    info,
                    "engine::site",
                    "Skip conf owned by another site: {}",
                    dest.display()
                );
                return Ok(());
            }
            None => {
                app_log!(
                    info,
                    "engine::site",
                    "Skip user conf without managed marker: {}",
                    dest.display()
                );
                return Ok(());
            }
        }
    }
    let body = render_site_conf(site);
    fs::write(&dest, body).map_err(|e| format!("failed to write {}: {e}", dest.display()))
}

fn remove_stale_managed_confs(project_root: &Path, sites: &[SiteEntry]) -> Result<(), String> {
    let services = project_root.join("services");
    if !services.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&services).map_err(|e| format!("failed to read services/: {e}"))? {
        let entry = entry.map_err(|e| format!("failed to read services entry: {e}"))?;
        let conf_dir = entry.path().join("conf.d");
        if !conf_dir.is_dir() {
            continue;
        }
        let nginx_dir = entry.file_name().to_string_lossy().to_string();
        for file in fs::read_dir(&conf_dir).map_err(|e| format!("failed to read conf.d: {e}"))? {
            let file = file.map_err(|e| format!("failed to read conf.d entry: {e}"))?;
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("conf") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(id) = managed_site_id(&content) else {
                continue;
            };
            let still_here = sites
                .iter()
                .any(|site| site.id == id && site.nginx_service == nginx_dir);
            if !still_here {
                fs::remove_file(&path).map_err(|e| {
                    format!("failed to remove stale site conf {}: {e}", path.display())
                })?;
                app_log!(
                    info,
                    "engine::site",
                    "Removed stale managed conf: {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

/// 工作区内站点打进 `projects/{相对路径}`，工作区外打进 `projects/sites/{id}`。
pub fn site_zip_prefix(site: &ManifestSite) -> String {
    if site.kind == KIND_ABSOLUTE {
        format!("projects/sites/{}", sanitize_site_id(&site.id))
    } else {
        let rel = normalize_host_path(&site.host_path);
        let rel = rel.trim_start_matches("./").trim_matches('/');
        if rel.is_empty() {
            "projects/www".to_string()
        } else {
            format!("projects/{rel}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_site(id: &str, host: &str) -> SiteEntry {
        let mut sites = vec![
            SiteEntry {
                id: "main".to_string(),
                server_name: "localhost".to_string(),
                host_path: "./www".to_string(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".to_string(),
                php_service: "php82".to_string(),
                public_dir: String::new(),
            },
            SiteEntry {
                id: id.to_string(),
                server_name: "shop.test".to_string(),
                host_path: host.to_string(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".to_string(),
                php_service: "php82".to_string(),
                public_dir: String::new(),
            },
        ];
        if id == "main" {
            sites.truncate(1);
            sites[0].host_path = host.to_string();
        }
        normalize_sites(&mut sites);
        sites.into_iter().last().unwrap()
    }

    #[test]
    fn normalizes_windows_slashes_and_detects_absolute_paths() {
        assert_eq!(normalize_host_path(r"D:\code\shop"), "D:/code/shop");
        assert!(is_absolute_host_path(r"D:\code"));
        assert!(is_absolute_host_path("/mnt/data/shop"));
        assert!(!is_absolute_host_path("./www"));
        assert!(!is_absolute_host_path("www/app"));
        assert_eq!(path_kind(r"D:\code"), KIND_ABSOLUTE);
        assert_eq!(path_kind("./www"), KIND_RELATIVE);
    }

    #[test]
    fn first_site_keeps_source_dir_and_www() {
        let mut sites = vec![
            SiteEntry {
                id: "Main Site".to_string(),
                server_name: " localhost ".to_string(),
                host_path: r".\www".to_string(),
                env_key: "IGNORED".to_string(),
                container_path: "/ignored".to_string(),
                nginx_service: "nginx125".to_string(),
                php_service: "php82".to_string(),
                public_dir: String::new(),
            },
            SiteEntry {
                id: "my-shop".to_string(),
                server_name: "shop.test".to_string(),
                host_path: r"D:\code\shop".to_string(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".to_string(),
                php_service: "php82".to_string(),
                public_dir: String::new(),
            },
        ];
        normalize_sites(&mut sites);
        assert_eq!(sites[0].id, "MainSite");
        assert_eq!(sites[0].env_key, "SOURCE_DIR");
        assert_eq!(sites[0].container_path, "/www");
        assert_eq!(sites[0].host_path, "./www");
        assert_eq!(sites[1].env_key, "SITE_MYSHOP");
        assert_eq!(sites[1].container_path, "/sites/my-shop");
        assert_eq!(sites[1].host_path, "D:/code/shop");
    }

    #[test]
    fn renders_managed_conf_with_container_root() {
        let site = sample_site("shop", "D:/code/shop");
        let conf = render_site_conf(&site);
        assert!(conf.starts_with("# php-stack:managed site=shop\n"));
        assert!(conf.contains("root /sites/shop;"));
        assert!(conf.contains("fastcgi_pass ps-php82:9000;"));
        assert!(!conf.contains("D:/code"));
        assert_eq!(managed_site_id(&conf).as_deref(), Some("shop"));
        assert_eq!(managed_site_id("server { listen 80; }\n"), None);
    }

    #[test]
    fn volume_lines_follow_the_selected_php_and_nginx() {
        let mut sites = vec![
            SiteEntry {
                id: "main".into(),
                server_name: "localhost".into(),
                host_path: "./www".into(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".into(),
                php_service: "php82".into(),
                public_dir: String::new(),
            },
            SiteEntry {
                id: "shop".into(),
                server_name: "shop.test".into(),
                host_path: "D:/code/shop".into(),
                env_key: String::new(),
                container_path: String::new(),
                nginx_service: "nginx125".into(),
                php_service: "php84".into(),
                public_dir: String::new(),
            },
        ];
        normalize_sites(&mut sites);
        let php82 = source_volume_lines(&sites, MountRole::Php, "php82");
        assert_eq!(php82, vec!["      - ${SOURCE_DIR}:/www/:rw".to_string()]);
        let php84 = source_volume_lines(&sites, MountRole::Php, "php84");
        assert_eq!(
            php84,
            vec!["      - ${SITE_SHOP}:/sites/shop/:rw".to_string()]
        );
        let nginx = source_volume_lines(&sites, MountRole::Nginx, "nginx125");
        assert_eq!(nginx.len(), 2);
        assert!(source_volume_lines(&[], MountRole::Php, "php82")
            .iter()
            .any(|line| line.contains("${SOURCE_DIR}:/www/")));
    }

    #[test]
    fn absolute_sites_pack_under_projects_sites_not_drive_letters() {
        let site = ManifestSite {
            id: "shop".into(),
            env_key: "SITE_SHOP".into(),
            container_path: "/sites/shop".into(),
            host_path: "D:/code/shop".into(),
            kind: KIND_ABSOLUTE.into(),
            server_name: "shop.test".into(),
            public_dir: String::new(),
        };
        assert_eq!(site_zip_prefix(&site), "projects/sites/shop");
        let relative = ManifestSite {
            host_path: "./www".into(),
            kind: KIND_RELATIVE.into(),
            ..site
        };
        assert_eq!(site_zip_prefix(&relative), "projects/www");
    }

    #[test]
    fn user_sites_file_omits_host_path_and_conf_respects_marker() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut sites = vec![sample_site("main", "D:/code/app")];
        normalize_sites(&mut sites);
        save_sites(root, &sites).unwrap();
        let raw = fs::read_to_string(root.join(SITES_FILE_NAME)).unwrap();
        assert!(!raw.contains("D:/code"));
        assert!(raw.contains("SOURCE_DIR"));

        sync_managed_confs(root, &sites).unwrap();
        let conf_path = root.join("services/nginx125/conf.d/main.conf");
        assert!(conf_path.exists());
        assert!(fs::read_to_string(&conf_path)
            .unwrap()
            .contains("root /www;"));

        fs::write(
            root.join("services/nginx125/conf.d/custom.conf"),
            "server { listen 80; root /keep; }\n",
        )
        .unwrap();
        sync_managed_confs(root, &sites).unwrap();
        assert!(
            fs::read_to_string(root.join("services/nginx125/conf.d/custom.conf"))
                .unwrap()
                .contains("/keep")
        );

        sync_managed_confs(root, &[]).unwrap();
        save_sites(root, &[]).unwrap();
        assert!(!conf_path.exists(), "删除站点后应移除托管 conf");
        assert!(
            root.join("services/nginx125/conf.d/custom.conf").exists(),
            "无标记的用户 conf 不应删除"
        );
        assert!(!root.join(SITES_FILE_NAME).exists());
    }

    #[test]
    fn path_inside_workspace_becomes_relative() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("www").join("shop");
        fs::create_dir_all(&nested).unwrap();
        let shown = normalize_mount_against_workspace(&nested.to_string_lossy(), dir.path());
        assert_eq!(shown.replace('\\', "/"), "./www/shop");
    }

    #[test]
    fn other_drive_is_not_prefixed_with_dot_slash() {
        let dir = tempfile::tempdir().unwrap();
        let shown = normalize_mount_against_workspace(r"E:\projects\fm-cmp\www", dir.path());
        assert!(
            !shown.starts_with("./"),
            "absolute path must not be stored as {shown}"
        );
        assert!(
            is_absolute_host_path(&shown) || shown.starts_with("../"),
            "got {shown}"
        );
    }

    #[test]
    fn repairs_dot_slash_prefixed_drive() {
        assert_eq!(
            repair_host_path("./E:/projects/fm-cmp/www"),
            "E:/projects/fm-cmp/www"
        );
        assert_eq!(repair_host_path("./www"), "./www");
        let mut sites = vec![sample_site("main", "./E:/projects/fm-cmp/www")];
        normalize_sites(&mut sites);
        assert_eq!(sites[0].host_path, "E:/projects/fm-cmp/www");
    }

    #[test]
    fn public_dir_changes_nginx_root_not_the_volume() {
        let mut site = sample_site("shop", "E:/projects/fm-cmp");
        site.public_dir = "www".into();
        assert_eq!(nginx_root(&site), "/sites/shop/www");
        let conf = render_site_conf(&site);
        assert!(conf.contains("root /sites/shop/www;"));
        assert!(!conf.contains("E:/projects"));
        let lines = source_volume_lines(std::slice::from_ref(&site), MountRole::Nginx, "nginx125");
        assert_eq!(
            lines,
            vec!["      - ${SITE_SHOP}:/sites/shop/:rw".to_string()]
        );
        site.public_dir.clear();
        assert_eq!(nginx_root(&site), "/sites/shop");
    }

    #[test]
    fn public_subdir_must_stay_inside_the_mount() {
        let dir = tempfile::tempdir().unwrap();
        let mount = dir.path().join("fm-cmp");
        let public = mount.join("www");
        fs::create_dir_all(&public).unwrap();
        assert_eq!(
            public_subdir(&mount.to_string_lossy(), &public.to_string_lossy()).unwrap(),
            "www"
        );
        assert_eq!(
            public_subdir(&mount.to_string_lossy(), &mount.to_string_lossy()).unwrap(),
            ""
        );
        assert!(public_subdir(&mount.to_string_lossy(), &dir.path().to_string_lossy()).is_err());
        assert!(validate_public_dir("shop", "../secret").is_err());
        assert!(validate_public_dir("shop", "E:/public").is_err());
        assert!(validate_public_dir("shop", "public").is_ok());
    }

    #[test]
    fn old_site_record_without_public_dir_deserializes() {
        let json = r#"{"sites":[{"id":"main","server_name":"localhost","env_key":"SOURCE_DIR","container_path":"/www","nginx_service":"nginx125","php_service":"php82"}]}"#;
        let file: UserSitesFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.sites[0].public_dir, "");
    }
}
