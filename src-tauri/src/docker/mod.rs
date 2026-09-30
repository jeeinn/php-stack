pub mod host_status;
pub mod manager;
#[cfg(test)]
mod tests;

use std::process::Command;

/// 构造 `docker` CLI 进程；Windows 下隐藏控制台窗口，避免黑框闪烁。
///
/// 标志：`CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)`
pub fn docker_cli() -> Command {
    let mut cmd = Command::new("docker");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000200);
    }
    cmd
}
