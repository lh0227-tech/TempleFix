//! 翻译调度层
//!
//! 【合规说明】
//! 本文件负责所有对外模型 API 调用。严格遵守产品合规要求：
//! 1. 前端无任何代理 UI；本层也不提供任何代理参数。
//! 2. reqwest 不设置显式代理，仅默认跟随操作系统环境变量
//!    （HTTP_PROXY / HTTPS_PROXY / ALL_PROXY）。
//! 3. 海外模型访问由用户自行通过系统全局代理解决，本程序不提供翻墙辅助。
//!
//! 统一返回 TranslateResult。支持两类 API 格式：
//! - openai 兼容：豆包 / OpenAI / Gemini / 自定义
//! - anthropic 兼容：Anthropic

use crate::config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// 写调试日志到文件（定位翻译问题用，带时间戳）
#[cfg(debug_assertions)]
fn log_debug(msg: &str) {
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            format!("{}:{:03}", secs % 1000, d.subsec_millis())
        })
        .unwrap_or_default();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("C:\\tyc\\templefix\\debug.log")
    {
        let _ = writeln!(f, "[{ts}] {msg}");
        let _ = f.flush();
    }
}

#[cfg(not(debug_assertions))]
fn log_debug(_msg: &str) {}

fn preview(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SelectionContext {
    pub css_x: f64,
    pub css_y: f64,
    pub css_w: f64,
    pub css_h: f64,
    pub win_w: f64,
    pub win_h: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OverlayLine {
    pub text: String,
    pub rect: crate::ocr::OcrRect,
}

/// 统一返回格式（文档 3.3）
#[derive(Serialize, Deserialize, Clone)]
pub struct TranslateResult {
    pub success: bool,
    pub original: String,
    pub translated: String,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlay_lines: Vec<OverlayLine>,
}

impl TranslateResult {
    fn ok(original: String, translated: String, model: String) -> Self {
        TranslateResult {
            success: true,
            original,
            translated,
            error: None,
            error_code: None,
            model,
            overlay_lines: Vec::new(),
        }
    }
    pub fn err(code: &str, msg: String, model: String) -> Self {
        TranslateResult {
            success: false,
            original: String::new(),
            translated: String::new(),
            error: Some(msg),
            error_code: Some(code.into()),
            model,
            overlay_lines: Vec::new(),
        }
    }

    fn err_with_original(code: &str, msg: String, model: String, original: String) -> Self {
        TranslateResult {
            success: false,
            original,
            translated: String::new(),
            error: Some(msg),
            error_code: Some(code.into()),
            model,
            overlay_lines: Vec::new(),
        }
    }
}

/// 翻译缓存（内存）：key = 截图哈希 + 目标语言
pub struct TranslateCache {
    map: Mutex<HashMap<String, TranslateResult>>,
}

impl TranslateCache {
    pub fn new() -> Self {
        TranslateCache {
            map: Mutex::new(HashMap::new()),
        }
    }
    fn get(&self, key: &str) -> Option<TranslateResult> {
        self.map.lock().unwrap().get(key).cloned()
    }
    fn set(&self, key: String, val: TranslateResult) {
        self.map.lock().unwrap().insert(key, val);
    }
    pub fn clear(&self) {
        self.map.lock().unwrap().clear();
    }
}

/// 最近一次翻译结果（供浮窗窗口加载时取，跨窗口传递）
pub struct LastResult {
    pub result: Mutex<Option<TranslateResult>>,
    pub data_uri: Mutex<Option<String>>,
    pub target_lang: Mutex<Option<String>>,
    pub selection: Mutex<Option<SelectionContext>>,
}

impl LastResult {
    pub fn new() -> Self {
        LastResult {
            result: Mutex::new(None),
            data_uri: Mutex::new(None),
            target_lang: Mutex::new(None),
            selection: Mutex::new(None),
        }
    }
    pub fn set(
        &self,
        result: TranslateResult,
        data_uri: String,
        target_lang: String,
        selection: Option<SelectionContext>,
    ) {
        *self.result.lock().unwrap() = Some(result);
        *self.data_uri.lock().unwrap() = Some(data_uri);
        *self.target_lang.lock().unwrap() = Some(target_lang);
        *self.selection.lock().unwrap() = selection;
    }
}

/// 翻译参数
pub struct TranslateParams {
    pub data_uri: String,    // 截图 data URI
    pub target_lang: String, // 目标语言（空则用母语，由调用方处理）
}

/// 顶层翻译入口：OCR优先 -> DeepSeek纠错翻译 -> 短文本且开多模态则提示
pub async fn translate(app: &AppHandle, params: TranslateParams) -> TranslateResult {
    let cfg = config::load(app);
    // 目标语言：target_lang 空就用母语
    let target_lang = if params.target_lang.trim().is_empty() {
        cfg.native_lang.clone()
    } else {
        params.target_lang.clone()
    };
    log_debug(&format!(
        "translate 入口: data_uri长度={} 目标语言={}",
        params.data_uri.len(),
        target_lang
    ));

    // 缓存键（同一张图+目标语言只算一次）
    let hash = crate::screenshot::shot_hash(&params.data_uri);
    let cache_key = format!("{hash}|{target_lang}");
    {
        let cache = app.state::<TranslateCache>();
        if let Some(mut hit) = cache.get(&cache_key) {
            hit.model = format!("{}（缓存）", hit.model);
            return hit;
        }
    }

    // ===== 主力：OCR提文本 -> DeepSeek纠错翻译 =====
    // 1. 把 data_uri 解码成图片字节
    let img_bytes = match decode_data_uri(&params.data_uri) {
        Ok(b) => b,
        Err(e) => return TranslateResult::err("OCR", format!("图片解码失败：{e}"), "".into()),
    };

    // 2. OCR 提取文本（在阻塞线程跑，避免卡 async runtime）
    let source_lang = cfg.source_lang.clone();
    let use_rapidocr = cfg.use_rapidocr;
    let (ocr_result, ocr_error) = {
        let bytes = img_bytes.clone();
        let sl = source_lang.clone();
        let worker_app = app.clone();
        match tokio::task::spawn_blocking(move || {
            crate::ocr::recognize(&worker_app, &bytes, &sl, use_rapidocr)
        })
        .await
        {
            Ok(Ok(result)) => {
                log_debug(&format!(
                    "OCR结果: {:?} ({}字符)",
                    preview(&result.text, 60),
                    result.text.chars().count()
                ));
                (result, None)
            }
            Ok(Err(e)) => {
                log_debug(&format!("OCR失败: {e}"));
                (crate::ocr::OcrResult::default(), Some(e))
            }
            Err(e) => {
                log_debug(&format!("OCR线程失败: {e}"));
                (
                    crate::ocr::OcrResult::default(),
                    Some(format!("OCR 线程失败：{e}")),
                )
            }
        }
    };

    // 3. OCR 结果为空 -> 直接走多模态保底（如果开了）
    if ocr_result.text.trim().is_empty() {
        if cfg.use_multimodal {
            if cfg.mm_api_key.trim().is_empty() {
                return TranslateResult::err(
                    "NO_MM_KEY",
                    "OCR 没识别到文字，但多模态保底未填写 API Key。请在设置中补充，或关闭多模态保底。".into(),
                    cfg.mm_model,
                );
            }
            if cfg.mm_model.trim().is_empty() {
                return TranslateResult::err(
                    "NO_MM_MODEL",
                    "OCR 没识别到文字，但多模态保底未填写模型名称。请在设置中补充。".into(),
                    cfg.mm_model,
                );
            }
            log_debug("OCR空，走多模态保底");
            let result = multimodal_translate(app, &params).await;
            if result.success {
                app.state::<TranslateCache>().set(cache_key, result.clone());
            }
            return result;
        }
        if let Some(error) = ocr_error {
            return TranslateResult::err("OCR_UNAVAILABLE", error, "".into());
        }
        return TranslateResult::err(
            "OCR_EMPTY",
            "没识别到文字。可在设置中开启多模态保底来识别复杂图片。".into(),
            "".into(),
        );
    }

    // 4. OCR 结果很短且开了多模态 -> 提示用户选
    let char_count = ocr_result.text.chars().count() as u32;
    if char_count < cfg.multimodal_threshold
        && cfg.use_multimodal
        && !cfg.mm_api_key.trim().is_empty()
    {
        // 返回特殊状态：有OCR结果但太短，前端提示是否用多模态
        return TranslateResult {
            success: true,
            original: ocr_result.text,
            translated: "（识别到的文字较少，可能不准。点击「用大模型重试」用多模态重新识别）"
                .into(),
            error: Some("SHORT_OCR".into()),
            error_code: Some("SHORT_OCR".into()),
            model: "ocr".into(),
            overlay_lines: Vec::new(),
        };
    }

    // 5. 正常：DeepSeek 纠错+翻译
    let mut r = llm_translate(app, &ocr_result.text, &target_lang, &source_lang).await;
    if r.success {
        r.overlay_lines =
            align_overlay_lines(&ocr_result.lines, &r.original, &r.translated).unwrap_or_default();
        let cache = app.state::<TranslateCache>();
        cache.set(cache_key, r.clone());
    }
    r
}

fn align_overlay_lines(
    ocr_lines: &[crate::ocr::OcrLine],
    corrected_original: &str,
    translated: &str,
) -> Option<Vec<OverlayLine>> {
    if ocr_lines.is_empty() || ocr_lines.len() > 80 {
        return None;
    }
    let original_lines = corrected_original
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>();
    let translated_lines = translated.lines().map(str::trim).collect::<Vec<_>>();
    if original_lines.len() != ocr_lines.len()
        || translated_lines.len() != ocr_lines.len()
        || translated_lines.iter().any(|line| line.is_empty())
    {
        return None;
    }
    ocr_lines
        .iter()
        .zip(translated_lines)
        .map(|(source, text)| {
            let rect = source
                .rect
                .clone()
                .filter(crate::ocr::OcrRect::is_reliable)?;
            Some(OverlayLine {
                text: text.to_string(),
                rect,
            })
        })
        .collect()
}

/// 把 data URI 解码成原始图片字节
fn decode_data_uri(data_uri: &str) -> Result<Vec<u8>, String> {
    let b64 = data_uri.split(',').nth(1).unwrap_or("");
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        .map_err(|e| format!("{e}"))
}

/// 语言模型纠错+翻译（主力）
async fn llm_translate(
    app: &AppHandle,
    ocr_text: &str,
    target_lang: &str,
    source_lang: &str,
) -> TranslateResult {
    let cfg = config::load(app);

    llm_translate_with_config(&cfg, ocr_text, target_lang, source_lang).await
}

/// 使用给定配置执行一次文字翻译。欢迎向导的连接测试与正式翻译共用这条路径，
/// 避免出现“测试成功，但实际翻译走的是另一套请求格式”。
async fn llm_translate_with_config(
    cfg: &config::Config,
    ocr_text: &str,
    target_lang: &str,
    source_lang: &str,
) -> TranslateResult {
    if cfg.llm_api_key.trim().is_empty() {
        return TranslateResult::err_with_original(
            "NO_KEY",
            "尚未配置 AI 翻译服务".into(),
            cfg.llm_model.clone(),
            ocr_text.into(),
        );
    }
    if cfg.llm_model.trim().is_empty() {
        return TranslateResult::err_with_original(
            "NO_MODEL",
            "尚未配置模型名称".into(),
            cfg.llm_model.clone(),
            ocr_text.into(),
        );
    }
    if cfg.llm_base_url.trim().is_empty() {
        return TranslateResult::err_with_original(
            "NO_BASE_URL",
            "尚未配置接口地址".into(),
            cfg.llm_model.clone(),
            ocr_text.into(),
        );
    }

    let prompt = build_llm_prompt(ocr_text, target_lang, source_lang);
    let mut result = match config::api_format(&cfg.llm_provider) {
        "anthropic" => call_anthropic_text(cfg, &prompt).await,
        _ => call_openai_text(cfg, &prompt).await,
    };
    if !result.success && result.original.is_empty() {
        result.original = ocr_text.into();
    }
    result
}

async fn call_openai_text(cfg: &config::Config, prompt: &str) -> TranslateResult {
    let url = format!(
        "{}/chat/completions",
        cfg.llm_base_url.trim_end_matches('/')
    );
    log_debug(&format!("llm_translate: url={url} model={}", cfg.llm_model));

    let mut body = serde_json::json!({
        "model": cfg.llm_model,
        "messages": [{"role":"user","content":prompt}],
        "temperature": 0.2
    });
    if cfg.llm_base_url.contains("deepseek.com") || cfg.llm_model.starts_with("deepseek-") {
        body["thinking"] = serde_json::json!({"type":"disabled"});
    }

    let client = build_client();
    let start = std::time::Instant::now();
    let mut last_err = None;
    for attempt in 1..=2 {
        log_debug(&format!("llm_translate: 第{attempt}次发送"));
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", cfg.llm_api_key))
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await;
        match resp {
            Err(e) => {
                log_debug(&format!(
                    "llm_translate: 第{attempt}次出错 {:.1}s {e}",
                    start.elapsed().as_secs_f64()
                ));
                last_err = Some(e);
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
            Ok(r) => {
                log_debug(&format!(
                    "llm_translate: 收到响应 status={} 耗时{:.1}s",
                    r.status(),
                    start.elapsed().as_secs_f64()
                ));
                let res = parse_llm_resp(&cfg.llm_model, r).await;
                log_debug(&format!(
                    "llm_translate: 解析完成 耗时{:.1}s",
                    start.elapsed().as_secs_f64()
                ));
                return res;
            }
        }
    }
    map_network_err(&cfg.llm_model, last_err.unwrap())
}

async fn call_anthropic_text(cfg: &config::Config, prompt: &str) -> TranslateResult {
    let url = format!("{}/messages", cfg.llm_base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": cfg.llm_model,
        "max_tokens": 2048,
        "messages": [{"role":"user","content":prompt}]
    });
    let client = build_client();
    let mut last_err = None;
    for attempt in 1..=2 {
        let response = client
            .post(&url)
            .header("x-api-key", &cfg.llm_api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await;
        match response {
            Ok(response) => return parse_anthropic_resp(&cfg.llm_model, response).await,
            Err(error) => {
                last_err = Some(error);
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }
    map_network_err(&cfg.llm_model, last_err.unwrap())
}

/// 欢迎向导使用的真实连接测试。
pub async fn test_llm_config(cfg: &config::Config) -> Result<String, String> {
    let result = llm_translate_with_config(cfg, "Temple", "简体中文", "English").await;
    if result.success {
        Ok(result.model)
    } else {
        Err(result.error.unwrap_or_else(|| "连接测试失败".into()))
    }
}

async fn parse_llm_resp(model: &str, r: reqwest::Response) -> TranslateResult {
    let status = r.status().as_u16();
    if !r.status().is_success() {
        let txt = r.text().await.unwrap_or_default();
        log_debug(&format!("parse_llm_resp: 非2xx status={status} body={txt}"));
        return map_http_err(model, status, &txt);
    }
    let v: serde_json::Value = match r.json().await {
        Ok(v) => v,
        Err(e) => return TranslateResult::err("PARSE", format!("返回解析失败：{e}"), model.into()),
    };
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    log_debug(&format!(
        "parse_llm_resp: content={}",
        preview(&content, 100)
    ));
    parse_content(model, &content)
}

fn build_llm_prompt(ocr_text: &str, target_lang: &str, source_lang: &str) -> String {
    // 源语言说明：手动指定就告诉模型，否则让模型自动识别
    let source_hint = if source_lang.is_empty() {
        "先判断原文是什么语言".to_string()
    } else {
        format!("原文是{source_lang}")
    };
    format!(
        "下面是OCR识别结果，常有错误：字母数字混淆(0/O、1/l/I、5/S)、中英文混杂乱码、符号错认、断字粘连。\n\
         请你作为纠错专家：\n\
         1. {source_hint}，纠正所有OCR错误，还原成通顺正确的原文（中英混杂的按主要语言整理）\n\
         2. 保留原文的换行结构\n\
         3. 再把纠正后的原文翻译成{lang}；译文必须逐行对应原文，原文在哪里换行，译文就在哪里换行，行数尽量一致\n\
         4. 当原文与目标语言不同时，品牌主体名称可以保留原文，但产品类型、功能名称、标题和普通词必须翻译；不要因为短语包含品牌名或像专有名词，就把整行原样返回\n\
         严格只输出一行JSON，不要markdown代码块、不要解释：\n\
         {{\"original\":\"纠正后的原文\",\"translated\":\"译文\"}}\n\n{ocr_text}",
        lang = target_lang
    )
}

/// 公开接口：只走多模态（绕过OCR，用户手动重试用）
pub async fn multimodal_only(app: &AppHandle, params: &TranslateParams) -> TranslateResult {
    multimodal_translate(app, params).await
}

/// 多模态发图翻译（保底）
async fn multimodal_translate(app: &AppHandle, params: &TranslateParams) -> TranslateResult {
    let cfg = config::load(app);
    let model = cfg.mm_model.clone();
    // 目标语言：空则用母语
    let target_lang = if params.target_lang.trim().is_empty() {
        cfg.native_lang.clone()
    } else {
        params.target_lang.clone()
    };

    if cfg.mm_api_key.trim().is_empty() {
        return TranslateResult::err(
            "NO_MM_KEY",
            "未配置多模态 API Key，请在设置中填写".into(),
            model,
        );
    }
    if cfg.mm_model.trim().is_empty() {
        return TranslateResult::err(
            "NO_MM_MODEL",
            "未配置多模态模型名称，请在设置中填写".into(),
            model,
        );
    }

    // 压图：宽>600等比缩小，JPEG质量65
    let compressed = match compress_for_multimodal(&params.data_uri) {
        Ok(d) => d,
        Err(e) => return TranslateResult::err("OCR", format!("图片压缩失败：{e}"), model),
    };

    let mm_params = TranslateParams {
        data_uri: compressed,
        target_lang,
    };
    call_api(&cfg, &mm_params).await
}

/// 多模态用：压图（宽>600缩小，JPEG质量65）
pub fn compress_for_multimodal(data_uri: &str) -> Result<String, String> {
    let bytes = decode_data_uri(data_uri)?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("{e}"))?;
    let mut rgb = img.to_rgb8();
    if rgb.width() > 600 {
        let ratio = 600.0 / rgb.width() as f32;
        let new_h = (rgb.height() as f32 * ratio).round() as u32;
        rgb = image::imageops::resize(&rgb, 600, new_h, image::imageops::FilterType::Lanczos3);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 65);
    image::DynamicImage::ImageRgb8(rgb)
        .write_with_encoder(encoder)
        .map_err(|e| format!("{e}"))?;
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

/// 根据 API 格式分发（多模态保底用）
async fn call_api(cfg: &config::Config, params: &TranslateParams) -> TranslateResult {
    let format = config::api_format(&cfg.mm_provider);
    match format {
        "anthropic" => call_anthropic(cfg, params).await,
        _ => call_openai(cfg, params).await,
    }
}

/// 取出 data URI 里 base64 部分（去掉 "data:image/jpeg;base64," 前缀）
fn b64_part(data_uri: &str) -> &str {
    data_uri.split(',').nth(1).unwrap_or("")
}

/// 生成多模态翻译提示词
fn prompt(target_lang: &str, source_lang: &str) -> String {
    let source_hint = if source_lang.trim().is_empty() {
        "请先判断原文语言".to_string()
    } else {
        format!("原文语言是{source_lang}")
    };
    format!(
         "请把图片中的文字翻译成{lang}。要求：\n\
         1. {source_hint}，准确识别图中的原文\n\
         2. 再给出翻译；译文必须逐行对应原文的换行，行数尽量一致\n\
         3. 当原文与目标语言不同时，品牌主体名称可以保留原文，但产品类型、功能名称、标题和普通词必须翻译；不要因为短语包含品牌名或像专有名词，就把整行原样返回\n\
         严格按下面 JSON 格式输出，只输出 JSON，不要任何解释、不要 markdown 代码块：\n\
         {{\"original\":\"识别到的原文\",\"translated\":\"翻译结果\"}}",
        lang = target_lang
    )
}

// ===== OpenAI 兼容格式 =====
async fn call_openai(cfg: &config::Config, params: &TranslateParams) -> TranslateResult {
    let url = format!("{}/chat/completions", cfg.mm_base_url.trim_end_matches('/'));
    log_debug(&format!("call_openai: url={url} model={}", cfg.mm_model));
    let body = serde_json::json!({
        "model": cfg.mm_model,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": prompt(&params.target_lang, &cfg.source_lang)
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": params.data_uri
                    }
                }
            ]
        }],
        "temperature": 0.2,
        // 关闭思考模式：截图翻译不需要推理，关掉快一倍（实测 8.5s->4.4s）
        "thinking": { "type": "disabled" }
    });

    let client = build_client();

    // 发送请求，失败重试 1 次（网络不稳/大图传输断开时）
    let mut last_err = None;
    for attempt in 1..=2 {
        let start = std::time::Instant::now();
        log_debug(&format!(
            "call_openai: 第 {attempt} 次发送请求 data_uri={}字节",
            params.data_uri.len()
        ));
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", cfg.mm_api_key))
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await;

        match resp {
            Err(e) => {
                log_debug(&format!(
                    "call_openai: 第 {attempt} 次出错 {:.1}s {e}",
                    start.elapsed().as_secs_f64()
                ));
                last_err = Some(e);
                // 网络层错误才重试，等 1 秒
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
            Ok(r) => {
                log_debug(&format!(
                    "call_openai: 收到响应 status={} 耗时 {:.1}s",
                    r.status(),
                    start.elapsed().as_secs_f64()
                ));
                let parse_start = std::time::Instant::now();
                let res = parse_openai_resp(&cfg.mm_model, r).await;
                log_debug(&format!(
                    "call_openai: 解析完成 耗时 {:.1}s",
                    parse_start.elapsed().as_secs_f64()
                ));
                return res;
            }
        }
    }
    // 两次都失败
    map_network_err(&cfg.mm_model, last_err.unwrap())
}

async fn parse_openai_resp(model: &str, r: reqwest::Response) -> TranslateResult {
    let status = r.status().as_u16();
    // 非 2xx -> 错误
    if !r.status().is_success() {
        let txt = r.text().await.unwrap_or_default();
        log_debug(&format!(
            "parse_openai_resp: 非2xx status={status} body={txt}"
        ));
        return map_http_err(model, status, &txt);
    }
    let v: serde_json::Value = match r.json().await {
        Ok(v) => v,
        Err(e) => {
            log_debug(&format!("parse_openai_resp: json解析失败 {e}"));
            return TranslateResult::err("PARSE", format!("返回解析失败：{e}"), model.into());
        }
    };
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    log_debug(&format!(
        "parse_openai_resp: content={}",
        preview(&content, 200)
    ));
    parse_content(model, &content)
}

// ===== Anthropic 兼容格式 =====
async fn call_anthropic(cfg: &config::Config, params: &TranslateParams) -> TranslateResult {
    let url = format!("{}/messages", cfg.mm_base_url.trim_end_matches('/'));
    let b64 = b64_part(&params.data_uri);
    let body = serde_json::json!({
        "model": cfg.mm_model,
        "max_tokens": 2048,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/jpeg",
                        "data": b64
                    }
                },
                {
                    "type": "text",
                    "text": prompt(&params.target_lang, &cfg.source_lang)
                }
            ]
        }]
    });

    let client = build_client();
    let mut last_err = None;
    for attempt in 1..=2 {
        let resp = client
            .post(&url)
            .header("x-api-key", &cfg.mm_api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await;

        match resp {
            Err(e) => {
                last_err = Some(e);
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
            Ok(r) => return parse_anthropic_resp(&cfg.mm_model, r).await,
        }
    }
    map_network_err(&cfg.mm_model, last_err.unwrap())
}

async fn parse_anthropic_resp(model: &str, r: reqwest::Response) -> TranslateResult {
    let status = r.status().as_u16();
    if !r.status().is_success() {
        let txt = r.text().await.unwrap_or_default();
        return map_http_err(model, status, &txt);
    }
    let v: serde_json::Value = match r.json().await {
        Ok(v) => v,
        Err(e) => return TranslateResult::err("PARSE", format!("返回解析失败：{e}"), model.into()),
    };
    // content 是数组，每个元素有 text
    let content = v["content"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c["text"].as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    parse_content(model, &content)
}

/// 把模型返回的文本内容解析成 original/translated
fn parse_content(model: &str, content: &str) -> TranslateResult {
    let content = content.trim();
    // 尝试：直接解析 -> 去代码块 -> 提取{}包围的JSON
    let parsed = parse_json_content(content)
        .or_else(|| parse_json_content(&strip_code_fence(content)))
        .or_else(|| parse_json_content(&extract_json_braces(content)));

    if let Some((orig, trans)) = parsed {
        return TranslateResult::ok(orig, trans, model.into());
    }
    // 解析失败：把整段当译文，原文留空
    TranslateResult::err(
        "PARSE",
        "模型返回格式异常，未能解析原文和译文".into(),
        model.into(),
    )
}

/// 从内容里提取第一个 { 到最后一个 } 之间的部分
fn extract_json_braces(s: &str) -> String {
    let start = match s.find('{') {
        Some(i) => i,
        None => return s.to_string(),
    };
    let end = match s.rfind('}') {
        Some(i) => i + 1,
        None => return s.to_string(),
    };
    if end > start {
        s[start..end].to_string()
    } else {
        s.to_string()
    }
}

/// 从 JSON 文本里取 original 和 translated
fn parse_json_content(s: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let orig = v["original"].as_str().unwrap_or("").to_string();
    let trans = v["translated"].as_str().unwrap_or("").to_string();
    if orig.is_empty() && trans.is_empty() {
        return None;
    }
    Some((orig, trans))
}

/// 去掉 markdown 代码块 ```json ... ```
fn strip_code_fence(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("```") {
        let inner = s
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        return inner.to_string();
    }
    s.to_string()
}

// ===== 错误映射（文档第四节）=====
fn map_http_err(model: &str, status: u16, _body: &str) -> TranslateResult {
    let (code, msg) = match status {
        401 | 403 => ("AUTH", "API Key 无效，请在设置中重新配置"),
        402 => ("QUOTA", "API 额度已耗尽"),
        429 => ("RATE", "请求太频繁，请 30 秒后再试"),
        408 => ("TIMEOUT", "连接超时，请检查网络或代理设置"),
        _ => ("HTTP", "无法连接到模型服务，请检查网络"),
    };
    TranslateResult::err(code, msg.into(), model.into())
}

fn map_network_err(model: &str, e: reqwest::Error) -> TranslateResult {
    let msg = if e.is_timeout() {
        "连接超时，请检查网络或代理设置"
    } else if e.is_connect() {
        "无法连接到模型服务，请检查网络"
    } else {
        "网络请求失败，请检查网络或代理设置"
    };
    TranslateResult::err("NET", msg.into(), model.into())
}

/// 构造 reqwest 客户端
/// 【合规】不设置显式代理，仅默认跟随系统环境变量 HTTP_PROXY/HTTPS_PROXY/ALL_PROXY
fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[cfg(test)]
mod tests {
    use super::{
        align_overlay_lines, build_llm_prompt, compress_for_multimodal, llm_translate_with_config,
        parse_content, preview, prompt,
    };
    use base64::Engine;

    #[test]
    fn unicode_preview_never_slices_inside_a_character() {
        assert_eq!(preview("播放列表abc", 3), "播放列");
    }

    #[test]
    fn parses_plain_and_fenced_model_json() {
        let plain = parse_content("model", r#"{"original":"hello","translated":"你好"}"#);
        assert!(plain.success);
        assert_eq!(plain.original, "hello");

        let fenced = parse_content(
            "model",
            "```json\n{\"original\":\"寺庙\",\"translated\":\"temple\"}\n```",
        );
        assert!(fenced.success);
        assert_eq!(fenced.translated, "temple");
    }

    #[test]
    fn prompts_include_manual_source_language() {
        let text_prompt = build_llm_prompt("原文", "English", "简体中文");
        assert!(text_prompt.contains("原文是简体中文"));
        assert!(text_prompt.contains("产品类型、功能名称、标题和普通词必须翻译"));
        let image_prompt = prompt("English", "日本語");
        assert!(image_prompt.contains("原文语言是日本語"));
        assert!(image_prompt.contains("产品类型、功能名称、标题和普通词必须翻译"));
    }

    #[test]
    fn overlay_alignment_requires_exact_lines_and_reliable_rectangles() {
        let lines = vec![
            crate::ocr::OcrLine {
                text: "Hello".into(),
                rect: Some(crate::ocr::OcrRect {
                    left: 0.1,
                    top: 0.2,
                    width: 0.3,
                    height: 0.1,
                }),
            },
            crate::ocr::OcrLine {
                text: "World".into(),
                rect: Some(crate::ocr::OcrRect {
                    left: 0.1,
                    top: 0.4,
                    width: 0.4,
                    height: 0.1,
                }),
            },
        ];
        let aligned = align_overlay_lines(&lines, "Hello\nWorld", "你好\n世界").unwrap();
        assert_eq!(aligned.len(), 2);
        assert_eq!(aligned[1].text, "世界");
        assert!(align_overlay_lines(&lines, "Hello\nWorld", "你好，世界").is_none());

        let mut missing_rect = lines;
        missing_rect[0].rect = None;
        assert!(align_overlay_lines(&missing_rect, "Hello\nWorld", "你好\n世界").is_none());
    }

    #[tokio::test]
    async fn missing_key_keeps_recognized_text_for_ocr_only_mode() {
        let config = crate::config::Config::default();
        let result =
            llm_translate_with_config(&config, "recognized text", "简体中文", "English").await;

        assert!(!result.success);
        assert_eq!(result.error_code.as_deref(), Some("NO_KEY"));
        assert_eq!(result.original, "recognized text");
    }

    #[test]
    fn multimodal_image_is_jpeg_and_limited_to_600_pixels() {
        let image = image::RgbImage::from_pixel(1200, 100, image::Rgb([255, 255, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let data_uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png.into_inner())
        );

        let compressed = compress_for_multimodal(&data_uri).unwrap();
        assert!(compressed.starts_with("data:image/jpeg;base64,"));
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(compressed.split_once(',').unwrap().1)
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 600);
    }
}
