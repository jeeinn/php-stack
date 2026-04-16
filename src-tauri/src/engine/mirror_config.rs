/// 镜像源配置模块
/// 
/// 统一管理 Docker、APT、Composer、PyPI、NPM 等镜像源配置
/// 配置存储在 .env 文件中，支持动态加载和保存

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 镜像源配置
/// 
/// V1.0: 统一管理容器内依赖镜像源（APT/Composer/PyPI/NPM）
/// 注意：Docker Daemon 的 registry-mirrors 由 docker/mirror.rs 管理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorConfig {
    // 容器内依赖镜像源
    pub apt_mirror: MirrorSource,
    pub composer_mirror: MirrorSource,
    pub pypi_mirror: MirrorSource,
    pub npm_mirror: MirrorSource,
    
    // Docker 构建代理（可选）
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<String>,
}

/// 镜像源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MirrorSource {
    Default,
    Aliyun,
    Tsinghua,
    Ustc,
    Tencent,
    HuaweiCloud,
    Taobao,
}

impl MirrorSource {
    /// 获取镜像 URL
    pub fn get_url(&self, service: &str) -> String {
        match (self, service) {
            // Docker Registry
            (MirrorSource::Ustc, "docker") => {
                "https://docker.mirrors.ustc.edu.cn".to_string()
            }
            (MirrorSource::Tencent, "docker") => {
                "https://mirror.ccs.tencentyun.com".to_string()
            }
            
            // APT
            (MirrorSource::Aliyun, "apt") => {
                "http://mirrors.aliyun.com/debian/".to_string()
            }
            (MirrorSource::Tsinghua, "apt") => {
                "https://mirrors.tuna.tsinghua.edu.cn/debian/".to_string()
            }
            (MirrorSource::Ustc, "apt") => {
                "https://mirrors.ustc.edu.cn/debian/".to_string()
            }
            
            // Composer
            (MirrorSource::Aliyun, "composer") => {
                "https://mirrors.aliyun.com/composer/".to_string()
            }
            (MirrorSource::HuaweiCloud, "composer") => {
                "https://repo.huaweicloud.com/repository/php/".to_string()
            }
            
            // PyPI
            (MirrorSource::Aliyun, "pypi") => {
                "https://mirrors.aliyun.com/pypi/simple/".to_string()
            }
            (MirrorSource::Tsinghua, "pypi") => {
                "https://pypi.tuna.tsinghua.edu.cn/simple".to_string()
            }
            
            // NPM
            (MirrorSource::Taobao, "npm") => {
                "https://registry.npmmirror.com".to_string()
            }
            (MirrorSource::Tencent, "npm") => {
                "https://mirrors.cloud.tencent.com/npm/".to_string()
            }
            
            _ => String::new(), // Default or unsupported
        }
    }
    
    /// 转换为字符串标识
    pub fn as_str(&self) -> &str {
        match self {
            MirrorSource::Default => "default",
            MirrorSource::Aliyun => "aliyun",
            MirrorSource::Tsinghua => "tsinghua",
            MirrorSource::Ustc => "ustc",
            MirrorSource::Tencent => "tencent",
            MirrorSource::HuaweiCloud => "huaweicloud",
            MirrorSource::Taobao => "taobao",
        }
    }
    
    /// 从字符串解析
    pub fn from_str(s: &str) -> Self {
        match s {
            "aliyun" => MirrorSource::Aliyun,
            "tsinghua" => MirrorSource::Tsinghua,
            "ustc" => MirrorSource::Ustc,
            "tencent" => MirrorSource::Tencent,
            "huaweicloud" => MirrorSource::HuaweiCloud,
            "taobao" => MirrorSource::Taobao,
            _ => MirrorSource::Default,
        }
    }
}

impl MirrorConfig {
    /// 从 .env 文件加载配置
    pub fn load_from_env() -> Result<Self, String> {
        let env_path = Path::new(".env");
        
        if !env_path.exists() {
            log::warn!("⚠️ .env 文件不存在，使用默认配置");
            return Ok(Self::default());
        }
        
        let env_content = fs::read_to_string(env_path)
            .map_err(|e| format!("读取 .env 文件失败: {}", e))?;
        
        let env_map = Self::parse_env_file(&env_content);
        
        Ok(Self {
            apt_mirror: MirrorSource::from_str(
                env_map.get("APT_MIRROR").map(|s| s.as_str()).unwrap_or("default")
            ),
            composer_mirror: MirrorSource::from_str(
                env_map.get("COMPOSER_MIRROR").map(|s| s.as_str()).unwrap_or("default")
            ),
            pypi_mirror: MirrorSource::from_str(
                env_map.get("PYPI_MIRROR").map(|s| s.as_str()).unwrap_or("default")
            ),
            npm_mirror: MirrorSource::from_str(
                env_map.get("NPM_MIRROR").map(|s| s.as_str()).unwrap_or("default")
            ),
            http_proxy: env_map.get("HTTP_PROXY").cloned(),
            https_proxy: env_map.get("HTTPS_PROXY").cloned(),
            no_proxy: env_map.get("NO_PROXY").cloned(),
        })
    }
    
    /// 解析 .env 文件内容
    fn parse_env_file(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        
        for line in content.lines() {
            let line = line.trim();
            
            // 跳过注释和空行
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // 解析 KEY=VALUE
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
                map.insert(key, value);
            }
        }
        
        map
    }
    
    /// 保存到 .env 文件
    pub fn save_to_env(&self) -> Result<(), String> {
        let env_path = Path::new(".env");
        let mut env_content = if env_path.exists() {
            fs::read_to_string(env_path)
                .map_err(|e| format!("读取 .env 文件失败: {}", e))?
        } else {
            String::new()
        };
        
        // 更新或添加配置项（仅容器内依赖镜像源）
        env_content = Self::update_env_value(&env_content, "APT_MIRROR", self.apt_mirror.as_str());
        env_content = Self::update_env_value(&env_content, "COMPOSER_MIRROR", self.composer_mirror.as_str());
        env_content = Self::update_env_value(&env_content, "PYPI_MIRROR", self.pypi_mirror.as_str());
        env_content = Self::update_env_value(&env_content, "NPM_MIRROR", self.npm_mirror.as_str());
        
        if let Some(proxy) = &self.http_proxy {
            env_content = Self::update_env_value(&env_content, "HTTP_PROXY", proxy);
        }
        if let Some(proxy) = &self.https_proxy {
            env_content = Self::update_env_value(&env_content, "HTTPS_PROXY", proxy);
        }
        
        fs::write(env_path, env_content)
            .map_err(|e| format!("写入 .env 文件失败: {}", e))?;
        log::info!("✅ 容器内镜像源配置已保存到 .env");
        Ok(())
    }
    
    /// 更新 .env 文件中的值
    fn update_env_value(content: &str, key: &str, value: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut new_lines: Vec<String> = Vec::new();
        let mut found = false;
        
        for line in lines {
            if line.trim().starts_with(&format!("{}=", key)) {
                new_lines.push(format!("{}={}", key, value));
                found = true;
            } else {
                new_lines.push(line.to_string());
            }
        }
        
        if !found {
            new_lines.push(format!("{}={}", key, value));
        }
        
        new_lines.join("\n")
    }
    
    /// 生成 Docker 构建参数
    pub fn to_build_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        
        // 添加代理配置
        if let Some(proxy) = &self.http_proxy {
            args.push(format!("HTTP_PROXY={}", proxy));
        }
        if let Some(proxy) = &self.https_proxy {
            args.push(format!("HTTPS_PROXY={}", proxy));
        }
        if let Some(no_proxy) = &self.no_proxy {
            args.push(format!("NO_PROXY={}", no_proxy));
        }
        
        args
    }
    
    /// 生成 Dockerfile 中的镜像源配置片段
    pub fn to_dockerfile_snippet(&self) -> String {
        let mut snippet = String::new();
        
        // APT 镜像源
        if self.apt_mirror != MirrorSource::Default {
            let apt_url = self.apt_mirror.get_url("apt");
            snippet.push_str(&format!(
                r#"# 配置 APT 镜像源
RUN sed -i 's|deb.debian.org/debian|{}|g' /etc/apt/sources.list && \
    sed -i 's|security.debian.org|{}|g' /etc/apt/sources.list

"#,
                apt_url.trim_end_matches('/'),
                apt_url.replace("debian", "debian-security").trim_end_matches('/')
            ));
        }
        
        // Composer 镜像源（先安装 Composer）
        if self.composer_mirror != MirrorSource::Default {
            let composer_url = self.composer_mirror.get_url("composer");
            snippet.push_str(&format!(
                r#"# 安装并配置 Composer 镜像源
RUN curl -sS https://getcomposer.org/installer | php -- --install-dir=/usr/local/bin --filename=composer && \
    composer config -g repo.packagist composer {}

"#,
                composer_url.trim_end_matches('/')
            ));
        }
        
        // PyPI 镜像源
        if self.pypi_mirror != MirrorSource::Default {
            let pypi_url = self.pypi_mirror.get_url("pypi");
            snippet.push_str(&format!(
                r#"# 配置 PyPI 镜像源
RUN pip config set global.index-url {}

"#,
                pypi_url.trim_end_matches('/')
            ));
        }
        
        snippet
    }
}

impl Default for MirrorConfig {
    fn default() -> Self {
        Self {
            apt_mirror: MirrorSource::Default,
            composer_mirror: MirrorSource::Default,
            pypi_mirror: MirrorSource::Default,
            npm_mirror: MirrorSource::Default,
            http_proxy: None,
            https_proxy: None,
            no_proxy: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mirror_source_from_str() {
        assert_eq!(MirrorSource::from_str("aliyun"), MirrorSource::Aliyun);
        assert_eq!(MirrorSource::from_str("tsinghua"), MirrorSource::Tsinghua);
        assert_eq!(MirrorSource::from_str("invalid"), MirrorSource::Default);
    }
    
    #[test]
    fn test_mirror_source_get_url() {
        let url = MirrorSource::Aliyun.get_url("apt");
        assert!(url.contains("aliyun.com"));
        
        let url = MirrorSource::Tsinghua.get_url("pypi");
        assert!(url.contains("tsinghua.edu.cn"));
    }
    
    #[test]
    fn test_parse_env_file() {
        let content = r#"
# Comment
KEY1=value1
KEY2="value2"
KEY3='value3'
"#;
        let map = MirrorConfig::parse_env_file(content);
        assert_eq!(map.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(map.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(map.get("KEY3"), Some(&"value3".to_string()));
    }
}
