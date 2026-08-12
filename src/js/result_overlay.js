/* ===== 原位覆盖结果渲染 ===== */
(function () {
  "use strict";

  const root = document.getElementById("overlay_result");

  function validRect(rect) {
    if (!rect) return false;
    const values = [rect.left, rect.top, rect.width, rect.height].map(Number);
    return values.every(Number.isFinite) &&
      rect.left >= 0 && rect.top >= 0 && rect.width >= 0.002 && rect.height >= 0.002 &&
      rect.left + rect.width <= 1.001 && rect.top + rect.height <= 1.001;
  }

  async function fallback() {
    root.textContent = "";
    try {
      await TF.invoke("fallback_to_plain_popup");
    } catch (_) {
      // 后端在显示前也会校验；此处只是渲染期的最后一道保险。
    }
  }

  function fitText(element) {
    let size = Math.max(8, Math.min(52, element.clientHeight * 0.72));
    element.style.fontSize = size + "px";
    while (size > 8 && (element.scrollWidth > element.clientWidth || element.scrollHeight > element.clientHeight)) {
      size -= 1;
      element.style.fontSize = size + "px";
    }
    return element.scrollWidth <= element.clientWidth && element.scrollHeight <= element.clientHeight;
  }

  async function loadData() {
    try {
      const [data, config] = await Promise.all([
        TF.invoke("get_last_result"),
        TF.invoke("get_config"),
      ]);
      TFI18n.setLanguage(TFI18n.detect(config));
      const result = data && data.result;
      const lines = result && result.success && Array.isArray(result.overlay_lines)
        ? result.overlay_lines
        : [];
      if (!lines.length || lines.length > 80 || lines.some((line) => !line.text || !validRect(line.rect))) {
        await fallback();
        return;
      }

      root.textContent = "";
      for (const line of lines) {
        const element = document.createElement("div");
        element.className = "translated-line";
        element.textContent = line.text;
        element.style.left = (line.rect.left * 100) + "%";
        element.style.top = (line.rect.top * 100) + "%";
        element.style.width = (line.rect.width * 100) + "%";
        element.style.height = (line.rect.height * 100) + "%";
        root.appendChild(element);
      }
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const fitted = [...root.querySelectorAll(".translated-line")].every(fitText);
      if (!fitted) await fallback();
    } catch (_) {
      await fallback();
    }
  }

  TF.listen("refresh-overlay-result", loadData);
})();
