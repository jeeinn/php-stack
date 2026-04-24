/// 统一镜像源管理器
///
/// 在 mirror_config.rs 基础上，增加预设方案管理、连接测试和统一 .env 集成。
/// 通过 env_parser.rs 读写 .env 文件中的镜像源配置。

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::env_parser::EnvFile;
use super::mirror_config::MirrorSource;

/// 镜像源预设方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorPreset {
    pub name: String,
    pub docker_registry: String,
    pub apt: MirrorSource,
    pub composer: MirrorSource,
    pub npm: String,
}

/// 当前镜像源状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorStatus {
    pub docker_registry: String,
    pub apt: String,
    pub composer: String,
    pub npm: String,
    pub github: String,
}

pub struct MirrorManager;

impl MirrorManager {
    /// 获取所有预设镜像源配置方案
    ///
    /// 返回 5 个预设：阿里云、清华大学、腾讯云、中科大、官方默认
    pub fn get_presets() -> Vec<MirrorPreset> {
        vec![
            MirrorPreset {
                name: "阿里云全套".to_string(),
                docker_registry: "https://registry.cn-hangzhou.aliyuncs.com".to_string(),
                apt: MirrorSource::Aliyun,
                composer: MirrorSource::Aliyun,
                npm: "https://registry.npmmirror.com".to_string(),
            },
            MirrorPreset {
                name: "清华大学全套".to_string(),
                docker_registry: "https://docker.mirrors.tuna.tsinghua.edu.cn".to_string(),
                apt: MirrorSource::Tsinghua,
                composer: MirrorSource::Default,
                npm: "https://registry.npmmirror.com".to_string(),
            },
            MirrorPreset {
                name: "腾讯云全套".to_string(),
                docker_registry: "https://mirror.ccs.tencentyun.com".to_string(),
                apt: MirrorSource::Default,
                composer: MirrorSource::Tencent,
                npm: "https://registry.npmmirror.com".to_string(),
            },
            MirrorPreset {
                name: "中科大全套".to_string(),
                docker_registry: "https://docker.mirrors.ustc.edu.cn".to_string(),
                apt: MirrorSource::Ustc,
                composer: MirrorSource::Default,
                npm: "https://registry.npmmirror.com".to_string(),
            },
            MirrorPreset {
                name: "官方默认".to_string(),
                docker_registry: String::new(),
                apt: MirrorSource::Default,
                composer: MirrorSource::Default,
                npm: "https://registry.npmjs.org".to_string(),
            },
        ]
    }

    /// 按预设名称应用镜像源配置
    ///
    /// 读取 .env 文件，设置 4 个镜像源键，写回文件。
    pub fn apply_preset(preset_name: &str, env_path: &Path) -> Result<(), String> {
        let presets = Self::get_presets();
        let preset = presets
            .iter()
            .find(|p| p.name == preset_name)
            .ok_or_else(|| format!("未找到预设方案: {preset_name}"))?;

        let content = if env_path.exists() {
            std::fs::read_to_string(env_path)
                .map_err(|e| format!("读取 .env 文件失败: {e}"))?
        } else {
            String::new()
        };

        let mut env_file = EnvFile::parse(&content)
            .map_err(|e| format!("解析 .env 文件失败: {e}"))?;

        env_file.set("DOCKER_REGISTRY_MIRROR", &preset.docker_registry);
        env_file.set("APT_MIRROR", preset.apt.as_str());
        env_file.set("COMPOSER_MIRROR", preset.composer.as_str());
        env_file.set("NPM_MIRROR", &preset.npm);

        let output = env_file.format();
        std::fs::write(env_path, output)
            .map_err(|e| format!("写入 .env 文件失败: {e}"))?;

        Ok(())
    }

    /// 独立更新单个镜像源类别
    ///
    /// category: "docker", "apt", "composer", "npm", "github"
    /// 只更新指定类别的键，其他类别保持不变。
    pub fn update_single(
        category: &str,
        value: &str,
        env_path: &Path,
    ) -> Result<(), String> {
        let key = match category {
            "docker_registry" => "DOCKER_REGISTRY_MIRROR",
            "apt" => "APT_MIRROR",
            "composer" => "COMPOSER_MIRROR",
            "npm" => "NPM_MIRROR",
            "github_proxy" => "GITHUB_PROXY",
            _ => return Err(format!("未知的镜像源类别: {category}")),
        };

        let content = if env_path.exists() {
            std::fs::read_to_string(env_path)
                .map_err(|e| format!("读取 .env 文件失败: {e}"))?
        } else {
            String::new()
        };

        let mut env_file = EnvFile::parse(&content)
            .map_err(|e| format!("解析 .env 文件失败: {e}"))?;

        env_file.set(key, value);

        let output = env_file.format();
        std::fs::write(env_path, output)
            .map_err(|e| format!("写入 .env 文件失败: {e}"))?;

        Ok(())
    }

    /// 测试镜像源连接（3 秒超时）
    ///
    /// 向指定 URL 发送 HEAD 请求，3 秒内返回结果。
    /// 返回 Ok(true) 表示可达，Ok(false) 表示不可达，Err 表示其他错误。
    pub async fn test_connection(url: &str) -> Result<bool, String> {
        if url.is_empty() {
            return Ok(false);
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

        match client.head(url).send().await {
            Ok(response) => Ok(response.status().is_success() || response.status().is_redirection()),
            Err(e) => {
                if e.is_timeout() || e.is_connect() {
                    Ok(false)
                } else {
                    Err(format!("连接测试失败: {e}"))
                }
            }
        }
    }

    /// 获取当前镜像源状态
    ///
    /// 从 .env 文件中读取 4 个镜像源键的值。
    pub fn get_current_status(env_path: &Path) -> Result<MirrorStatus, String> {
        let content = if env_path.exists() {
            std::fs::read_to_string(env_path)
                .map_err(|e| format!("读取 .env 文件失败: {e}"))?
        } else {
            String::new()
        };

        let env_file = EnvFile::parse(&content)
            .map_err(|e| format!("解析 .env 文件失败: {e}"))?;

        Ok(MirrorStatus {
            docker_registry: env_file
                .get("DOCKER_REGISTRY_MIRROR")
                .unwrap_or("")
                .to_string(),
            apt: env_file.get("APT_MIRROR").unwrap_or("default").to_string(),
            composer: env_file
                .get("COMPOSER_MIRROR")
                .unwrap_or("default")
                .to_string(),
            npm: env_file.get("NPM_MIRROR").unwrap_or("default").to_string(),
            github: env_file.get("GITHUB_PROXY").unwrap_or("").to_string(),
        })
    }

    /// 检测当前配置匹配的预设名称
    ///
    /// 比较当前 .env 中的配置与所有预设，返回最匹配的预设名称。
    /// 如果没有任何预设完全匹配，返回 "官方默认"。
    pub fn detect_current_preset(env_path: &Path) -> Result<String, String> {
        let status = Self::get_current_status(env_path)?;
        let presets = Self::get_presets();

        // 尝试找到完全匹配的预设
        for preset in &presets {
            if status.docker_registry == preset.docker_registry
                && status.apt.as_str() == preset.apt.as_str()
                && status.composer.as_str() == preset.composer.as_str()
                && status.npm == preset.npm
            {
                return Ok(preset.name.clone());
            }
        }

        // 如果没有完全匹配，检查是否为空配置（首次使用）
        if status.docker_registry.is_empty()
            && (status.apt == "default" || status.apt.is_empty())
            && (status.composer == "default" || status.composer.is_empty())
            && (status.npm == "default" || status.npm.is_empty() || status.npm == "https://registry.npmjs.org")
        {
            return Ok("官方默认".to_string());
        }

        // 否则返回"官方默认"作为fallback
        Ok("官方默认".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 辅助函数：创建临时 .env 文件用于测试
    fn create_temp_env(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let env_path = dir.path().join(".env");
        fs::write(&env_path, content).expect("写入临时 .env 文件失败");
        (dir, env_path)
    }

    #[test]
    fn test_get_presets() {
        let presets = MirrorManager::get_presets();
        assert_eq!(presets.len(), 5);

        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"阿里云全套"));
        assert!(names.contains(&"清华大学全套"));
        assert!(names.contains(&"腾讯云全套"));
        assert!(names.contains(&"中科大全套"));
        assert!(names.contains(&"官方默认"));

        // 验证阿里云预设的具体值
        let aliyun = presets.iter().find(|p| p.name == "阿里云全套").unwrap();
        assert_eq!(
            aliyun.docker_registry,
            "https://registry.cn-hangzhou.aliyuncs.com"
        );
        assert_eq!(aliyun.apt, MirrorSource::Aliyun);
        assert_eq!(aliyun.composer, MirrorSource::Aliyun);
        assert_eq!(aliyun.npm, "https://registry.npmmirror.com");

        // 验证官方默认预设
        let default = presets.iter().find(|p| p.name == "官方默认").unwrap();
        assert_eq!(default.docker_registry, "");
        assert_eq!(default.apt, MirrorSource::Default);
        assert_eq!(default.composer, MirrorSource::Default);
        assert_eq!(default.npm, "https://registry.npmjs.org");
    }

    #[test]
    fn test_apply_preset_aliyun() {
        let (_dir, env_path) = create_temp_env("# 测试文件\nSOURCE_DIR=/projects\n");

        MirrorManager::apply_preset("阿里云全套", &env_path).expect("应用预设失败");

        let content = fs::read_to_string(&env_path).expect("读取文件失败");
        assert!(content.contains("DOCKER_REGISTRY_MIRROR=https://registry.cn-hangzhou.aliyuncs.com"));
        assert!(content.contains("APT_MIRROR=aliyun"));
        assert!(content.contains("COMPOSER_MIRROR=aliyun"));
        assert!(content.contains("NPM_MIRROR=https://registry.npmmirror.com"));
        // 验证原有内容保留
        assert!(content.contains("SOURCE_DIR=/projects"));
        assert!(content.contains("# 测试文件"));
    }

    #[test]
    fn test_apply_preset_not_found() {
        let (_dir, env_path) = create_temp_env("");
        let result = MirrorManager::apply_preset("不存在的预设", &env_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到预设方案"));
    }

    #[test]
    fn test_update_single_preserves_others() {
        let initial = "DOCKER_REGISTRY_MIRROR=https://old.example.com\nAPT_MIRROR=aliyun\nCOMPOSER_MIRROR=aliyun\nNPM_MIRROR=https://registry.npmmirror.com\n";
        let (_dir, env_path) = create_temp_env(initial);

        // 只更新 apt
        MirrorManager::update_single("apt", "tsinghua", &env_path).expect("更新失败");

        let content = fs::read_to_string(&env_path).expect("读取文件失败");
        // apt 已更新
        assert!(content.contains("APT_MIRROR=tsinghua"));
        // 其他保持不变
        assert!(content.contains("DOCKER_REGISTRY_MIRROR=https://old.example.com"));
        assert!(content.contains("COMPOSER_MIRROR=aliyun"));
        assert!(content.contains("NPM_MIRROR=https://registry.npmmirror.com"));
    }

    #[test]
    fn test_update_single_invalid_category() {
        let (_dir, env_path) = create_temp_env("");
        let result = MirrorManager::update_single("invalid", "value", &env_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未知的镜像源类别"));
    }

    #[test]
    fn test_get_current_status() {
        let content = "DOCKER_REGISTRY_MIRROR=https://registry.cn-hangzhou.aliyuncs.com\nAPT_MIRROR=aliyun\nCOMPOSER_MIRROR=huaweicloud\nNPM_MIRROR=https://registry.npmmirror.com\n";
        let (_dir, env_path) = create_temp_env(content);

        let status = MirrorManager::get_current_status(&env_path).expect("获取状态失败");
        assert_eq!(
            status.docker_registry,
            "https://registry.cn-hangzhou.aliyuncs.com"
        );
        assert_eq!(status.apt, "aliyun");
        assert_eq!(status.composer, "huaweicloud");
        assert_eq!(status.npm, "https://registry.npmmirror.com");
    }

    #[test]
    fn test_get_current_status_empty_env() {
        let (_dir, env_path) = create_temp_env("");

        let status = MirrorManager::get_current_status(&env_path).expect("获取状态失败");
        assert_eq!(status.docker_registry, "");
        assert_eq!(status.apt, "default");
        assert_eq!(status.composer, "default");
        assert_eq!(status.npm, "default");
    }

    #[test]
    fn test_get_current_status_nonexistent_file() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let env_path = dir.path().join(".env.nonexistent");

        let status = MirrorManager::get_current_status(&env_path).expect("获取状态失败");
        assert_eq!(status.docker_registry, "");
        assert_eq!(status.apt, "default");
        assert_eq!(status.composer, "default");
        assert_eq!(status.npm, "default");
    }

    #[test]
    fn test_apply_preset_creates_env_if_missing() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let env_path = dir.path().join(".env");

        // .env 文件不存在时也能正常应用预设
        MirrorManager::apply_preset("官方默认", &env_path).expect("应用预设失败");

        let content = fs::read_to_string(&env_path).expect("读取文件失败");
        assert!(content.contains("DOCKER_REGISTRY_MIRROR="));
        assert!(content.contains("APT_MIRROR=default"));
        assert!(content.contains("COMPOSER_MIRROR=default"));
        assert!(content.contains("NPM_MIRROR=https://registry.npmjs.org"));
    }
}
