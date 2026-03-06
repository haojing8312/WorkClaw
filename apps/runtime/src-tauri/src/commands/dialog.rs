use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
#[cfg(target_os = "windows")]
use std::process::Command;

#[tauri::command]
pub async fn select_directory(
    app: AppHandle,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    let mut builder = app.dialog().file();

    if let Some(path) = default_path {
        builder = builder.set_directory(&path);
    }

    let result = builder.blocking_pick_folder();

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL 不能为空".to_string());
    }

    #[cfg(target_os = "windows")]
    let status = Command::new("cmd")
        .args(["/C", "start", "", trimmed])
        .status();

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(trimmed).status();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(trimmed).status();

    let status = status.map_err(|e| format!("打开外部链接失败: {}", e))?;
    if !status.success() {
        return Err(format!("打开外部链接失败: {}", trimmed));
    }
    Ok(())
}
