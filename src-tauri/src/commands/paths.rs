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
//! 约定：**工作区内的文件（.env、docker-compose.yml、services/、备份包、.user_*.json）
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
    let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;

    if cfg!(debug_assertions) {
        // 开发模式：src-tauri 的父目录 = 项目根
        exe.parent() // target/debug/
            .and_then(|p| p.parent()) // target/
            .and_then(|p| p.parent()) // src-tauri/
            .and_then(|p| p.parent()) // 项目根
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "无法获取项目根目录".to_string())
    } else {
        exe.parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "无法获取程序所在目录".to_string())
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

/// 工作区根目录（.env / docker-compose.yml / services/ 所在）。
///
/// 优先读 workspace.json 中已配置的路径；未配置或路径失效时回退到
/// 可执行文件附近的目录。
pub fn project_root() -> Result<PathBuf, String> {
    // 1. 尝试从 workspace.json 读取配置
    if let Some(workspace) = WorkspaceManager::load_workspace()? {
        let path = PathBuf::from(&workspace.workspace_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. 未配置或路径无效：开发模式用项目根，生产模式用 exe 同级
    legacy_app_dir()
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
        eprintln!("无法创建应用数据目录 {}: {e}", to_dir.display());
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
            Err(e) => eprintln!("迁移 {} 失败: {e}", from.display()),
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
}
