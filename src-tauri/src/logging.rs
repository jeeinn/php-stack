use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing_subscriber::{fmt, EnvFilter};

/// 全局日志文件句柄（线程安全）
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// 初始化日志系统
pub fn init_logging(app_data_dir: &PathBuf) -> Result<(), String> {
    // 确保目录存在
    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("无法创建应用数据目录 {:?}: {}", app_data_dir, e))?;
    
    let log_path = app_data_dir.join("php-stack.log");
    
    // 每次启动时覆盖写入（truncate）
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)  // 关键：覆盖旧日志
        .open(&log_path)
        .map_err(|e| format!("无法创建日志文件 {:?}: {}", log_path, e))?;
    
    // 保存文件句柄到全局变量
    *LOG_FILE.lock().unwrap() = Some(file);
    
    // 配置 tracing subscriber
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,app=debug"));
    
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
    if let Some(file) = LOG_FILE.lock().unwrap().as_mut() {
        let now = chrono::Local::now();
        let timestamp = now.format("[%H:%M:%S%.3f]").to_string();
        let log_line = format!("{} {} [{}] {}\n", timestamp, level, module, message);
        let _ = file.write_all(log_line.as_bytes());
        let _ = file.flush();
    }
}
