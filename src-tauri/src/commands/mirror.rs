use crate::engine::mirror_manager::{MirrorManager as UnifiedMirrorManager, MirrorPreset};
use crate::engine::mirror_config_manager::{MirrorConfigManager, MergedMirrorCategory};

use super::get_project_root;

/// 标准化镜像源 URL（去除尾部斜杠）
fn normalize_mirror_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // 去除尾部斜杠，但保留协议部分的斜杠（如 https://）
    trimmed.trim_end_matches('/').to_string()
}

/// 获取所有镜像源预设方案
#[tauri::command]
pub fn get_mirror_presets() -> Result<Vec<MirrorPreset>, String> {
    Ok(UnifiedMirrorManager::get_presets())
}

/// 应用镜像源预设方案
#[tauri::command]
pub async fn apply_mirror_preset(preset_name: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    UnifiedMirrorManager::apply_preset(&preset_name, &env_path)
}

/// 更新单个镜像源类别
#[tauri::command]
pub fn update_single_mirror(category: String, source: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    
    // 标准化镜像源地址（去除尾部斜杠）
    let normalized_source = normalize_mirror_url(&source);
    
    UnifiedMirrorManager::update_single(&category, &normalized_source, &env_path)
}

/// 测试镜像源连接（3秒超时）
#[tauri::command]
pub async fn test_mirror(url: String) -> Result<bool, String> {
    UnifiedMirrorManager::test_connection(&url).await
}

/// 获取当前镜像源状态
#[tauri::command]
pub fn get_mirror_status() -> Result<serde_json::Value, String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    let status = UnifiedMirrorManager::get_current_status(&env_path)?;
    serde_json::to_value(&status)
        .map_err(|e| format!("序列化镜像源状态失败: {e}"))
}

/// 获取当前匹配的预设名称
#[tauri::command]
pub fn get_current_mirror_preset() -> Result<String, String> {
    let project_root = get_project_root()?;
    let env_path = project_root.join(".env");
    UnifiedMirrorManager::detect_current_preset(&env_path)
}

// ==================== 增强镜像源管理命令 ====================

/// 获取合并后的镜像源列表（默认配置 + 用户自定义）
#[tauri::command]
pub fn get_merged_mirror_list() -> Result<Vec<MergedMirrorCategory>, String> {
    let project_root = get_project_root()?;
    MirrorConfigManager::get_merged_mirror_list(&project_root)
}

/// 保存用户选择的镜像源选项
#[tauri::command]
pub fn save_selected_mirror_option(
    category_id: String,
    option_id: String,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    MirrorConfigManager::save_selected_option(&project_root, &category_id, &option_id)
}

/// 保存用户自定义的单个类别配置
#[tauri::command]
pub fn save_user_mirror_category(
    category_id: String,
    source: String,
    description: Option<String>,
) -> Result<(), String> {
    let project_root = get_project_root()?;
    
    // 标准化镜像源地址（去除尾部斜杠）
    let normalized_source = normalize_mirror_url(&source);
    
    MirrorConfigManager::save_user_category(&project_root, &category_id, &normalized_source, description)
}

/// 删除用户自定义的类别配置
#[tauri::command]
pub fn remove_user_mirror_category(category_id: String) -> Result<(), String> {
    let project_root = get_project_root()?;
    
    // 1. 从用户配置中删除
    MirrorConfigManager::remove_user_category(&project_root, &category_id)?;
    
    // 2. 同步更新 .env 文件，恢复为默认值
    let env_path = project_root.join(".env");
    let default_value = match category_id.as_str() {
        "docker_registry" => "",
        "apt" => "https://deb.debian.org/debian",
        "composer" => "https://packagist.org",
        "npm" => "https://registry.npmjs.org",
        "github_proxy" => "",
        _ => return Err(format!("未知的镜像源类别: {category_id}")),
    };
    
    UnifiedMirrorManager::update_single(&category_id, default_value, &env_path)
}

/// 重置所有用户自定义镜像源配置
#[tauri::command]
pub fn reset_all_mirror_overrides() -> Result<(), String> {
    let project_root = get_project_root()?;
    MirrorConfigManager::reset_all_overrides(&project_root)
}
