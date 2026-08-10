//! Windows OCR 模块
//!
//! 使用 Windows.Media.Ocr 做离线文字识别。小尺寸选区会先无损放大；
//! 自动模式会尝试本机已安装的 OCR 引擎并选择最完整的结果。

/// 统一 OCR 入口。安装并启用 RapidOCR 后优先走增强离线识别；
/// 组件不可用或当前语言不受支持时再使用 Windows OCR。
pub fn recognize(
    app: &tauri::AppHandle,
    image_bytes: &[u8],
    source_lang: &str,
    use_rapidocr: bool,
) -> Result<String, String> {
    let mut rapid_error = None;
    if use_rapidocr && crate::rapidocr::supports_language(app, source_lang) {
        match crate::rapidocr::recognize(app, image_bytes) {
            Ok(text) => return Ok(text),
            Err(error) => {
                log_debug(&format!("RapidOCR 失败，回退 Windows OCR：{error}"));
                rapid_error = Some(error);
            }
        }
    }

    match recognize_windows(image_bytes, source_lang) {
        Ok(text) => Ok(text),
        Err(windows_error) => match rapid_error {
            Some(rapid_error) => Err(format!(
                "增强 OCR 失败：{rapid_error}；Windows OCR 也失败：{windows_error}"
            )),
            None => Err(windows_error),
        },
    }
}

#[cfg(debug_assertions)]
fn log_debug(msg: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("C:\\tyc\\templefix\\debug.log")
    {
        let _ = writeln!(file, "[OCR] {msg}");
    }
}

#[cfg(not(debug_assertions))]
fn log_debug(_msg: &str) {}

/// 小号文字在 Windows OCR 中容易直接返回空。把高度放大到约 120px，
/// 同时受 OCR 最大图片尺寸约束；全程用 PNG，避免二次有损压缩。
fn prepare_image(image_bytes: &[u8], max_dimension: u32) -> Result<Vec<u8>, String> {
    use image::ImageFormat;

    let image = image::load_from_memory(image_bytes).map_err(|e| format!("图片预处理失败：{e}"))?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 {
        return Err("图片尺寸无效".into());
    }

    let desired_scale = if height < 120 {
        (120.0 / height as f32).min(4.0)
    } else {
        1.0
    };
    let dimension_scale = max_dimension as f32 / width.max(height) as f32;
    let scale = desired_scale.min(dimension_scale).max(1.0);
    if scale <= 1.01 {
        return Ok(image_bytes.to_vec());
    }

    let new_width = (width as f32 * scale).round() as u32;
    let new_height = (height as f32 * scale).round() as u32;
    let resized = image.resize_exact(
        new_width,
        new_height,
        image::imageops::FilterType::Lanczos3,
    );
    let mut output = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|e| format!("图片预处理编码失败：{e}"))?;
    log_debug(&format!(
        "小图无损放大 {width}x{height} -> {new_width}x{new_height}"
    ));
    Ok(output.into_inner())
}

#[cfg(windows)]
pub fn recognize_windows(image_bytes: &[u8], source_lang: &str) -> Result<String, String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let max_dimension = OcrEngine::MaxImageDimension().unwrap_or(2600);
    let prepared = prepare_image(image_bytes, max_dimension)?;

    let stream = InMemoryRandomAccessStream::new().map_err(|e| format!("创建流失败：{e}"))?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(|e| format!("创建 writer 失败：{e}"))?;
    writer
        .WriteBytes(&prepared)
        .map_err(|e| format!("写入图片失败：{e}"))?;
    writer
        .StoreAsync()
        .map_err(|e| format!("存储图片失败：{e}"))?
        .get()
        .map_err(|e| format!("存储图片失败：{e}"))?;
    stream.Seek(0).map_err(|e| format!("图片流定位失败：{e}"))?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(|e| format!("创建图片解码器失败：{e}"))?
        .get()
        .map_err(|e| format!("图片解码失败：{e}"))?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(|e| format!("读取图片失败：{e}"))?
        .get()
        .map_err(|e| format!("读取图片失败：{e}"))?;

    let engines = make_engines(source_lang)?;
    let mut best_text = String::new();
    let mut best_score = 0usize;

    for (engine, tag) in engines {
        let text = recognize_with_engine(&engine, &bitmap)?;
        let score = text_score(&text);
        log_debug(&format!("引擎={tag} 字符={} 评分={score}", text.chars().count()));
        if score > best_score {
            best_score = score;
            best_text = text;
        }
        // 手动指定语言时只有一个引擎，不做无意义的额外尝试。
        if !source_lang.trim().is_empty() {
            break;
        }
    }

    Ok(best_text)
}

#[cfg(windows)]
fn recognize_with_engine(
    engine: &windows::Media::Ocr::OcrEngine,
    bitmap: &windows::Graphics::Imaging::SoftwareBitmap,
) -> Result<String, String> {
    let result = engine
        .RecognizeAsync(bitmap)
        .map_err(|e| format!("识别失败：{e}"))?
        .get()
        .map_err(|e| format!("识别失败：{e}"))?;
    let lines = result.Lines().map_err(|e| format!("读取识别结果失败：{e}"))?;
    let mut text = String::new();
    for line in &lines {
        if !text.is_empty() {
            text.push('\n');
        }
        let line_text = line.Text().map_err(|e| format!("读取识别文字失败：{e}"))?;
        text.push_str(&line_text.to_string());
    }
    Ok(text)
}

/// 手动指定时严格使用指定引擎；自动模式尝试用户首选引擎和全部已安装引擎。
#[cfg(windows)]
fn make_engines(
    source_lang: &str,
) -> Result<Vec<(windows::Media::Ocr::OcrEngine, String)>, String> {
    use std::collections::HashSet;
    use windows::Globalization::Language;
    use windows::Media::Ocr::OcrEngine;

    if !source_lang.trim().is_empty() {
        let tag = lang_to_tag(source_lang);
        if tag.is_empty() {
            return Err(format!("不支持的原文语言：{source_lang}"));
        }
        let language = Language::CreateLanguage(&windows::core::HSTRING::from(tag))
            .map_err(|e| format!("无法使用原文语言“{source_lang}”：{e}"))?;
        if !OcrEngine::IsLanguageSupported(&language).unwrap_or(false) {
            return Err(format!(
                "当前 OCR 不支持“{source_lang}”。请安装增强离线 OCR 组件，或改用已安装的语言。"
            ));
        }
        let engine = OcrEngine::TryCreateFromLanguage(&language)
            .map_err(|e| format!("创建“{source_lang}”OCR 引擎失败：{e}"))?;
        let actual_tag = engine_tag(&engine).unwrap_or_else(|| tag.to_string());
        return Ok(vec![(engine, actual_tag)]);
    }

    let mut engines = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
        let tag = engine_tag(&engine).unwrap_or_else(|| "user-profile".into());
        seen.insert(tag.to_lowercase());
        engines.push((engine, tag));
    }

    if let Ok(languages) = OcrEngine::AvailableRecognizerLanguages() {
        let count = languages.Size().unwrap_or(0);
        let mut available_tags = Vec::new();
        for index in 0..count {
            let Ok(language) = languages.GetAt(index) else {
                continue;
            };
            let Ok(tag_value) = language.LanguageTag() else {
                continue;
            };
            let tag = tag_value.to_string();
            available_tags.push(tag.clone());
            if seen.insert(tag.to_lowercase()) {
                if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language) {
                    engines.push((engine, tag));
                }
            }
        }
        log_debug(&format!("本机可用语言={}", available_tags.join(",")));
    }

    if engines.is_empty() {
        Err("Windows 没有可用的 OCR；请在首选项中安装增强离线 OCR 组件".into())
    } else {
        Ok(engines)
    }
}

#[cfg(windows)]
fn engine_tag(engine: &windows::Media::Ocr::OcrEngine) -> Option<String> {
    engine
        .RecognizerLanguage()
        .ok()?
        .LanguageTag()
        .ok()
        .map(|tag| tag.to_string())
}

/// 把产品里的语言名转成 Windows BCP-47 语言标签。
fn lang_to_tag(lang: &str) -> &'static str {
    match lang {
        "简体中文" => "zh-Hans-CN",
        "繁體中文" => "zh-Hant-TW",
        "English" => "en-US",
        "日本語" => "ja-JP",
        "한국어" => "ko-KR",
        "Français" => "fr-FR",
        "Português" => "pt-BR",
        "Español" => "es-ES",
        _ => "",
    }
}

fn text_score(text: &str) -> usize {
    let meaningful = text
        .chars()
        .filter(|c| c.is_alphanumeric() || (*c as u32) >= 0x2e80)
        .count();
    let whitespace = text.chars().filter(|c| c.is_whitespace()).count();
    meaningful.saturating_mul(4).saturating_sub(whitespace)
}

#[cfg(not(windows))]
pub fn recognize_windows(_image_bytes: &[u8], _source_lang: &str) -> Result<String, String> {
    Err("OCR 仅支持 Windows 平台".to_string())
}

#[cfg(test)]
mod tests {
    use super::{lang_to_tag, text_score};

    #[test]
    fn language_names_map_to_windows_tags() {
        assert_eq!(lang_to_tag("简体中文"), "zh-Hans-CN");
        assert_eq!(lang_to_tag("日本語"), "ja-JP");
        assert_eq!(lang_to_tag("English"), "en-US");
    }

    #[test]
    fn useful_ocr_text_scores_above_empty_or_spaced_noise() {
        assert!(text_score("播放列表") > text_score(""));
        assert!(text_score("Hello world") > text_score("  \n  "));
    }

    /// 本机 OCR 探针。平时没有环境变量时直接跳过；收尾测试时传入图片路径执行。
    #[cfg(windows)]
    #[test]
    fn recognizes_local_probe_image_when_requested() {
        let Ok(path) = std::env::var("TEMPLEFIX_OCR_TEST_IMAGE") else {
            return;
        };
        let source_lang = std::env::var("TEMPLEFIX_OCR_TEST_LANG").unwrap_or_default();
        let bytes = std::fs::read(path).expect("读取 OCR 探针图片失败");
        let text = super::recognize_windows(&bytes, &source_lang).expect("OCR 探针执行失败");
        assert!(!text.trim().is_empty(), "OCR 探针没有识别出任何文字");
        println!("OCR_PROBE_CHARS={}", text.chars().count());
    }
}
