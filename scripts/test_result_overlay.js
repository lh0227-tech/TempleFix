"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const script = fs.readFileSync(
  path.resolve(__dirname, "..", "src", "js", "result_overlay.js"),
  "utf8"
);

class MockElement {
  constructor(width = 200, height = 40) {
    this.children = [];
    this.style = {};
    this.className = "";
    this.clientWidth = width;
    this.clientHeight = height;
    this.scrollWidth = width;
    this.scrollHeight = height;
    this._text = "";
  }

  set textContent(value) {
    this._text = String(value);
    if (value === "") this.children = [];
  }

  get textContent() {
    return this._text;
  }

  appendChild(element) {
    this.children.push(element);
  }

  querySelectorAll(selector) {
    return selector === ".translated-line" ? this.children : [];
  }
}

async function runCase(result) {
  const root = new MockElement(600, 300);
  const calls = [];
  let listener = null;
  const context = {
    document: {
      getElementById: () => root,
      createElement: () => new MockElement(),
    },
    requestAnimationFrame: (callback) => callback(),
    TF: {
      invoke: async (command) => {
        calls.push(command);
        if (command === "get_last_result") return { result };
        if (command === "get_config") return { ui_language: "en" };
        return null;
      },
      listen: (_event, handler) => {
        listener = handler;
      },
    },
    TFI18n: { detect: () => "en", setLanguage: () => "en" },
  };
  vm.runInNewContext(script, context);
  if (!listener) throw new Error("result overlay listener was not registered");
  await listener({ payload: null });
  return { calls, root };
}

(async () => {
  const valid = await runCase({
    success: true,
    overlay_lines: [
      { text: "Hello", rect: { left: 0.1, top: 0.2, width: 0.4, height: 0.15 } },
      { text: "World", rect: { left: 0.1, top: 0.5, width: 0.5, height: 0.15 } },
    ],
  });
  if (valid.root.children.length !== 2) throw new Error("valid overlay lines were not rendered");
  if (valid.calls.includes("fallback_to_plain_popup")) throw new Error("valid overlay unexpectedly fell back");

  const invalid = await runCase({
    success: true,
    overlay_lines: [
      { text: "Outside", rect: { left: 0.9, top: 0.2, width: 0.4, height: 0.15 } },
    ],
  });
  if (!invalid.calls.includes("fallback_to_plain_popup")) {
    throw new Error("invalid overlay did not fall back to the plain popup");
  }

  console.log("Result overlay rendering and fallback checks passed.");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
