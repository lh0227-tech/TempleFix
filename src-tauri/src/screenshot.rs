//! 截图模块
//!
//! 负责：截全屏（存全局缓存）、按 CSS 坐标 + 窗口逻辑尺寸裁剪选区。
//! DPI 缩放：前端传 CSS 像素坐标，后端用 截图物理尺寸/窗口逻辑尺寸 算出 scale 换算后裁剪。

use image::ImageFormat;
use std::sync::Mutex;
use tauri::Manager;
use xcap::Monitor;

/// 写调试日志到文件（临时，定位截图问题用）
#[cfg(debug_assertions)]
fn log_debug(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("C:\\tyc\\templefix\\debug.log")
    {
        let _ = writeln!(f, "{msg}");
        let _ = f.flush();
    }
}

#[cfg(not(debug_assertions))]
fn log_debug(_msg: &str) {}

/// 全局截图缓存：开遮罩时截一次，选完区裁剪时复用，避免重复截图
pub struct ShotCache {
    pub image: Mutex<Option<image::RgbaImage>>,
}

pub fn new_cache() -> ShotCache {
    ShotCache {
        image: Mutex::new(None),
    }
}

/// 截全屏（主显示器），存入全局缓存，返回截图物理尺寸 (宽, 高)
pub fn capture_full(app: &tauri::AppHandle) -> Result<(u32, u32), String> {
    log_debug("capture_full 开始");
    // 截图失败时不能继续误用上一次的画面。
    *app.state::<ShotCache>().image.lock().unwrap() = None;
    let monitors = Monitor::all().map_err(|e| {
        let msg = format!("获取显示器失败：{e}");
        log_debug(&msg);
        msg
    })?;
    log_debug(&format!("找到 {} 个显示器", monitors.len()));
    let primary = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| {
            Monitor::all()
                .ok()
                .and_then(|ms| ms.into_iter().next())
        })
        .ok_or_else(|| "找不到显示器".to_string())?;

    log_debug("开始截屏...");
    let img = primary
        .capture_image()
        .map_err(|e| {
            let msg = format!("截图失败：{e}");
            log_debug(&msg);
            msg
        })?;

    let (w, h) = (img.width(), img.height());
    log_debug(&format!("截屏成功 {w}x{h}"));

    let state = app.state::<ShotCache>();
    *state.image.lock().unwrap() = Some(img);
    log_debug("已存入缓存");

    Ok((w, h))
}

/// 裁剪选区
pub fn crop_region(
    app: &tauri::AppHandle,
    css_x: i32,
    css_y: i32,
    css_w: i32,
    css_h: i32,
    win_w: f64,
    win_h: f64,
    shot_w: u32,
    shot_h: u32,
) -> Result<String, String> {
    log_debug(&format!(
        "crop_region: css=({css_x},{css_y},{css_w},{css_h}) win=({win_w},{win_h}) shot=({shot_w},{shot_h})"
    ));
    let state = app.state::<ShotCache>();
    let guard = state.image.lock().unwrap();
    let img = guard
        .as_ref()
        .ok_or_else(|| "截图缓存为空，请重新框选".to_string())?;
    log_debug(&format!("缓存图实际尺寸 {}x{}", img.width(), img.height()));

    let scale_x = shot_w as f64 / win_w;
    let scale_y = shot_h as f64 / win_h;

    // 换算成物理像素
    let px = ((css_x as f64) * scale_x).round().max(0.0) as i32;
    let py = ((css_y as f64) * scale_y).round().max(0.0) as i32;
    let pw = ((css_w as f64) * scale_x).round().max(1.0) as u32;
    let ph = ((css_h as f64) * scale_y).round().max(1.0) as u32;

    // 裁剪并夹紧到截图范围内
    let x0 = (px as u32).min(shot_w.saturating_sub(1));
    let y0 = (py as u32).min(shot_h.saturating_sub(1));
    let x1 = (x0 + pw).min(shot_w);
    let y1 = (y0 + ph).min(shot_h);
    if x1 <= x0 || y1 <= y0 {
        return Err("截图裁剪失败：选区无效".to_string());
    }

    let sub = image::imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0)
        .to_image();

    // RGBA 转 RGB，避免透明通道影响 OCR。
    let mut rgb = image::DynamicImage::ImageRgba8(sub).to_rgb8();

    // 大图自动缩放：宽超过 1000px 等比缩小（翻译不需要高清，省流量提速）
    const MAX_W: u32 = 1000;
    if rgb.width() > MAX_W {
        let ratio = MAX_W as f32 / rgb.width() as f32;
        let new_h = (rgb.height() as f32 * ratio).round() as u32;
        rgb = image::imageops::resize(
            &rgb,
            MAX_W,
            new_h,
            image::imageops::FilterType::Lanczos3,
        );
    }

    // OCR 主通道使用无损 PNG，避免小号中文被 JPEG 压缩成空结果。
    // 多模态请求会在 translate.rs 中另行压成 JPEG，不增加网络开销。
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(rgb)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("截图编码失败：{e}"))?;
    let b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        buf.into_inner(),
    );
    Ok(format!("data:image/png;base64,{b64}"))
}

/// 取截图的 SHA256（用于翻译缓存键）
pub fn shot_hash(data_uri: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data_uri.as_bytes());
    hex(&h.finalize())
}

/// 把整张截图转成 JPEG data URI 给前端（放大镜背景用）
pub fn full_data_uri(app: &tauri::AppHandle) -> Result<String, String> {
    let state = app.state::<ShotCache>();
    let guard = state.image.lock().unwrap();
    let img = guard
        .as_ref()
        .ok_or_else(|| "截图缓存为空".to_string())?;
    // 放大镜不需要原分辨率，缩小到一半省内存
    let resized = if img.width() > 2000 {
        image::imageops::resize(
            img,
            img.width() / 2,
            img.height() / 2,
            image::imageops::FilterType::Nearest,
        )
    } else {
        img.clone()
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    // RGBA 转 RGB：JPEG 不支持透明通道
    let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8();
    image::DynamicImage::ImageRgb8(rgb)
        .write_to(&mut buf, ImageFormat::Jpeg)
        .map_err(|e| format!("截图编码失败：{e}"))?;
    let b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        buf.into_inner(),
    );
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
