/* ===== 太阳穴 TempleFix - 前端工具函数 ===== */
(function (global) {
  "use strict";

  // Tauri v2 全局对象（withGlobalTauri: true 已开启）
  const TAURI = global.__TAURI__;

  /** 调用后端 Rust 命令 */
  function invoke(cmd, args) {
    if (TAURI && TAURI.core) {
      return TAURI.core.invoke(cmd, args);
    }
    return Promise.reject(new Error("Tauri 不可用"));
  }

  /** 监听后端事件 */
  function listen(event, handler) {
    if (TAURI && TAURI.event) {
      return TAURI.event.listen(event, handler);
    }
    return Promise.resolve(() => {});
  }

  /** 检测浏览器是否支持 backdrop-filter（决定毛玻璃 / 降级） */
  function detectBlurSupport() {
    const el = document.createElement("div");
    el.style.cssText = "-webkit-backdrop-filter:blur(1px);backdrop-filter:blur(1px)";
    const ok = CSS.supports
      ? CSS.supports("backdrop-filter", "blur(1px)") ||
        CSS.supports("-webkit-backdrop-filter", "blur(1px)")
      : el.style.webkitBackdropFilter !== undefined;
    if (!ok) document.documentElement.classList.add("no-blur");
    return ok;
  }

  /** 防抖 */
  function debounce(fn, ms) {
    let t = null;
    return function (...args) {
      clearTimeout(t);
      t = setTimeout(() => fn.apply(this, args), ms);
    };
  }

  global.TF = { invoke, listen, detectBlurSupport, debounce };
})(window);
