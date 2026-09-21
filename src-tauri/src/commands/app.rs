use os_info;

/// 关于页展示的支持信息（可复制）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SupportInfo {
    pub app_version: String,
    pub os: String,
    pub os_version: String,
    pub arch: String,
}

#[tauri::command]
pub fn get_support_info() -> SupportInfo {
    let info = os_info::get();
    SupportInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: info.os_type().to_string(),
        os_version: info.version().to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_support_info_fields_nonempty() {
        let info = get_support_info();
        assert!(!info.app_version.is_empty(), "app_version");
        assert!(!info.os.is_empty(), "os");
        assert!(!info.os_version.is_empty(), "os_version");
        assert!(!info.arch.is_empty(), "arch");
    }
}
