// 独立测试 xcap 截图
fn main() {
    println!("开始测试截图...");
    match xcap::Monitor::all() {
        Ok(monitors) => {
            println!("找到 {} 个显示器", monitors.len());
            for (i, m) in monitors.iter().enumerate() {
                println!(
                    "  [{}] {} {}x{} primary={}",
                    i,
                    m.name().unwrap_or_default(),
                    m.width().unwrap_or(0),
                    m.height().unwrap_or(0),
                    m.is_primary().unwrap_or(false)
                );
            }
            let primary = monitors
                .into_iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .or_else(|| {
                    xcap::Monitor::all()
                        .ok()
                        .and_then(|ms| ms.into_iter().next())
                });
            match primary {
                Some(m) => {
                    println!("用主显示器截图...");
                    match m.capture_image() {
                        Ok(img) => {
                            println!("截图成功！{}x{}", img.width(), img.height());
                            img.save("C:\\tyc\\test_shot.png").ok();
                            println!("已存到 C:\\tyc\\test_shot.png");
                        }
                        Err(e) => {
                            println!("截图失败：{:?}", e);
                            println!("错误类型：{}", e);
                            println!("错误显示：{e}");
                        }
                    }
                }
                None => println!("找不到主显示器"),
            }
        }
        Err(e) => println!("获取显示器失败：{:?}", e),
    }
}
