mod config;
mod ocr;
mod rapidocr;
mod screenshot;
mod translate;

use std::sync::Mutex;
use std::time::Instant;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
};
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// 写调试日志到文件（定位问题用）
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

/// 防抖状态：记录上次触发时间
struct State {
    last_trigger: Mutex<Option<Instant>>,
    last_esc: Mutex<Option<Instant>>,
}

#[cfg(desktop)]
fn default_hotkey() -> Shortcut {
    "Alt+Z".parse().expect("默认快捷键必须有效")
}

#[cfg(desktop)]
fn parse_hotkey(value: &str) -> Result<Shortcut, String> {
    let value = value.trim();
    let shortcut: Shortcut = value
        .parse()
        .map_err(|_| "快捷键格式不正确，例如 Alt+Z 或 Ctrl+Shift+X".to_string())?;
    let esc: Shortcut = "Escape".parse().expect("ESC 快捷键必须有效");
    if shortcut == esc {
        return Err("ESC 已用于关闭窗口，不能设为截图快捷键".into());
    }
    if shortcut.mods.is_empty() {
        return Err("截图快捷键至少要包含 Ctrl、Alt、Shift 或 Win 中的一个".into());
    }
    Ok(shortcut)
}

fn should_prevent_exit(code: Option<i32>) -> bool {
    // 没有退出码：通常只是最后一个窗口被关，托盘程序应继续常驻。
    // 有退出码：来自托盘菜单 app.exit(0)，必须允许真正退出。
    code.is_none()
}

fn clamp_popup_size(
    requested_width: f64,
    requested_height: f64,
    monitor_width: f64,
    monitor_height: f64,
) -> (f64, f64) {
    let max_width = 720.0_f64.min((monitor_width - 32.0).max(240.0));
    let max_height = 760.0_f64.min((monitor_height - 64.0).max(160.0));
    let min_width = 360.0_f64.min(max_width);
    let min_height = 180.0_f64.min(max_height);
    (
        requested_width.clamp(min_width, max_width),
        requested_height.clamp(min_height, max_height),
    )
}

/// 触发截图（带 500ms 防抖，不用永久锁，避免首次失败卡死）
fn trigger_screenshot(app: &tauri::AppHandle) {
    let state = app.state::<State>();
    let now = Instant::now();
    let mut last = state.last_trigger.lock().unwrap();
    if let Some(t) = *last {
        if now.duration_since(t).as_millis() < 500 {
            return; // 500ms 内重复触发，忽略
        }
    }
    *last = Some(now);
    drop(last);
    open_overlay(app);
}

/// 隐藏截图遮罩窗口（窗口常驻复用，避免重试时在命令里创建窗口而死锁）
#[tauri::command]
fn close_overlay(app: tauri::AppHandle) {
    log_debug("close_overlay 被调用");
    if let Some(win) = app.get_webview_window("overlay") {
        log_debug("close_overlay: 隐藏 overlay");
        let _ = win.hide();
    } else {
        log_debug("close_overlay: overlay 不存在");
    }
}

/// 取截图物理尺寸（前端用来算 DPI 缩放）
#[tauri::command]
fn get_shot_size(app: tauri::AppHandle) -> Result<(u32, u32), String> {
    let state = app.state::<screenshot::ShotCache>();
    let guard = state.image.lock().unwrap();
    match guard.as_ref() {
        Some(img) => Ok((img.width(), img.height())),
        None => Err("截图缓存为空".to_string()),
    }
}

/// 裁剪选区，返回 data URI（传给翻译）
#[tauri::command]
fn crop_region(
    app: tauri::AppHandle,
    css_x: i32,
    css_y: i32,
    css_w: i32,
    css_h: i32,
    win_w: f64,
    win_h: f64,
    shot_w: u32,
    shot_h: u32,
) -> Result<String, String> {
    screenshot::crop_region(&app, css_x, css_y, css_w, css_h, win_w, win_h, shot_w, shot_h)
}

/// 取整张截图 data URI（放大镜背景用）
#[tauri::command]
fn get_full_image(app: tauri::AppHandle) -> Result<String, String> {
    screenshot::full_data_uri(&app)
}

/// 读取配置
#[tauri::command]
fn get_config(app: tauri::AppHandle) -> config::Config {
    config::load(&app)
}

/// 保存配置；快捷键变化时立即重注册，失败则保留旧配置和旧快捷键。
#[tauri::command]
async fn save_config(app: tauri::AppHandle, cfg: config::Config) -> Result<(), String> {
    let old_cfg = config::load(&app);

    #[cfg(desktop)]
    let shortcut_change = {
        let new_shortcut = parse_hotkey(&cfg.hotkey)?;
        let old_shortcut = parse_hotkey(&old_cfg.hotkey).unwrap_or_else(|_| default_hotkey());
        if new_shortcut == old_shortcut {
            None
        } else {
            let manager = app.global_shortcut();
            manager
                .register(new_shortcut)
                .map_err(|e| format!("快捷键注册失败，可能已被其他程序占用：{e}"))?;
            if manager.is_registered(old_shortcut) {
                if let Err(e) = manager.unregister(old_shortcut) {
                    let _ = manager.unregister(new_shortcut);
                    return Err(format!("旧快捷键注销失败：{e}"));
                }
            }
            Some((old_shortcut, new_shortcut))
        }
    };

    if let Err(e) = config::save(&app, &cfg) {
        #[cfg(desktop)]
        if let Some((old_shortcut, new_shortcut)) = shortcut_change {
            let manager = app.global_shortcut();
            let _ = manager.unregister(new_shortcut);
            let _ = manager.register(old_shortcut);
        }
        return Err(e);
    }

    app.state::<translate::TranslateCache>().clear();
    log_debug(&format!("配置保存成功，快捷键={}", cfg.hotkey));
    Ok(())
}

/// 取提供商列表（前端下拉用）
#[tauri::command]
fn list_providers() -> Vec<serde_json::Value> {
    config::PROVIDERS
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "default_base_url": p.default_base_url,
            })
        })
        .collect()
}

/// 读取 RapidOCR 选装组件状态。
#[tauri::command]
fn rapidocr_status(app: tauri::AppHandle) -> rapidocr::RapidOcrStatus {
    rapidocr::status(&app)
}

/// 读取可自动下载的 RapidOCR 发布信息。
#[tauri::command]
fn rapidocr_release_info() -> rapidocr::RapidOcrReleaseInfo {
    rapidocr::release_info()
}

/// 从已配置的国内主源自动下载、校验并安装 RapidOCR。
#[tauri::command]
async fn download_install_rapidocr_component(
    app: tauri::AppHandle,
) -> Result<rapidocr::RapidOcrStatus, String> {
    let status = rapidocr::download_and_install(&app).await?;
    app.state::<translate::TranslateCache>().clear();
    Ok(status)
}

/// 高级入口：从用户选择的本地 ZIP 安装 RapidOCR。
#[tauri::command]
async fn install_rapidocr_component_from_file(
    app: tauri::AppHandle,
) -> Result<Option<rapidocr::RapidOcrStatus>, String> {
    use tauri_plugin_dialog::DialogExt;

    let selected = app
        .dialog()
        .file()
        .add_filter("TempleFix RapidOCR 组件包", &["zip"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| "无法读取所选组件包路径".to_string())?;
    let installed = rapidocr::install_local_package_with_events(&app, path).await?;
    app.state::<translate::TranslateCache>().clear();
    Ok(Some(installed))
}

/// 卸载 RapidOCR 选装组件。
#[tauri::command]
fn uninstall_rapidocr_component(
    app: tauri::AppHandle,
) -> Result<rapidocr::RapidOcrStatus, String> {
    let status = rapidocr::uninstall(&app)?;
    app.state::<translate::TranslateCache>().clear();
    Ok(status)
}

/// 翻译：传截图 data URI + 目标语言（空则用母语）
#[tauri::command]
async fn translate(
    app: tauri::AppHandle,
    data_uri: String,
    target_lang: String,
) -> translate::TranslateResult {
    let cfg = config::load(&app);
    let effective_target = if target_lang.trim().is_empty() {
        cfg.native_lang
    } else {
        target_lang.clone()
    };
    let params = translate::TranslateParams {
        data_uri: data_uri.clone(),
        target_lang: target_lang.clone(),
    };
    let result = translate::translate(&app, params).await;
    log_debug(&format!(
        "translate 命令完成: success={} translated={}",
        result.success, result.translated
    ));
    // 存到最近结果，供浮窗加载时取
    let last = app.state::<translate::LastResult>();
    last.set(result.clone(), data_uri, effective_target);
    log_debug("translate: 已存入 LastResult，返回");
    result
}

/// 取最近一次翻译结果（浮窗加载时调）
#[tauri::command]
fn get_last_result(app: tauri::AppHandle) -> serde_json::Value {
    let last = app.state::<translate::LastResult>();
    let result = last.result.lock().unwrap().clone();
    let target_lang = last.target_lang.lock().unwrap().clone();
    log_debug(&format!(
        "get_last_result 被调用: result_is_some={} success={} translated={}",
        result.is_some(),
        result.as_ref().map(|r| r.success).unwrap_or(false),
        result.as_ref().map(|r| r.translated.clone()).unwrap_or_default()
    ));
    serde_json::json!({
        "result": result,
        "target_lang": target_lang,
    })
}

/// 显示翻译结果浮窗（预创建的，只 show 不 build，避免命令里 build 死锁）
#[tauri::command]
fn open_popup(app: tauri::AppHandle) {
    log_debug("open_popup 被调用");
    if let Some(win) = app.get_webview_window("popup") {
        log_debug("open_popup: show popup");
        let _ = win.show();
        // 通知浮窗刷新数据（预创建浮窗 JS 只跑一次，show 后需主动刷新）
        let _ = app.emit_to("popup", "refresh-result", ());
    } else {
        log_debug("open_popup: popup 不存在（应在 setup 预创建）");
    }
}

/// 隐藏浮窗（不销毁，复用）
#[tauri::command]
fn hide_popup(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("popup") {
        let _ = win.hide();
    }
}

/// 按内容调整浮窗大小，并尽量保持浮窗中心位置不跳动。
#[tauri::command]
fn resize_popup(app: tauri::AppHandle, width: f64, height: f64) -> Result<(f64, f64), String> {
    let win = app
        .get_webview_window("popup")
        .ok_or_else(|| "翻译浮窗不存在".to_string())?;
    let scale = win.scale_factor().unwrap_or(1.0);
    let monitor = win
        .current_monitor()
        .map_err(|e| format!("读取显示器失败：{e}"))?
        .or_else(|| win.primary_monitor().ok().flatten());

    let (monitor_width, monitor_height) = monitor
        .as_ref()
        .map(|m| {
            let size = m.size().to_logical::<f64>(scale);
            (size.width, size.height)
        })
        .unwrap_or((1920.0, 1080.0));
    let (width, height) = clamp_popup_size(width, height, monitor_width, monitor_height);

    let old_position = win.outer_position().ok();
    let old_size = win.outer_size().ok();
    win.set_size(LogicalSize::new(width, height))
        .map_err(|e| format!("调整浮窗大小失败：{e}"))?;

    if let (Some(monitor), Some(old_position), Some(old_size)) = (monitor, old_position, old_size) {
        let new_width = (width * scale).round() as i32;
        let new_height = (height * scale).round() as i32;
        let center_x = old_position.x + old_size.width as i32 / 2;
        let center_y = old_position.y + old_size.height as i32 / 2;
        let min_x = monitor.position().x;
        let min_y = monitor.position().y;
        let max_x = min_x + monitor.size().width as i32 - new_width;
        let max_y = min_y + monitor.size().height as i32 - new_height;
        let x = (center_x - new_width / 2).clamp(min_x, max_x.max(min_x));
        let y = (center_y - new_height / 2).clamp(min_y, max_y.max(min_y));
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    log_debug(&format!("popup 自适应尺寸: {width:.0}x{height:.0}"));

    Ok((width, height))
}

/// 重新触发截图（浮窗"重新翻译"用）
#[tauri::command]
fn trigger_screenshot_cmd(app: tauri::AppHandle) {
    // 隐藏浮窗（不关闭，复用），打开遮罩
    if let Some(win) = app.get_webview_window("popup") {
        let _ = win.hide();
    }
    trigger_screenshot(&app);
}

/// 用多模态重新翻译最近的截图（用户点"用大模型重试"时调）
#[tauri::command]
async fn multimodal_retry(app: tauri::AppHandle) -> translate::TranslateResult {
    let last = app.state::<translate::LastResult>();
    let data_uri = last.data_uri.lock().unwrap().clone();
    let target_lang = last.target_lang.lock().unwrap().clone().unwrap_or_default();
    let data_uri = match data_uri {
        Some(d) if !d.is_empty() => d,
        _ => return translate::TranslateResult::err("NO_DATA", "没有可用的截图，请重新框选".into(), "".into()),
    };
    let params = translate::TranslateParams {
        data_uri,
        target_lang,
    };
    // 直接走多模态（绕过OCR）
    let result = translate::multimodal_only(&app, &params).await;
    let l = app.state::<translate::LastResult>();
    *l.result.lock().unwrap() = Some(result.clone());
    let _ = app.emit("translate-done", &result);
    result
}

/// 测试命令：直接读最近结果，返回 JSON（前端调，验证数据传递）
#[tauri::command]
fn debug_last_result(app: tauri::AppHandle) -> String {
    let last = app.state::<translate::LastResult>();
    let result = last.result.lock().unwrap().clone();
    let target_lang = last.target_lang.lock().unwrap().clone();
    let s = serde_json::to_string(&serde_json::json!({
        "result": result,
        "target_lang": target_lang,
    }))
    .unwrap_or_default();
    log_debug(&format!("debug_last_result: {s}"));
    s
}

/// 打开截图遮罩窗口（已存在则重新唤起）
fn open_overlay(app: &tauri::AppHandle) {
    // 先隐藏结果浮窗，避免把它截进新截图。
    if let Some(win) = app.get_webview_window("popup") {
        let _ = win.hide();
    }

    // 每次唤起都重新截屏，不能复用上一次画面。
    if let Err(e) = screenshot::capture_full(app) {
        log_debug(&format!("截屏失败：{e}"));
    }

    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit_to("overlay", "refresh-shot", ());
    } else {
        log_debug("overlay 窗口不存在（应在 setup 预创建）");
    }
}

/// 打开设置窗口（已存在则聚焦）
fn open_settings(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit_to("settings", "refresh-settings", ());
    } else {
        log_debug("settings 窗口不存在（应在 setup 预创建）");
    }
}

/// 自测：用内置测试图走完整翻译流程 + 开浮窗（不依赖鼠标框选）
fn self_test(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        log_debug("self_test 启动");
        // 读测试图
        let img_bytes = match std::fs::read("C:\\tyc\\test_shot.png") {
            Ok(b) => b,
            Err(e) => {
                log_debug(&format!("self_test: 读图失败 {e}"));
                return;
            }
        };
        let b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &img_bytes,
        );
        let data_uri = format!("data:image/png;base64,{b64}");
        let cfg = config::load(&app);
        let effective_target = if cfg.target_lang.trim().is_empty() {
            cfg.native_lang.clone()
        } else {
            cfg.target_lang.clone()
        };
        log_debug("self_test: 开始翻译");
        let params = translate::TranslateParams {
            data_uri: data_uri.clone(),
            target_lang: cfg.target_lang.clone(),
        };
        let result = translate::translate(&app, params).await;
        log_debug(&format!(
            "self_test: 翻译完成 success={} translated={}",
            result.success, result.translated
        ));
        // 存结果
        let last = app.state::<translate::LastResult>();
        last.set(result, data_uri, effective_target);
        log_debug("self_test: 已存结果，开浮窗");
        // 浮窗已在 setup 预创建，直接 show
        if let Some(win) = app.get_webview_window("popup") {
            let _ = win.show();
            let _ = app.emit_to("popup", "refresh-result", ());
        }
        log_debug("self_test: 浮窗已开");
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(State {
            last_trigger: Mutex::new(None),
            last_esc: Mutex::new(None),
        })
        .manage(screenshot::new_cache())
        .manage(translate::TranslateCache::new())
        .manage(translate::LastResult::new())
        .manage(rapidocr::RapidOcrState::default())
        .invoke_handler(tauri::generate_handler![
            close_overlay,
            get_shot_size,
            crop_region,
            get_full_image,
            get_config,
            save_config,
            list_providers,
            rapidocr_status,
            rapidocr_release_info,
            download_install_rapidocr_component,
            install_rapidocr_component_from_file,
            uninstall_rapidocr_component,
            translate,
            get_last_result,
            open_popup,
            hide_popup,
            resize_popup,
            trigger_screenshot_cmd,
            multimodal_retry,
            debug_last_result
        ])
        .setup(|app| {
            // ===== 系统托盘 =====
            let test_i = MenuItem::with_id(app, "test", "测试翻译", true, None::<&str>)?;
            let prefs_i = MenuItem::with_id(app, "prefs", "首选项", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&test_i, &prefs_i, &quit_i])?;

            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("太阳穴 TempleFix")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "prefs" => open_settings(app),
                    "test" => self_test(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        trigger_screenshot(tray.app_handle());
                    }
                })
                .build(app)?;

            // ===== 所有窗口预创建、初始隐藏；运行中只 show/hide =====
            WebviewWindowBuilder::new(
                app,
                "overlay",
                WebviewUrl::App("overlay.html".into()),
            )
            .title("")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .transparent(true)
            .fullscreen(true)
            .visible(false)
            .build()?;
            let settings = WebviewWindowBuilder::new(
                app,
                "settings",
                WebviewUrl::App("settings.html".into()),
            )
            .title("太阳穴 - 首选项")
            .inner_size(560.0, 640.0)
            .resizable(false)
            .visible(false)
            .build()?;
            let _ = settings.center();
            let settings_to_hide = settings.clone();
            settings.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = settings_to_hide.hide();
                }
            });

            // ===== 预创建浮窗（初始隐藏，翻译时 show，避免在命令里 build 死锁）=====
            let popup = WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("popup.html".into()))
                .title("")
                .inner_size(420.0, 320.0)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .resizable(false)
                .shadow(true)
                .visible(false) // 初始隐藏
                .build()?;
            // 居中到主显示器上方
            let _ = popup.center();

            // ===== 全局热键（用官方插件，自动处理事件循环线程）=====
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, ShortcutState};
                let esc = Shortcut::new(None, Code::Escape);
                let configured_hotkey = config::load(app.handle()).hotkey;
                let capture_hotkey = parse_hotkey(&configured_hotkey).unwrap_or_else(|e| {
                    log_debug(&format!("配置中的快捷键无效，回退 Alt+Z：{e}"));
                    default_hotkey()
                });

                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, shortcut, event| {
                            // 只处理按下（Released 在按住时会连续触发，导致误关）
                            if event.state() != ShortcutState::Pressed {
                                return;
                            }
                            if shortcut == &esc {
                                // ESC 防抖：500ms 内只生效一次，避免连按/系统重复
                                let state = app.state::<State>();
                                let now = Instant::now();
                                let mut last = state.last_esc.lock().unwrap();
                                if let Some(t) = *last {
                                    if now.duration_since(t).as_millis() < 500 {
                                        return;
                                    }
                                }
                                *last = Some(now);
                                drop(last);
                                // 只关当前最上层的窗口：popup 优先（hide 复用），没有才关 overlay
                                if let Some(win) = app.get_webview_window("popup") {
                                    if win.is_visible().unwrap_or(false) {
                                        let _ = win.hide();
                                        return;
                                    }
                                }
                                if let Some(win) = app.get_webview_window("overlay") {
                                    let _ = win.hide();
                                }
                            } else {
                                // 除 ESC 外，本插件只注册当前截图快捷键。
                                trigger_screenshot(app);
                            }
                        })
                        .build(),
                )?;

                match app.global_shortcut().register(capture_hotkey) {
                    Ok(_) => log_debug(&format!("{} 注册成功", configured_hotkey)),
                    Err(e) => log_debug(&format!("{} 注册失败：{e}", configured_hotkey)),
                }
                match app.global_shortcut().register(esc) {
                    Ok(_) => log_debug("ESC 注册成功"),
                    Err(e) => log_debug(&format!("ESC 注册失败：{e}")),
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle: &tauri::AppHandle, event: tauri::RunEvent| {
            // 托盘常驻程序：初始无窗口，必须拦住默认的"窗口全关就退出"行为，
            // 否则程序一启动就跑完了。只有用户从托盘菜单点"退出"才真正退出。
            match event {
                tauri::RunEvent::ExitRequested { code, api, .. } => {
                    if should_prevent_exit(code) {
                        api.prevent_exit();
                    }
                }
                tauri::RunEvent::Exit => {
                    app_handle.cleanup_before_exit();
                }
                _ => {}
            }
        });
}

#[cfg(all(test, desktop))]
mod tests {
    use super::{clamp_popup_size, parse_hotkey, should_prevent_exit};

    #[test]
    fn accepts_common_shortcuts_and_rejects_reserved_or_unsafe_keys() {
        assert!(parse_hotkey("Alt+Z").is_ok());
        assert!(parse_hotkey("Ctrl+Shift+X").is_ok());
        assert!(parse_hotkey("Escape").is_err());
        assert!(parse_hotkey("Z").is_err());
    }

    #[test]
    fn tray_exit_is_allowed_but_window_close_keeps_tray_alive() {
        assert!(should_prevent_exit(None));
        assert!(!should_prevent_exit(Some(0)));
    }

    #[test]
    fn popup_size_is_bounded_by_content_limits_and_monitor() {
        assert_eq!(clamp_popup_size(200.0, 100.0, 1920.0, 1080.0), (360.0, 180.0));
        assert_eq!(clamp_popup_size(900.0, 900.0, 1920.0, 1080.0), (720.0, 760.0));
        assert_eq!(clamp_popup_size(700.0, 700.0, 500.0, 400.0), (468.0, 336.0));
    }
}
