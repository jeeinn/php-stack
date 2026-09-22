use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use tracing_subscriber::{fmt, EnvFilter};

/// Payload pushed to the frontend via the `env-log` event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UiLogPayload {
    pub level: String,
    pub message: String,
}

/// 全局日志文件句柄（线程安全）
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// 保留的历史日志份数（不含当前日志）
const RETAINED_LOG_GENERATIONS: usize = 2;

/// 当前日志名与历史日志名（`.1` 最近，`.2` 更早）
const CURRENT_LOG_NAME: &str = "php-stack.log";

/// 日志轮转：当前 → `.1` → `.2`，超出保留份数的删除。
///
/// 此前每次启动 `truncate(true)` 直接覆盖，用户遇到问题后一重启，现场日志
/// 就没了——"导出日志"命令也随之失去排查价值。
fn rotate_logs(dir: &Path) -> Result<(), String> {
    // 从最老的一份开始处理，避免覆盖尚未移走的日志
    for generation in (1..=RETAINED_LOG_GENERATIONS).rev() {
        let from = if generation == 1 {
            dir.join(CURRENT_LOG_NAME)
        } else {
            dir.join(format!("php-stack.{}.log", generation - 1))
        };
        let to = dir.join(format!("php-stack.{generation}.log"));

        if !from.exists() {
            continue;
        }

        // 最老的一份已被挤出保留范围，直接丢弃
        if generation == RETAINED_LOG_GENERATIONS && to.exists() {
            std::fs::remove_file(&to)
                .map_err(|e| format!("failed to delete old log {}: {e}", to.display()))?;
        }

        std::fs::rename(&from, &to)
            .map_err(|e| format!("failed to rotate log {}: {e}", from.display()))?;
    }

    Ok(())
}

/// 取锁并在锁被毒化时恢复。
///
/// 直接用 `unwrap()` 的话，任何一次持锁期间 panic 都会毒化全局日志锁，
/// 之后每一次写日志都连锁 panic——把一个小故障放大成整个应用不可用。
fn lock_log_file() -> std::sync::MutexGuard<'static, Option<File>> {
    LOG_FILE.lock().unwrap_or_else(|e| e.into_inner())
}

/// 初始化日志系统
pub fn init_logging(app_data_dir: &std::path::PathBuf) -> Result<(), String> {
    // 确保目录存在
    std::fs::create_dir_all(app_data_dir).map_err(|e| {
        format!(
            "failed to create app data dir {}: {e}",
            app_data_dir.display()
        )
    })?;

    rotate_logs(app_data_dir)?;

    let log_path = app_data_dir.join(CURRENT_LOG_NAME);

    // 轮转后当前日志一定是新文件；truncate 仅作兜底
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|e| format!("failed to create log file {}: {e}", log_path.display()))?;

    // 写入启动分隔线，便于区分多次启动的日志段
    let now = chrono::Local::now();
    let _ = writeln!(
        file,
        "=========== PHP-Stack started {} ===========",
        now.format("%Y-%m-%d %H:%M:%S")
    );
    let _ = file.flush();

    // 保存文件句柄到全局变量
    *lock_log_file() = Some(file);

    // 配置 tracing subscriber
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,app=debug"));

    // 自定义格式化器：添加时间前缀
    let formatter = fmt::format()
        .with_target(false)
        .with_level(true)
        .with_timer(CustomTimer)
        .compact();

    // 输出到控制台
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .event_format(formatter)
        .with_writer(std::io::stdout)
        .init();

    Ok(())
}

/// 自定义时间格式化器
struct CustomTimer;

impl tracing_subscriber::fmt::time::FormatTime for CustomTimer {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("[%H:%M:%S%.3f]"))
    }
}

/// 向日志文件写入消息（供宏调用）
pub fn write_to_log_file(level: &str, module: &str, message: &str) {
    if let Some(file) = lock_log_file().as_mut() {
        let now = chrono::Local::now();
        let timestamp = now.format("[%H:%M:%S%.3f]").to_string();
        let log_line = format!("{timestamp} {level} [{module}] {message}\n");
        let _ = file.write_all(log_line.as_bytes());
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("php-stack-logging-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_rotate_shifts_generations() {
        let dir = fresh_dir("shift");
        std::fs::write(dir.join("php-stack.log"), "current").unwrap();
        std::fs::write(dir.join("php-stack.1.log"), "previous").unwrap();

        rotate_logs(&dir).unwrap();

        assert!(
            !dir.join("php-stack.log").exists(),
            "current log should be rotated away"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("php-stack.1.log")).unwrap(),
            "current",
            "current log should become .1"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("php-stack.2.log")).unwrap(),
            "previous",
            "previous .1 should become .2"
        );
    }

    #[test]
    fn test_rotate_drops_oldest_beyond_retention() {
        let dir = fresh_dir("retention");
        std::fs::write(dir.join("php-stack.log"), "t0").unwrap();
        std::fs::write(dir.join("php-stack.1.log"), "t1").unwrap();
        std::fs::write(dir.join("php-stack.2.log"), "t2").unwrap();

        rotate_logs(&dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.join("php-stack.1.log")).unwrap(),
            "t0"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("php-stack.2.log")).unwrap(),
            "t1"
        );
        // t2 已超出保留份数，应被丢弃而不是留下第三份
        assert!(!dir.join("php-stack.3.log").exists());
    }

    #[test]
    fn test_rotate_noop_on_empty_dir() {
        let dir = fresh_dir("empty");
        rotate_logs(&dir).unwrap();
        assert!(!dir.join("php-stack.1.log").exists());
    }

    #[test]
    fn test_rotate_preserves_content() {
        let dir = fresh_dir("content");
        let body: String = (0..2000)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        std::fs::write(dir.join("php-stack.log"), &body).unwrap();

        rotate_logs(&dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.join("php-stack.1.log")).unwrap(),
            body
        );
    }

    #[test]
    fn test_ui_log_payload_roundtrip() {
        let payload = UiLogPayload {
            level: "warn".to_string(),
            message: "port in use".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: UiLogPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.level, "warn");
        assert_eq!(back.message, "port in use");
        assert!(json.contains("\"level\""));
        assert!(json.contains("\"message\""));
    }
}
