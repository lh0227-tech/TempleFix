//! 原生界面文案（系统托盘和窗口标题）。
//!
//! 网页界面的完整文案由 `src/js/i18n.js` 管理；这里仅保留无法由网页控制的
//! 系统级文字。菜单 id 始终不变，因此切换语言不会改变功能行为。

use crate::config::Config;

pub fn locale(config: &Config) -> &'static str {
    match config.ui_language.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-hans" => "zh-CN",
        "en" | "en-us" | "en-gb" => "en",
        "ja" | "ja-jp" => "ja",
        "fr" | "fr-fr" => "fr",
        "de" | "de-de" => "de",
        "es" | "es-es" => "es",
        "pt" | "pt-br" => "pt-BR",
        _ => match config.native_lang.as_str() {
            "简体中文" | "繁體中文" => "zh-CN",
            "日本語" => "ja",
            "Français" => "fr",
            "Deutsch" => "de",
            "Español" => "es",
            "Português" => "pt-BR",
            _ => "en",
        },
    }
}

pub fn text(config: &Config, key: &str) -> &'static str {
    match (locale(config), key) {
        ("zh-CN", "welcome") => "新手引导",
        ("zh-CN", "preferences") => "首选项",
        ("zh-CN", "quit") => "退出",
        ("zh-CN", "welcome_title") => "太阳穴 - 欢迎",
        ("zh-CN", "preferences_title") => "太阳穴 - 首选项",

        ("ja", "welcome") => "初期設定ガイド",
        ("ja", "preferences") => "設定",
        ("ja", "quit") => "終了",
        ("ja", "welcome_title") => "TempleFix - ようこそ",
        ("ja", "preferences_title") => "TempleFix - 設定",

        ("fr", "welcome") => "Guide de démarrage",
        ("fr", "preferences") => "Préférences",
        ("fr", "quit") => "Quitter",
        ("fr", "welcome_title") => "TempleFix - Bienvenue",
        ("fr", "preferences_title") => "TempleFix - Préférences",

        ("de", "welcome") => "Einrichtungsassistent",
        ("de", "preferences") => "Einstellungen",
        ("de", "quit") => "Beenden",
        ("de", "welcome_title") => "TempleFix - Willkommen",
        ("de", "preferences_title") => "TempleFix - Einstellungen",

        ("es", "welcome") => "Guía de inicio",
        ("es", "preferences") => "Preferencias",
        ("es", "quit") => "Salir",
        ("es", "welcome_title") => "TempleFix - Bienvenida",
        ("es", "preferences_title") => "TempleFix - Preferencias",

        ("pt-BR", "welcome") => "Guia inicial",
        ("pt-BR", "preferences") => "Preferências",
        ("pt-BR", "quit") => "Sair",
        ("pt-BR", "welcome_title") => "TempleFix - Boas-vindas",
        ("pt-BR", "preferences_title") => "TempleFix - Preferências",

        (_, "welcome") => "Setup guide",
        (_, "preferences") => "Preferences",
        (_, "quit") => "Quit",
        (_, "welcome_title") => "TempleFix - Welcome",
        (_, "preferences_title") => "TempleFix - Preferences",
        // 用户已明确要求保留这个测试入口的原名称。
        (_, "test") => "测试翻译",
        (_, "tooltip") => "太阳穴 TempleFix",
        _ => "TempleFix",
    }
}

pub fn update_available(config: &Config, version: &str) -> String {
    let prefix = match locale(config) {
        "zh-CN" => "发现新版本",
        "ja" => "新しいバージョン",
        "fr" => "Nouvelle version",
        "de" => "Neue Version",
        "es" => "Nueva versión",
        "pt-BR" => "Nova versão",
        _ => "Update available",
    };
    format!("{prefix} · {version}")
}

#[cfg(test)]
mod tests {
    use super::{locale, text, update_available};
    use crate::config::Config;

    #[test]
    fn explicit_interface_language_wins_over_translation_language() {
        let config = Config {
            ui_language: "de".into(),
            native_lang: "简体中文".into(),
            ..Config::default()
        };
        assert_eq!(locale(&config), "de");
        assert_eq!(text(&config, "preferences"), "Einstellungen");
    }

    #[test]
    fn legacy_config_uses_native_language_as_safe_fallback() {
        let config = Config {
            native_lang: "日本語".into(),
            ..Config::default()
        };
        assert_eq!(locale(&config), "ja");
    }

    #[test]
    fn test_translation_label_stays_unchanged() {
        let config = Config {
            ui_language: "fr".into(),
            ..Config::default()
        };
        assert_eq!(text(&config, "test"), "测试翻译");
    }

    #[test]
    fn update_menu_uses_the_interface_language() {
        let config = Config {
            ui_language: "en".into(),
            ..Config::default()
        };
        assert_eq!(
            update_available(&config, "2.0.0"),
            "Update available · 2.0.0"
        );
    }
}
