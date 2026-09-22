use crate::app_log;
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

/// 前端上报的错误（来自 public/boot-guard.js 或 Vue 的 errorHandler）。
///
/// 字段全部可选/可空：前端在「业务代码都没跑起来」的场景下上报，能拿到的
/// 信息本就不完整，缺字段不应导致整条上报被丢弃。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FrontendErrorReport {
    pub message: String,
    pub source: Option<String>,
    pub stack: Option<String>,
    pub location: Option<String>,
}

/// 单字段写入日志前的最大字符数。栈帧可以很长，截断避免一条错误刷满日志。
const FRONTEND_ERROR_FIELD_LIMIT: usize = 2000;

/// 按**字符**截断（不是字节）——按字节切片会在多字节字符中间断开并 panic，
/// 而错误文案里出现中文是常态。
fn truncate_field(value: Option<String>) -> Option<String> {
    let value = value.map(|v| v.trim().to_string())?;
    if value.is_empty() {
        return None;
    }
    if value.chars().count() <= FRONTEND_ERROR_FIELD_LIMIT {
        return Some(value);
    }
    let head: String = value.chars().take(FRONTEND_ERROR_FIELD_LIMIT).collect();
    Some(format!("{head}...(truncated)"))
}

/// 接收前端错误并写入日志文件的 `[frontend]` 通道。
///
/// 生产包未启用 devtools，白屏时用户看不到控制台；日志是唯一现场。此命令不
/// 返回错误——上报本身失败不该干扰界面。
#[tauri::command]
pub fn log_frontend_error(report: FrontendErrorReport) {
    let message = truncate_field(Some(report.message)).unwrap_or_else(|| "(empty)".to_string());
    let source = truncate_field(report.source).unwrap_or_else(|| "unknown".to_string());

    app_log!(error, "frontend", "[{}] {}", source, message);
    if let Some(location) = truncate_field(report.location) {
        app_log!(error, "frontend", "  at {}", location);
    }
    if let Some(stack) = truncate_field(report.stack) {
        app_log!(error, "frontend", "  stack: {}", stack);
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

    #[test]
    fn test_truncate_field_keeps_short_value() {
        assert_eq!(
            truncate_field(Some("boom".to_string())).as_deref(),
            Some("boom")
        );
    }

    #[test]
    fn test_truncate_field_drops_blank() {
        assert_eq!(truncate_field(None), None);
        assert_eq!(truncate_field(Some("   ".to_string())), None);
        assert_eq!(truncate_field(Some(String::new())), None);
    }

    #[test]
    fn test_truncate_field_caps_at_limit() {
        let long = "x".repeat(FRONTEND_ERROR_FIELD_LIMIT + 500);
        let out = truncate_field(Some(long)).expect("非空白应保留");
        assert!(out.ends_with("...(truncated)"), "超长应带截断标记");
        assert_eq!(
            out.chars().count(),
            FRONTEND_ERROR_FIELD_LIMIT + "...(truncated)".len()
        );
    }

    /// 按字节切片会在多字节字符中间断开并 panic，这里锁住按字符截断的行为
    #[test]
    fn test_truncate_field_survives_multibyte() {
        let long: String = "汉".repeat(FRONTEND_ERROR_FIELD_LIMIT + 10);
        let out = truncate_field(Some(long)).expect("非空白应保留");
        assert!(out.ends_with("...(truncated)"));
        assert_eq!(
            out.chars().count(),
            FRONTEND_ERROR_FIELD_LIMIT + "...(truncated)".chars().count()
        );
    }

    #[test]
    fn test_log_frontend_error_accepts_sparse_report() {
        // 前端在启动失败时可能只拿得到 message，其余字段缺失不应丢弃整条上报
        log_frontend_error(FrontendErrorReport {
            message: "boot failed".to_string(),
            source: None,
            stack: None,
            location: None,
        });
        log_frontend_error(FrontendErrorReport {
            message: "  with stack  ".to_string(),
            source: Some("window.onerror".to_string()),
            stack: Some("  at foo (bar.js:1:1)  ".to_string()),
            location: Some("http://tauri.localhost/assets/index.js:1:1".to_string()),
        });
    }
}
