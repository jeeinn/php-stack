//! 本机 Docker 宿主状态：区分未安装、已安装未启动、权限不足、自定义 DOCKER_HOST。
//!
//! `bollard` 建连是惰性的，`ping` 失败时的 `Connect` 同时覆盖「没装」和「装了没开」。
//! 分类只吃已经采集好的事实，方便不依赖本机是否装了 Docker 做单测。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 发给前端的宿主状态。序列化为 snake_case，与 `src/types/docker.ts` 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerHostKind {
    Ready,
    NotInstalled,
    InstalledStopped,
    PermissionDenied,
    CustomHost,
}

/// 一次探测的结果。`detail` 是技术原文，主文案由前端按 `kind` 做 i18n。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerHostReport {
    pub kind: DockerHostKind,
    pub detail: String,
    pub install_url: String,
    pub can_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostOs {
    Windows,
    Macos,
    Linux,
    Other,
}

/// 分类输入。探测（读环境变量、看文件、连套接字）在外面做完再传进来。
/// 四个布尔是互相独立的事实，收成枚举反而要先解释组合含义。
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostFacts {
    pub os: HostOs,
    pub docker_host: Option<String>,
    pub desktop_app_exists: bool,
    pub docker_cli_exists: bool,
    /// ping 或本地套接字返回了权限错误（Linux 上常见于用户不在 docker 组）。
    pub endpoint_permission_denied: bool,
    pub ping_ok: bool,
    pub ping_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopLaunch {
    pub program: PathBuf,
    pub args: Vec<String>,
}

const INSTALL_URL_WINDOWS: &str = "https://docs.docker.com/desktop/setup/install/windows-install/";
const INSTALL_URL_MACOS: &str = "https://docs.docker.com/desktop/setup/install/mac-install/";
const INSTALL_URL_LINUX: &str = "https://docs.docker.com/desktop/setup/install/linux/";

pub(crate) fn current_os() -> HostOs {
    match std::env::consts::OS {
        "windows" => HostOs::Windows,
        "macos" => HostOs::Macos,
        "linux" => HostOs::Linux,
        _ => HostOs::Other,
    }
}

pub(crate) fn docker_desktop_install_url(os: HostOs) -> &'static str {
    match os {
        HostOs::Windows => INSTALL_URL_WINDOWS,
        HostOs::Macos => INSTALL_URL_MACOS,
        HostOs::Linux | HostOs::Other => INSTALL_URL_LINUX,
    }
}

/// `DOCKER_HOST` 未设置，或指向本机默认管道/套接字时，不是自定义地址。
pub(crate) fn is_custom_docker_host(os: HostOs, docker_host: Option<&str>) -> bool {
    let Some(raw) = docker_host.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    !is_local_default_endpoint(os, raw)
}

fn is_local_default_endpoint(os: HostOs, raw: &str) -> bool {
    let normalized = raw.trim().trim_end_matches('/').replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    match os {
        HostOs::Windows => is_windows_default_pipe(&lower),
        HostOs::Macos | HostOs::Linux | HostOs::Other => is_unix_default_socket(&lower),
    }
}

fn is_windows_default_pipe(lower: &str) -> bool {
    lower.ends_with("/pipe/docker_engine") || lower.ends_with("/pipe/dockerdesktoplinuxengine")
}

fn is_unix_default_socket(lower: &str) -> bool {
    let path = lower.trim_start_matches("unix://");
    path == "/var/run/docker.sock"
        || path.ends_with("/.docker/run/docker.sock")
        || path.ends_with("/.docker/desktop/docker.sock")
}

pub(crate) fn error_is_permission_denied(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "permission denied",
        "access is denied",
        "os error 13",
        "os error 5",
        "eacces",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// 优先级：引擎可用 > 自定义地址 > 权限 > 未安装 > 已安装但无响应。
pub(crate) fn classify(facts: &HostFacts) -> DockerHostKind {
    if facts.ping_ok {
        return DockerHostKind::Ready;
    }
    if is_custom_docker_host(facts.os, facts.docker_host.as_deref()) {
        return DockerHostKind::CustomHost;
    }
    if facts.endpoint_permission_denied {
        return DockerHostKind::PermissionDenied;
    }
    if !facts.desktop_app_exists && !facts.docker_cli_exists {
        return DockerHostKind::NotInstalled;
    }
    DockerHostKind::InstalledStopped
}

pub(crate) fn can_open_desktop(facts: &HostFacts) -> bool {
    classify(facts) == DockerHostKind::InstalledStopped && facts.desktop_app_exists
}

pub(crate) fn build_report(facts: &HostFacts) -> DockerHostReport {
    DockerHostReport {
        kind: classify(facts),
        detail: facts.ping_error.clone().unwrap_or_default(),
        install_url: docker_desktop_install_url(facts.os).to_string(),
        can_open: can_open_desktop(facts),
    }
}

/// macOS 用 `open -a Docker`；Windows / Linux 直接启动找到的程序。
pub(crate) fn desktop_launch(os: HostOs, desktop_path: &Path) -> DesktopLaunch {
    match os {
        HostOs::Macos => DesktopLaunch {
            program: PathBuf::from("open"),
            args: vec!["-a".to_string(), "Docker".to_string()],
        },
        HostOs::Windows | HostOs::Linux | HostOs::Other => DesktopLaunch {
            program: desktop_path.to_path_buf(),
            args: Vec::new(),
        },
    }
}

/// ping 成功时不再扫磁盘。失败时才看本机有没有 Desktop / CLI，以及套接字权限。
pub fn report_from_ping(ping_error: Option<String>) -> DockerHostReport {
    if ping_error.is_none() {
        return DockerHostReport {
            kind: DockerHostKind::Ready,
            detail: String::new(),
            install_url: docker_desktop_install_url(current_os()).to_string(),
            can_open: false,
        };
    }
    build_report(&collect_facts(ping_error))
}

pub fn launch_docker_desktop() -> Result<(), String> {
    let os = current_os();
    let desktop = find_desktop_app(os).ok_or_else(|| "Docker Desktop was not found".to_string())?;
    let launch = desktop_launch(os, &desktop);
    spawn_detached(&launch.program, &launch.args)
}

fn collect_facts(ping_error: Option<String>) -> HostFacts {
    let os = current_os();
    let ping_denied = ping_error
        .as_deref()
        .is_some_and(error_is_permission_denied);
    // 引擎正常时不会走到这里。套接字探测只在 ping 已失败后做，避免多连一次。
    let socket_denied = local_socket_permission_denied();
    HostFacts {
        os,
        docker_host: std::env::var("DOCKER_HOST").ok(),
        desktop_app_exists: find_desktop_app(os).is_some(),
        docker_cli_exists: docker_cli_exists(os),
        endpoint_permission_denied: ping_denied || socket_denied,
        ping_ok: false,
        ping_error,
    }
}

fn find_desktop_app(os: HostOs) -> Option<PathBuf> {
    match os {
        HostOs::Windows => {
            let exe = program_files_dir()
                .join("Docker")
                .join("Docker")
                .join("Docker Desktop.exe");
            exe.is_file().then_some(exe)
        }
        HostOs::Macos => {
            let app = PathBuf::from("/Applications/Docker.app");
            app.is_dir().then_some(app)
        }
        HostOs::Linux => linux_desktop_candidates()
            .into_iter()
            .find(|path| path.is_file()),
        HostOs::Other => None,
    }
}

fn linux_desktop_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = find_on_path(&["docker-desktop"]) {
        paths.push(found);
    }
    paths.push(PathBuf::from("/opt/docker-desktop/bin/docker-desktop"));
    paths.push(PathBuf::from("/usr/bin/docker-desktop"));
    paths.push(PathBuf::from("/usr/local/bin/docker-desktop"));
    paths
}

fn docker_cli_exists(os: HostOs) -> bool {
    let names: &[&str] = match os {
        HostOs::Windows => &["docker.exe"],
        HostOs::Macos | HostOs::Linux | HostOs::Other => &["docker"],
    };
    if find_on_path(names).is_some() {
        return true;
    }
    cli_candidates(os).iter().any(|path| path.is_file())
}

fn cli_candidates(os: HostOs) -> Vec<PathBuf> {
    match os {
        HostOs::Windows => vec![program_files_dir()
            .join("Docker")
            .join("Docker")
            .join("resources")
            .join("bin")
            .join("docker.exe")],
        HostOs::Macos => vec![
            PathBuf::from("/usr/local/bin/docker"),
            PathBuf::from("/opt/homebrew/bin/docker"),
            PathBuf::from("/Applications/Docker.app/Contents/Resources/bin/docker"),
        ],
        HostOs::Linux => vec![
            PathBuf::from("/usr/bin/docker"),
            PathBuf::from("/usr/local/bin/docker"),
        ],
        HostOs::Other => Vec::new(),
    }
}

fn program_files_dir() -> PathBuf {
    std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
}

fn find_on_path(names: &[&str]) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 只认「套接字在、但当前用户连不上」。管道不存在不能当成未安装，这里也不去打开 Windows 管道
/// （管道忙时 `File::open` 可能堵住）。
fn local_socket_permission_denied() -> bool {
    #[cfg(unix)]
    {
        let mut paths = vec![PathBuf::from("/var/run/docker.sock")];
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(
                PathBuf::from(home)
                    .join(".docker")
                    .join("run")
                    .join("docker.sock"),
            );
        }
        paths.into_iter().any(|path| {
            matches!(
                std::os::unix::net::UnixStream::connect(&path),
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied
            )
        })
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn spawn_detached(program: &Path, args: &[String]) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start Docker Desktop: {e}"))?;
    // macOS 的 `open` 很快退出；不回收会在长生命周期进程里留下僵尸。
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(os: HostOs) -> HostFacts {
        HostFacts {
            os,
            docker_host: None,
            desktop_app_exists: false,
            docker_cli_exists: false,
            endpoint_permission_denied: false,
            ping_ok: false,
            ping_error: Some(
                "Docker service unavailable or not running: Error in the hyper legacy client: client error (Connect)".into(),
            ),
        }
    }

    // Feature: docker-host-status, Property: ping 成功一律视为就绪，不再建议打开或安装。
    #[test]
    fn ping_ok_is_ready_even_if_other_flags_look_bad() {
        let mut input = facts(HostOs::Linux);
        input.ping_ok = true;
        input.ping_error = None;
        input.endpoint_permission_denied = true;
        input.docker_host = Some("tcp://10.0.0.8:2375".into());
        let report = build_report(&input);
        assert_eq!(report.kind, DockerHostKind::Ready);
        assert!(!report.can_open);
        assert!(report.detail.is_empty());
    }

    // Feature: docker-host-status, Property: 三个平台的安装页跟 OS 走，不跟本机环境走。
    #[test]
    fn install_url_follows_os() {
        assert!(docker_desktop_install_url(HostOs::Windows).contains("windows-install"));
        assert!(docker_desktop_install_url(HostOs::Macos).contains("mac-install"));
        assert!(docker_desktop_install_url(HostOs::Linux).contains("linux"));
        let mut input = facts(HostOs::Windows);
        let report = build_report(&input);
        assert_eq!(report.kind, DockerHostKind::NotInstalled);
        assert!(!report.can_open);
        assert!(report.install_url.contains("windows-install"));
        input.os = HostOs::Macos;
        assert!(build_report(&input).install_url.contains("mac-install"));
        input.os = HostOs::Linux;
        assert!(build_report(&input).install_url.contains("linux"));
    }

    // Feature: docker-host-status, Property: 没有任何 Desktop 程序和 docker 命令才是未安装。
    #[test]
    fn missing_binaries_are_not_installed() {
        let report = build_report(&facts(HostOs::Windows));
        assert_eq!(report.kind, DockerHostKind::NotInstalled);
        assert!(!report.can_open);
    }

    // Feature: docker-host-status, Property: 找到 Desktop 且引擎无响应时可以打开，仅有 CLI 时不能。
    #[test]
    fn desktop_stopped_can_open_but_engine_only_cannot() {
        let mut desktop = facts(HostOs::Windows);
        desktop.desktop_app_exists = true;
        let report = build_report(&desktop);
        assert_eq!(report.kind, DockerHostKind::InstalledStopped);
        assert!(report.can_open);

        let mut cli_only = facts(HostOs::Linux);
        cli_only.docker_cli_exists = true;
        let report = build_report(&cli_only);
        assert_eq!(report.kind, DockerHostKind::InstalledStopped);
        assert!(!report.can_open);
    }

    // Feature: docker-host-status, Property: 套接字拒绝访问时不提供打开或安装。
    #[test]
    fn permission_denied_hides_open_even_if_desktop_exists() {
        let mut input = facts(HostOs::Linux);
        input.desktop_app_exists = true;
        input.docker_cli_exists = true;
        input.endpoint_permission_denied = true;
        let report = build_report(&input);
        assert_eq!(report.kind, DockerHostKind::PermissionDenied);
        assert!(!report.can_open);
    }

    // Feature: docker-host-status, Property: 非本机默认 DOCKER_HOST 优先于权限和安装探测。
    #[test]
    fn custom_host_wins_over_permission_and_desktop() {
        let mut input = facts(HostOs::Macos);
        input.desktop_app_exists = true;
        input.endpoint_permission_denied = true;
        input.docker_host = Some("tcp://192.168.1.10:2375".into());
        let report = build_report(&input);
        assert_eq!(report.kind, DockerHostKind::CustomHost);
        assert!(!report.can_open);
    }

    #[test]
    fn local_default_hosts_are_not_custom() {
        let cases = [
            (HostOs::Windows, None),
            (HostOs::Windows, Some("")),
            (HostOs::Windows, Some("   ")),
            (HostOs::Windows, Some(r"npipe:////./pipe/docker_engine")),
            (HostOs::Windows, Some(r"\\.\pipe\docker_engine")),
            (
                HostOs::Windows,
                Some(r"npipe:////./pipe/dockerDesktopLinuxEngine"),
            ),
            (HostOs::Macos, Some("unix:///var/run/docker.sock")),
            (
                HostOs::Macos,
                Some("unix:///Users/wei/.docker/run/docker.sock"),
            ),
            (HostOs::Linux, Some("unix:///var/run/docker.sock")),
            (
                HostOs::Linux,
                Some("unix:///home/wei/.docker/desktop/docker.sock"),
            ),
            (HostOs::Linux, Some("/var/run/docker.sock")),
        ];
        for (os, host) in cases {
            assert!(
                !is_custom_docker_host(os, host),
                "{os:?} host {host:?} should be local"
            );
        }
    }

    #[test]
    fn remote_and_odd_hosts_are_custom() {
        let cases = [
            (HostOs::Windows, Some("tcp://127.0.0.1:2375")),
            (HostOs::Linux, Some("ssh://user@example.com")),
            (HostOs::Macos, Some("unix:///tmp/other.sock")),
            (HostOs::Windows, Some("unix:///var/run/docker.sock")),
        ];
        for (os, host) in cases {
            assert!(
                is_custom_docker_host(os, host),
                "{os:?} host {host:?} should be custom"
            );
        }
    }

    // Feature: docker-host-status, Property: 截图里的 hyper Connect 不是权限错误。
    #[test]
    fn hyper_connect_is_not_permission_denied() {
        let msg = "Error in the hyper legacy client: client error (Connect)";
        assert!(!error_is_permission_denied(msg));
        assert!(error_is_permission_denied(
            "Permission denied (os error 13) while connecting to docker.sock"
        ));
        assert!(error_is_permission_denied("Access is denied. (os error 5)"));
    }

    #[test]
    fn launch_spec_matches_platform() {
        let mac = desktop_launch(HostOs::Macos, Path::new("/Applications/Docker.app"));
        assert_eq!(mac.program, PathBuf::from("open"));
        assert_eq!(mac.args, vec!["-a".to_string(), "Docker".to_string()]);

        let exe = Path::new(r"C:\Program Files\Docker\Docker\Docker Desktop.exe");
        let windows = desktop_launch(HostOs::Windows, exe);
        assert_eq!(windows.program, exe);
        assert!(windows.args.is_empty());

        let bin = Path::new("/opt/docker-desktop/bin/docker-desktop");
        let linux = desktop_launch(HostOs::Linux, bin);
        assert_eq!(linux.program, bin);
        assert!(linux.args.is_empty());
    }

    #[test]
    fn kind_serializes_snake_case() {
        let mut input = facts(HostOs::Linux);
        input.docker_cli_exists = true;
        let json = serde_json::to_value(build_report(&input)).unwrap();
        assert_eq!(json["kind"], "installed_stopped");
        assert_eq!(json["can_open"], false);
        assert!(json["detail"].as_str().unwrap().contains("Connect"));
    }

    #[test]
    fn ready_report_skips_disk_probe() {
        let report = report_from_ping(None);
        assert_eq!(report.kind, DockerHostKind::Ready);
        assert!(!report.can_open);
        assert!(!report.install_url.is_empty());
    }
}
