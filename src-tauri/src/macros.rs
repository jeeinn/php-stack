/// 应用内部日志（仅写入文件和控制台）
#[macro_export]
macro_rules! app_log {
    (info, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            tracing::info!(target: $module, "{}", msg);
            $crate::logging::write_to_log_file("INFO", $module, &msg);
        }
    };
    (warn, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            tracing::warn!(target: $module, "{}", msg);
            $crate::logging::write_to_log_file("WARN", $module, &msg);
        }
    };
    (error, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            tracing::error!(target: $module, "{}", msg);
            $crate::logging::write_to_log_file("ERROR", $module, &msg);
        }
    };
    (debug, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            tracing::debug!(target: $module, "{}", msg);
            $crate::logging::write_to_log_file("DEBUG", $module, &msg);
        }
    };
}

/// 用户可见日志（同时发送到前端 UI）
#[macro_export]
macro_rules! ui_log {
    ($app_handle:expr, info, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            $crate::app_log!(info, $module, "{}", msg);
            let _ = $app_handle.emit("env-log", &msg);
        }
    };
    ($app_handle:expr, warn, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            $crate::app_log!(warn, $module, "{}", msg);
            let _ = $app_handle.emit("env-log", &msg);
        }
    };
    ($app_handle:expr, error, $module:expr, $($arg:tt)*) => {
        {
            let msg = format!($($arg)*);
            $crate::app_log!(error, $module, "{}", msg);
            let _ = $app_handle.emit("env-log", &msg);
        }
    };
}
