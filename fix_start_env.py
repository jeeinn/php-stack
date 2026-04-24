import re

# 读取文件
with open('e:/study/php-stack/src-tauri/src/commands.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# 找到 start_environment 函数的范围
start_pattern = r'pub async fn start_environment\(app_handle: tauri::AppHandle\)'
end_pattern = r'^\}$'

# 使用正则表达式找到函数体
match = re.search(r'(pub async fn start_environment\(app_handle: tauri::AppHandle\) -> Result<String, String> \{)(.*?)(^\}$)', content, re.DOTALL | re.MULTILINE)

if match:
    func_start = match.group(1)
    func_body = match.group(2)
    func_end = match.group(3)
    
    # 替换 use 语句
    func_body = func_body.replace('use tauri::Emitter;', 'use crate::{app_log, ui_log};')
    
    # 删除 emit_log 辅助函数定义
    func_body = re.sub(r'\s*// 辅助函数：发送日志到前端并打印到终端\s*let emit_log = \|msg: &str\| \{[^}]+\};', '', func_body)
    
    # 替换所有 emit_log 调用为 ui_log! 宏
    # 模式1: emit_log("message")
    func_body = re.sub(r'emit_log\("([^"]+)"\);', r'ui_log!(app_handle, info, "commands::start_environment", "\1");', func_body)
    
    # 模式2: emit_log(&format!("message {}", var))
    func_body = re.sub(r'emit_log\(&format!\("([^"]+)"(?:,\s*([^)]+))?\)\);', 
                       lambda m: f'ui_log!(app_handle, info, "commands::start_environment", "{m.group(1)}"{", " + m.group(2) if m.group(2) else ""});', 
                       func_body)
    
    # 重建函数
    new_func = func_start + func_body + func_end
    
    # 替换原函数
    content = content[:match.start()] + new_func + content[match.end():]
    
    # 写入文件
    with open('e:/study/php-stack/src-tauri/src/commands.rs', 'w', encoding='utf-8') as f:
        f.write(content)
    
    print("✅ start_environment 函数重构完成！")
else:
    print("❌ 未找到 start_environment 函数")
