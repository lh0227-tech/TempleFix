//! 配置模块
//!
//! 用 tauri-plugin-store 持久化（JSON 文件）。
//! 配置项：提供商、Base URL、API Key、模型名、目标语言、快捷键。

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";

/// 提供商信息
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub api_format: &'static str,
    pub default_base_url: &'static str,
}

/// 五家提供商（文档 3.1 节）
pub static PROVIDERS: &[Provider] = &[
    Provider {
        id: "doubao",
        name: "豆包（火山引擎）",
        api_format: "openai",
        default_base_url: "https://ark.cn-beijing.volces.com/api/v3",
    },
    Provider {
        id: "openai",
        name: "OpenAI",
        api_format: "openai",
        default_base_url: "https://api.openai.com/v1",
    },
    Provider {
        id: "gemini",
        name: "Gemini",
        api_format: "openai",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    },
    Provider {
        id: "anthropic",
        name: "Anthropic",
        api_format: "anthropic",
        default_base_url: "https://api.anthropic.com/v1",
    },
    Provider {
        id: "custom",
        name: "自定义",
        api_format: "openai",
        default_base_url: "",
    },
];

/// 完整配置
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    // ===== 语言模型（主力：OCR提文本后用它纠错+翻译）=====
    pub llm_provider: String,   // 语言模型提供商 id
    pub llm_base_url: String,   // API Base URL
    pub llm_api_key: String,    // API Key
    pub llm_model: String,      // 模型名称

    // ===== 多模态（保底：OCR不行时发图翻译，可选）=====
    pub use_multimodal: bool,           // 是否启用多模态保底
    pub mm_provider: String,            // 多模态提供商 id
    pub mm_base_url: String,            // API Base URL
    pub mm_api_key: String,             // API Key
    pub mm_model: String,               // 模型名称

    // ===== 通用 =====
    pub native_lang: String,           // 用户母语（默认翻译目标）
    pub target_lang: String,           // 可覆盖的目标语言，空则用母语
    pub source_lang: String,           // 手动指定源语言，空则自动识别
    pub use_rapidocr: bool,            // 安装增强离线 OCR 后优先使用
    pub hotkey: String,                 // 快捷键
    pub multimodal_threshold: u32,      // OCR文本短于此字符数时提示用多模态（默认5）
}

impl Default for Config {
    fn default() -> Self {
        Config {
            llm_provider: "custom".into(),
            llm_base_url: "https://api.deepseek.com".into(),
            llm_api_key: "".into(),
            llm_model: "deepseek-v4-flash".into(),

            use_multimodal: false,
            mm_provider: "doubao".into(),
            mm_base_url: "https://ark.cn-beijing.volces.com/api/v3".into(),
            mm_api_key: "".into(),
            mm_model: "".into(),

            native_lang: "简体中文".into(),
            target_lang: "".into(),
            source_lang: "".into(),
            use_rapidocr: true,
            hotkey: "Alt+Z".into(),
            multimodal_threshold: 5,
        }
    }
}

/// 读取配置（读不到返回默认）
pub fn load(app: &AppHandle) -> Config {
    let store = app.store(STORE_FILE);
    match store {
        Ok(s) => {
            let v = s.get("config");
            match v {
                Some(val) => serde_json::from_value(val).unwrap_or_default(),
                None => Config::default(),
            }
        }
        Err(_) => Config::default(),
    }
}

/// 保存配置
pub fn save(app: &AppHandle, cfg: &Config) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("打开存储失败：{e}"))?;
    let val = serde_json::to_value(cfg).map_err(|e| format!("序列化失败：{e}"))?;
    store.set("config", val);
    store.save().map_err(|e| format!("保存失败：{e}"))?;
    Ok(())
}

/// 根据提供商 id 查 API 格式（openai / anthropic）
pub fn api_format(provider_id: &str) -> &'static str {
    PROVIDERS
        .iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.api_format)
        .unwrap_or("openai")
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn old_config_keeps_existing_values_when_new_fields_are_missing() {
        let old_config = serde_json::json!({
            "llm_provider": "custom",
            "llm_base_url": "https://example.com",
            "llm_api_key": "existing-key",
            "llm_model": "existing-model",
            "native_lang": "English",
            "multimodal_threshold": 2
        });

        let config: Config = serde_json::from_value(old_config).unwrap();

        assert_eq!(config.llm_api_key, "existing-key");
        assert_eq!(config.llm_model, "existing-model");
        assert_eq!(config.native_lang, "English");
        assert_eq!(config.multimodal_threshold, 2);
        assert_eq!(config.source_lang, "");
        assert!(config.use_rapidocr);
    }
}
