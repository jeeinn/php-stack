//! 构建型服务（PHP / Nginx）镜像指纹
//!
//! Compose 写出 `image: php-stack/{service_dir}:{fingerprint}`，导出/导入与启动复用同一规则。
//! 指纹只纳入影响 Dockerfile build args / 上下文的字段；TZ、端口、站点卷、php.ini 不参与。

use sha2::{Digest, Sha256};
use std::path::Path;

/// 构建产物本地镜像名前缀（与 compose `name: php-stack` 项目名对齐）
pub const BUILT_IMAGE_PREFIX: &str = "php-stack";

/// PHP 构建指纹输入
#[derive(Debug, Clone)]
pub struct PhpFingerprintInput<'a> {
    pub service_dir: &'a str,
    pub base_image: &'a str,
    /// 原始扩展列表（将规范化后再哈希）
    pub extensions: &'a [String],
    pub puid: u32,
    pub pgid: u32,
    pub apt_mirror: &'a str,
    pub composer_mirror: &'a str,
    pub github_proxy: &'a str,
    pub dockerfile_bytes: &'a [u8],
}

/// Nginx 构建指纹输入
#[derive(Debug, Clone)]
pub struct NginxFingerprintInput<'a> {
    pub service_dir: &'a str,
    pub base_image: &'a str,
    pub puid: u32,
    pub pgid: u32,
    pub dockerfile_bytes: &'a [u8],
}

/// 规范化 PHP 扩展列表：trim、去空、排序、逗号拼接
pub fn normalize_extensions(extensions: &[String]) -> String {
    let mut parts: Vec<String> = extensions
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    parts.sort();
    parts.dedup();
    parts.join(",")
}

/// 计算短指纹（SHA256 前 12 位 hex）
pub fn short_fingerprint(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            hasher.update([0xff]);
        }
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    hex_encode_12(&digest)
}

fn hex_encode_12(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(12);
    for &b in bytes.iter().take(6) {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn dockerfile_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode_12(&hasher.finalize())
}

/// PHP 服务构建指纹（12 hex）
pub fn php_fingerprint(input: &PhpFingerprintInput<'_>) -> String {
    let ext = normalize_extensions(input.extensions);
    let df = dockerfile_hash(input.dockerfile_bytes);
    short_fingerprint(&[
        "php",
        input.service_dir,
        input.base_image,
        &ext,
        &input.puid.to_string(),
        &input.pgid.to_string(),
        input.apt_mirror,
        input.composer_mirror,
        input.github_proxy,
        &df,
    ])
}

/// Nginx 服务构建指纹（12 hex）
pub fn nginx_fingerprint(input: &NginxFingerprintInput<'_>) -> String {
    let df = dockerfile_hash(input.dockerfile_bytes);
    short_fingerprint(&[
        "nginx",
        input.service_dir,
        input.base_image,
        &input.puid.to_string(),
        &input.pgid.to_string(),
        &df,
    ])
}

/// 构建产物完整镜像引用：`php-stack/{service_dir}:{fingerprint}`
pub fn built_image_ref(service_dir: &str, fingerprint: &str) -> String {
    format!("{BUILT_IMAGE_PREFIX}/{service_dir}:{fingerprint}")
}

/// Compose 历史默认名（无显式 `image:` 时）：`php-stack-{service_dir}`
pub fn compose_default_image_name(service_dir: &str) -> String {
    format!("{BUILT_IMAGE_PREFIX}-{service_dir}")
}

/// 读取工作区或模板 Dockerfile 字节；都没有则返回空切片等价内容
pub fn read_dockerfile_bytes(
    project_root: &Path,
    service_dir: &str,
    locate_template: impl FnOnce(&str) -> Option<std::path::PathBuf>,
) -> Vec<u8> {
    let workspace_df = project_root
        .join("services")
        .join(service_dir)
        .join("Dockerfile");
    if workspace_df.is_file() {
        return std::fs::read(&workspace_df).unwrap_or_default();
    }
    let template_rel = format!("{service_dir}/Dockerfile");
    if let Some(path) = locate_template(&template_rel) {
        return std::fs::read(&path).unwrap_or_default();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_extensions_order_independent() {
        let a = vec!["gd".into(), "redis".into()];
        let b = vec!["redis".into(), "gd".into()];
        assert_eq!(normalize_extensions(&a), normalize_extensions(&b));
        assert_eq!(normalize_extensions(&a), "gd,redis");
    }

    #[test]
    fn test_normalize_extensions_dedupes_and_splits() {
        let raw = vec!["gd, redis".into(), "gd".into(), " curl ".into()];
        assert_eq!(normalize_extensions(&raw), "curl,gd,redis");
    }

    #[test]
    fn test_php_fingerprint_extensions_order_independent() {
        let df = b"FROM php:8.2-fpm\n";
        let a = PhpFingerprintInput {
            service_dir: "php82",
            base_image: "php:8.2-fpm",
            extensions: &["gd".into(), "redis".into()],
            puid: 1000,
            pgid: 1000,
            apt_mirror: "deb.debian.org",
            composer_mirror: "https://packagist.org",
            github_proxy: "",
            dockerfile_bytes: df,
        };
        let b = PhpFingerprintInput {
            extensions: &["redis".into(), "gd".into()],
            ..a.clone()
        };
        assert_eq!(php_fingerprint(&a), php_fingerprint(&b));
    }

    #[test]
    fn test_php_fingerprint_ignores_tz_equivalent_fields() {
        // TZ 不在输入里；同输入应稳定
        let df = b"FROM php:8.2-fpm\n";
        let input = PhpFingerprintInput {
            service_dir: "php82",
            base_image: "php:8.2-fpm",
            extensions: &["pdo_mysql".into()],
            puid: 1000,
            pgid: 1000,
            apt_mirror: "deb.debian.org",
            composer_mirror: "https://packagist.org",
            github_proxy: "",
            dockerfile_bytes: df,
        };
        let fp1 = php_fingerprint(&input);
        let fp2 = php_fingerprint(&input);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 12);
    }

    #[test]
    fn test_php_fingerprint_changes_with_extensions() {
        let df = b"FROM php:8.2-fpm\n";
        let base = PhpFingerprintInput {
            service_dir: "php82",
            base_image: "php:8.2-fpm",
            extensions: &["gd".into()],
            puid: 1000,
            pgid: 1000,
            apt_mirror: "deb.debian.org",
            composer_mirror: "https://packagist.org",
            github_proxy: "",
            dockerfile_bytes: df,
        };
        let changed = PhpFingerprintInput {
            extensions: &["gd".into(), "redis".into()],
            ..base.clone()
        };
        assert_ne!(php_fingerprint(&base), php_fingerprint(&changed));
    }

    #[test]
    fn test_php_fingerprint_changes_with_puid() {
        let df = b"FROM php:8.2-fpm\n";
        let a = PhpFingerprintInput {
            service_dir: "php82",
            base_image: "php:8.2-fpm",
            extensions: &[],
            puid: 1000,
            pgid: 1000,
            apt_mirror: "deb.debian.org",
            composer_mirror: "https://packagist.org",
            github_proxy: "",
            dockerfile_bytes: df,
        };
        let b = PhpFingerprintInput {
            puid: 1001,
            ..a.clone()
        };
        assert_ne!(php_fingerprint(&a), php_fingerprint(&b));
    }

    #[test]
    fn test_nginx_fingerprint_stable() {
        let df = b"FROM nginx:1.25\n";
        let input = NginxFingerprintInput {
            service_dir: "nginx125",
            base_image: "nginx:1.25",
            puid: 1000,
            pgid: 1000,
            dockerfile_bytes: df,
        };
        assert_eq!(nginx_fingerprint(&input).len(), 12);
        assert_eq!(
            built_image_ref("nginx125", &nginx_fingerprint(&input)),
            format!("php-stack/nginx125:{}", nginx_fingerprint(&input))
        );
    }

    #[test]
    fn test_compose_default_image_name() {
        assert_eq!(compose_default_image_name("php82"), "php-stack-php82");
    }
}
