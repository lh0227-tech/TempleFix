/* ===== 翻译结果浮窗 ===== */
(function () {
  "use strict";

  const $ = (id) => document.getElementById(id);
  let curResult = null;
  let targetLang = "";

  // 1. 最先绑定所有交互（不依赖异步数据，保证按钮/ESC 永远可用）
  function bindActions() {
    $("close").addEventListener("click", close);
    $("copy").addEventListener("click", copyText);
    $("copy2").addEventListener("click", copyText);
    $("retranslate").addEventListener("click", retry);

    // 右键也能关（双保险）
    document.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      close();
    });

    // 前端 ESC（窗口有焦点时生效；全局 ESC 由后端热键兜底）
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") close();
    });

    // 头部拖动
    const header = document.querySelector(".header");
    header.addEventListener("mousedown", (e) => {
      if (e.target.closest(".icon-btn")) return;
      const w = window.__TAURI__?.window?.getCurrentWindow?.();
      if (w) w.startDragging();
    });
  }

  // 2. 再异步加载数据
  async function loadData() {
    try {
      const data = await TF.invoke("get_last_result");
      targetLang = (data && data.target_lang) || "简体中文";
      render(data && data.result);
    } catch (e) {
      showLoading("加载失败：" + e);
    }
  }

  // 监听刷新事件（每次 show 时后端 emit，触发重新加载最新结果）
  TF.listen("refresh-result", () => {
    loadData();
  });

  function render(result) {
    if (!result) {
      showLoading("正在翻译…");
      return;
    }
    curResult = result;
    const content = $("content");

    if (!result.success) {
      content.innerHTML = `
        <div class="error-box">
          <div class="msg">${esc(result.error || "翻译失败")}</div>
          <button class="btn btn-primary retry" id="retryBtn">重试</button>
        </div>`;
      const rb = $("retryBtn");
      if (rb) rb.addEventListener("click", retry);
      fitWindow(result);
      return;
    }

    // 短OCR提示：原文太少，建议用多模态
    const isShortOcr = result.error_code === "SHORT_OCR";

    content.innerHTML = `
      <div class="block">
        <div class="label">原文 ${isShortOcr ? "（识别较少）" : ""}</div>
        <div class="text ${result.original ? "" : "empty"}">${
      esc(result.original) || "（无）"
    }</div>
      </div>
      <div class="block">
        <div class="label">译文 · ${esc(targetLang)}</div>
        <div class="text ${result.translated ? "" : "empty"}">${
      esc(result.translated) || "（无）"
    }</div>
      </div>`;

    // 短OCR：加"用大模型重试"按钮
    if (isShortOcr) {
      const btn = document.createElement("button");
      btn.className = "btn btn-primary mm-retry-btn";
      btn.textContent = "用大模型重新识别";
      btn.addEventListener("click", mmRetry);
      content.appendChild(btn);
    }

    const t = $("title");
    if (t) t.textContent = result.model || "太阳穴";
    fitWindow(result);
  }

  function measureLongestLine(result) {
    const sample = document.querySelector(".block .text");
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (sample && ctx) ctx.font = getComputedStyle(sample).font;
    const values = [result && result.original, result && result.translated].filter(Boolean);
    let longest = 0;
    for (const value of values) {
      for (const line of String(value).split(/\r?\n/)) {
        longest = Math.max(longest, ctx ? ctx.measureText(line).width : line.length * 15);
      }
    }
    return longest;
  }

  async function fitWindow(result) {
    // 先让新内容完成排版，再用不可见副本测自然高度。
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const width = Math.max(360, Math.min(720, Math.ceil(measureLongestLine(result) + 72)));
    const popup = document.querySelector(".popup");
    if (!popup) return;
    const probe = popup.cloneNode(true);
    probe.classList.add("size-probe");
    probe.style.width = width + "px";
    document.body.appendChild(probe);
    const height = Math.max(180, Math.min(760, Math.ceil(probe.scrollHeight + 2)));
    probe.remove();
    try {
      await TF.invoke("resize_popup", { width, height });
    } catch (_) {
      // 调整失败不影响翻译内容本身，继续保留当前窗口大小。
    }
  }

  // 用多模态重新翻译（用户点"用大模型重新识别"）
  async function mmRetry() {
    showLoading("用大模型识别中…");
    try {
      const r = await TF.invoke("multimodal_retry");
      render(r);
    } catch (e) {
      showLoading("识别失败：" + e);
    }
  }

  function showLoading(msg) {
    $("content").innerHTML = `
      <div class="loading">
        <div class="spinner"></div>
        <span>${esc(msg)}</span>
      </div>`;
  }

  function copyText() {
    if (!curResult || !curResult.translated) return;
    navigator.clipboard.writeText(curResult.translated).then(() => {
      flashTitle("已复制 ✓");
    });
  }

  async function retry() {
    try {
      await TF.invoke("trigger_screenshot_cmd");
    } catch (e) {
      showLoading("重新截图失败：" + e);
    }
  }

  function close() {
    // 隐藏浮窗（不销毁，复用），由后端处理
    TF.invoke("hide_popup");
  }

  let titleTimer = null;
  function flashTitle(msg) {
    const t = $("title");
    if (!t) return;
    const old = t.textContent;
    t.textContent = msg;
    clearTimeout(titleTimer);
    titleTimer = setTimeout(() => (t.textContent = old), 1500);
  }

  function esc(s) {
    const d = document.createElement("div");
    d.textContent = s || "";
    return d.innerHTML;
  }

  // 启动：先绑定，再加载
  bindActions();
  loadData();
})();
