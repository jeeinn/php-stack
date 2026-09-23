//! 应用路径的唯一事实来源。
//!
//! 此前"开发模式向上爬 4 层 / 生产模式取 exe 父目录"这段逻辑被抄了三份：
//! `commands/mod.rs`（get_project_root）、`commands/workspace.rs`（export_logs）、
//! `lib.rs`（日志目录）。更关键的是产品问题——生产模式下 workspace.json 与
//! php-stack.log 写在 exe 同级目录，装进 `Program Files` 后无写权限，功能直接失效。
//!
//! 本模块收口三件事：
//! - `app_data_dir()`：用户级配置的唯一落点，由 setup 用 Tauri 官方
//!   `app.path().app_data_dir()` 注入，不再自己爬目录；
//! - `project_root()`：工作区目录（.env / services / 备份所在），语义不同，保持独立；
//! - `log_file()`：日志文件路径，随 app_data_dir 走。
//!
//! 约定：**工作区内的文件（.env、docker-compose.yml、services/、备份包、`.user-config/`）
//! 一律原地不动**——它们本来就是"工作区"的一部分，且备份/恢复管道按项目根读写。
//! 只有用户级配置与日志迁入 app_data_dir。

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::engine::workspace_manager::WorkspaceManager;

/// setup 阶段注入一次；未注入时回退到旧的 exe 同级逻辑（测试与早期调用）。
static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 由 `lib.rs` 的 setup 调用，传入 Tauri 官方应用数据目录。
///
/// 重复调用只保留首次值——路径在进程生命周期内必须稳定。
pub fn init_app_data_dir(dir: PathBuf) {
    let _ = APP_DATA_DIR.set(dir);
}

/// 回退路径：exe 同级（生产）或项目根（开发）。
///
/// 仅用于 `init_app_data_dir` 尚未执行的场景（单元测试、setup 之前的日志）。
fn legacy_app_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("failed to get executable path: {e}"))?;

    if cfg!(debug_assertions) {
        // 开发模式：src-tauri 的父目录 = 项目根
        exe.parent() // target/debug/
            .and_then(|p| p.parent()) // target/
            .and_then(|p| p.parent()) // src-tauri/
            .and_then(|p| p.parent()) // 项目根
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "failed to get project root".to_string())
    } else {
        exe.parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "failed to get executable directory".to_string())
    }
}

/// 用户级配置目录（workspace.json 等）。
pub fn app_data_dir() -> Result<PathBuf, String> {
    match APP_DATA_DIR.get() {
        Some(dir) => Ok(dir.clone()),
        None => legacy_app_dir(),
    }
}

/// 日志文件路径。
pub fn log_file() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("php-stack.log"))
}

/// 工作区解析结果。
///
/// `path` 是**实际生效**的落点；`fell_back` 为真表示配置的工作区用不了、
/// 数据正写到默认位置——这必须让上层和用户知道，不能静默发生。
#[derive(Debug, Clone)]
pub struct WorkspaceResolution {
    pub path: PathBuf,
    /// 配置的工作区不可用、已回退到默认目录
    pub fell_back: bool,
    /// 配置路径本身不存在（等待用户：重建 / 选新路径 / 临时回退）
    pub path_missing: bool,
    /// 回退原因（`fell_back` 为真时必有值）
    pub reason: Option<String>,
}

/// 解析工作区根目录（.env / docker-compose.yml / services/ 所在）。
///
/// 优先级：
/// 1. workspace.json 已配置且目录存在 → 直接用；
/// 2. 已配置但目录不存在 → **不自动创建**（避免盘符卸载/拼写错误时静默建空环境），
///    临时回退到默认目录，并标记 `path_missing`，由前端询问用户决策；
/// 3. 从未配置过 → 用默认位置，不算回退。
pub fn resolve_workspace() -> Result<WorkspaceResolution, String> {
    let Some(config) = WorkspaceManager::load_workspace()? else {
        return Ok(WorkspaceResolution {
            path: legacy_app_dir()?,
            fell_back: false,
            path_missing: false,
            reason: None,
        });
    };

    let path = PathBuf::from(&config.workspace_path);

    if path.exists() {
        return Ok(WorkspaceResolution {
            path,
            fell_back: false,
            path_missing: false,
            reason: None,
        });
    }

    // 目录不见了：不自动 create_dir_all。临时用默认目录保证读写不崩，
    // 同时标记 path_missing，让前端弹出「重建 / 选新路径 / 临时回退」。
    //
    // reason 传的是 i18n key（带 {path} 占位），由前端 t() 翻译后展示——
    // 这里拼自然语言的话，英文界面会看到中文原因。日志侧补上真实路径，
    // 避免出现一条只剩 key、看不出是哪个目录的日志。
    let reason_key = "workspace.reason.configuredMissing";
    eprintln!(
        "configured workspace {} does not exist ({reason_key}), \
         temporarily falling back to default dir",
        config.workspace_path
    );
    Ok(WorkspaceResolution {
        path: legacy_app_dir()?,
        fell_back: true,
        path_missing: true,
        reason: Some(reason_key.to_string()),
    })
}

/// 按用户确认，把配置的工作区目录重建出来。
///
/// 仅在用户明确点「重建」时调用；成功后后续 `resolve_workspace` 会命中该路径。
pub fn recreate_configured_workspace() -> Result<PathBuf, String> {
    let config = WorkspaceManager::load_workspace()?
        .ok_or_else(|| "workspace is not configured yet".to_string())?;
    let path = PathBuf::from(&config.workspace_path);
    create_workspace_dir(&path)?;
    Ok(path)
}

/// 创建工作区目录（纯路径操作，便于单测）。
pub fn create_workspace_dir(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(path)
        .map_err(|e| format!("failed to create workspace dir {}: {e}", path.display()))
}

/// 工作区根目录（.env / docker-compose.yml / services/ 所在）。
///
/// 只取路径；需要知道是否发生回退时用 [`resolve_workspace`]。
pub fn project_root() -> Result<PathBuf, String> {
    Ok(resolve_workspace()?.path)
}

/// 需要随 app_data_dir 迁移的用户级配置文件名。
const MIGRATABLE_FILES: [&str; 1] = ["workspace.json"];

/// 把 `from_dir` 中缺失于 `to_dir` 的用户级配置复制过去。
///
/// 纯函数、可单测：只在目标不存在时复制，绝不覆盖已有配置。
/// 返回被迁移的文件名列表。
fn migrate_files(from_dir: &std::path::Path, to_dir: &std::path::Path) -> Vec<String> {
    if from_dir == to_dir {
        return Vec::new();
    }

    if let Err(e) = std::fs::create_dir_all(to_dir) {
        eprintln!("failed to create app data dir {}: {e}", to_dir.display());
        return Vec::new();
    }

    let mut migrated = Vec::new();
    for name in MIGRATABLE_FILES {
        let from = from_dir.join(name);
        let to = to_dir.join(name);

        if !from.exists() || to.exists() {
            continue;
        }

        match std::fs::copy(&from, &to) {
            Ok(_) => migrated.push(name.to_string()),
            Err(e) => eprintln!("failed to migrate {}: {e}", from.display()),
        }
    }

    migrated
}

/// 首次启动时把旧位置的用户级配置迁移到 app_data_dir。
///
/// 复制失败仅记录，不阻断启动——迁移是便利手段，不是前置条件。
/// 返回被迁移的文件名列表，供启动日志提示用户。
pub fn migrate_legacy_config() -> Vec<String> {
    let (Ok(target_dir), Ok(legacy_dir)) = (app_data_dir(), legacy_app_dir()) else {
        return Vec::new();
    };

    migrate_files(&legacy_dir, &target_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 建一个干净的临时目录（已存在则先清空）
    fn fresh_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("php-stack-paths-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_log_file_lives_in_app_data_dir() {
        let dir = app_data_dir().expect("app_data_dir 不应失败");
        let log = log_file().expect("log_file 不应失败");
        assert_eq!(log.parent(), Some(dir.as_path()));
        assert_eq!(log.file_name().unwrap(), "php-stack.log");
    }

    #[test]
    fn test_project_root_is_absolute() {
        let root = project_root().expect("project_root 不应失败");
        assert!(root.is_absolute(), "project_root 应为绝对路径: {root:?}");
    }

    #[test]
    fn test_migrate_files_copies_when_missing() {
        let legacy = fresh_dir("legacy");
        let target = fresh_dir("target");
        std::fs::write(legacy.join("workspace.json"), r#"{"workspace_path":"/x"}"#).unwrap();

        let migrated = migrate_files(&legacy, &target);

        assert_eq!(migrated, vec!["workspace.json".to_string()]);
        let copied = std::fs::read_to_string(target.join("workspace.json")).unwrap();
        assert_eq!(copied, r#"{"workspace_path":"/x"}"#);
    }

    #[test]
    fn test_migrate_files_does_not_overwrite_existing() {
        let legacy = fresh_dir("legacy2");
        let target = fresh_dir("target2");
        std::fs::write(legacy.join("workspace.json"), "OLD").unwrap();
        std::fs::write(target.join("workspace.json"), "NEW").unwrap();

        assert!(migrate_files(&legacy, &target).is_empty());
        // 新位置的配置必须原封不动
        assert_eq!(
            std::fs::read_to_string(target.join("workspace.json")).unwrap(),
            "NEW"
        );
    }

    #[test]
    fn test_migrate_files_noop_for_same_dir() {
        let dir = fresh_dir("same");
        std::fs::write(dir.join("workspace.json"), "X").unwrap();
        assert!(migrate_files(&dir, &dir).is_empty());
    }

    #[test]
    fn test_migrate_files_noop_when_nothing_to_migrate() {
        let legacy = fresh_dir("legacy3");
        let target = fresh_dir("target3");
        assert!(migrate_files(&legacy, &target).is_empty());
        assert!(!Path::new(&target.join("workspace.json")).exists());
    }

    #[test]
    fn test_project_root_never_falls_back_silently_when_unconfigured() {
        // 未配置工作区时用默认目录，属于预期行为，不算回退
        let root = project_root().expect("project_root 不应失败");
        assert!(root.is_absolute(), "默认目录应为绝对路径: {root:?}");
    }

    #[test]
    fn test_create_workspace_dir_creates_missing_path() {
        let dir = fresh_dir("create-ws").join("nested-new");
        assert!(!dir.exists());
        create_workspace_dir(&dir).expect("应能创建缺失目录");
        assert!(dir.is_dir());
        // 幂等：已存在再调一次不报错
        create_workspace_dir(&dir).expect("已存在时应空操作成功");
    }

    #[test]
    fn test_create_workspace_dir_rejects_impossible_path() {
        // 用不存在的盘符（Windows）或根下无权限路径模拟失败
        #[cfg(windows)]
        let bad = PathBuf::from(r"Z:\php-stack-no-such-drive-xyz\ws");
        #[cfg(not(windows))]
        let bad = PathBuf::from("/proc/php-stack-cannot-create/ws");

        let err = create_workspace_dir(&bad).expect_err("不可能的路径应失败");
        assert!(
            err.contains("failed to create workspace dir"),
            "错误信息应说明创建失败: {err}"
        );
        assert!(!bad.exists());
    }
}
