pub mod host_status;
pub mod manager;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::Command;

use host_status::{current_os, resolve_docker_binary};

/// 构造 `docker` CLI 进程。
///
/// - 解析绝对路径（PATH + 常见安装位置），避免 macOS GUI 启动时 `PATH` 缺 `/usr/local/bin` 导致 `os error 2`
/// - Windows 下隐藏控制台窗口（`CREATE_NO_WINDOW`），避免黑框闪烁
pub fn docker_cli() -> Command {
    let program = resolve_docker_binary(current_os()).unwrap_or_else(|| {
        PathBuf::from(if cfg!(windows) {
            "docker.exe"
        } else {
            "docker"
        })
    });

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new(program);
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        cmd.creation_flags(0x08000200);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}
