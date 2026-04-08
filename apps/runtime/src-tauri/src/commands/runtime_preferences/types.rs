pub(crate) const KEY_RUNTIME_DEFAULT_WORK_DIR: &str = "runtime_default_work_dir";
pub(crate) const KEY_RUNTIME_DEFAULT_LANGUAGE: &str = "runtime_default_language";
pub(crate) const KEY_RUNTIME_IMMERSIVE_TRANSLATION_ENABLED: &str =
    "runtime_immersive_translation_enabled";
pub(crate) const KEY_RUNTIME_IMMERSIVE_TRANSLATION_DISPLAY: &str =
    "runtime_immersive_translation_display";
pub(crate) const KEY_RUNTIME_IMMERSIVE_TRANSLATION_TRIGGER: &str =
    "runtime_immersive_translation_trigger";
pub(crate) const KEY_RUNTIME_TRANSLATION_ENGINE: &str = "runtime_translation_engine";
pub(crate) const KEY_RUNTIME_TRANSLATION_MODEL_ID: &str = "runtime_translation_model_id";
pub(crate) const KEY_RUNTIME_LAUNCH_AT_LOGIN: &str = "runtime_launch_at_login";
pub(crate) const KEY_RUNTIME_LAUNCH_MINIMIZED: &str = "runtime_launch_minimized";
pub(crate) const KEY_RUNTIME_CLOSE_TO_TRAY: &str = "runtime_close_to_tray";
pub(crate) const KEY_RUNTIME_OPERATION_PERMISSION_MODE: &str =
    "runtime_operation_permission_mode";

pub(crate) const DEFAULT_LANGUAGE: &str = "zh-CN";
pub(crate) const DEFAULT_IMMERSIVE_TRANSLATION_ENABLED: bool = true;
pub(crate) const DEFAULT_IMMERSIVE_TRANSLATION_DISPLAY: &str = "translated_only";
pub(crate) const DEFAULT_IMMERSIVE_TRANSLATION_TRIGGER: &str = "auto";
pub(crate) const DEFAULT_TRANSLATION_ENGINE: &str = "model_then_free";
pub(crate) const DEFAULT_LAUNCH_AT_LOGIN: bool = false;
pub(crate) const DEFAULT_LAUNCH_MINIMIZED: bool = false;
pub(crate) const DEFAULT_CLOSE_TO_TRAY: bool = true;
pub(crate) const DEFAULT_OPERATION_PERMISSION_MODE: &str = "standard";
pub(crate) const AUTOSTART_NAME: &str = crate::branding_generated::AUTOSTART_NAME;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferences {
    pub default_work_dir: String,
    pub default_language: String,
    pub immersive_translation_enabled: bool,
    pub immersive_translation_display: String,
    pub immersive_translation_trigger: String,
    pub translation_engine: String,
    pub translation_model_id: String,
    pub launch_at_login: bool,
    pub launch_minimized: bool,
    pub close_to_tray: bool,
    pub operation_permission_mode: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferencesInput {
    pub default_work_dir: Option<String>,
    pub default_language: Option<String>,
    pub immersive_translation_enabled: Option<bool>,
    pub immersive_translation_display: Option<String>,
    pub immersive_translation_trigger: Option<String>,
    pub translation_engine: Option<String>,
    pub translation_model_id: Option<String>,
    pub launch_at_login: Option<bool>,
    pub launch_minimized: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub operation_permission_mode: Option<String>,
}
