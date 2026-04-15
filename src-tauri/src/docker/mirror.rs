use std::fs;
use std::path::PathBuf;
use serde_json::Value;

pub struct MirrorManager;

impl MirrorManager {
    /// 修改 Docker daemon.json 实现国内镜像源切换
    pub fn set_docker_mirror(mirror_url: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = if cfg!(target_os = "windows") {
            // Windows 下通常由 Docker Desktop 管理，此处提供路径参考
            PathBuf::from(std::env::var("USERPROFILE")?).join(".docker").join("daemon.json")
        } else {
            PathBuf::from("/etc/docker/daemon.json")
        };

        let mut config: Value = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            serde_json::json!({})
        };

        config["registry-mirrors"] = serde_json::json!([mirror_url]);

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
        Ok(())
    }

    /// 生成 PHP 容器内镜像源替换指令
    pub fn get_php_mirror_commands(mirror_type: &str) -> Vec<String> {
        match mirror_type {
            "aliyun" => vec![
                "sed -i 's/deb.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list".to_string(),
                "composer config -g repo.packagist php https://mirrors.aliyun.com/composer/".to_string(),
            ],
            "ustc" => vec![
                "sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list".to_string(),
                "composer config -g repo.packagist php https://mirrors.aliyun.com/composer/".to_string(),
            ],
            _ => vec![],
        }
    }
}
