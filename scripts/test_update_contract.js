"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const baseConfig = JSON.parse(read("src-tauri/tauri.conf.json"));
const releaseConfig = JSON.parse(read("src-tauri/tauri.updater.conf.json"));
assert(baseConfig.bundle.createUpdaterArtifacts !== true,
  "ordinary local builds must not require the release signing key");
assert(releaseConfig.bundle.createUpdaterArtifacts === true,
  "release builds must create signed updater artifacts");
assert(typeof releaseConfig.plugins?.updater?.pubkey === "string" &&
  releaseConfig.plugins.updater.pubkey.length > 100,
"release builds must bundle the updater verification public key");
const decodedPublicKey = Buffer.from(releaseConfig.plugins.updater.pubkey, "base64").toString("utf8");
assert(decodedPublicKey.startsWith("untrusted comment: minisign public key") &&
  decodedPublicKey.trim().split(/\r?\n/).length === 2,
"the bundled updater public key must use Tauri's encoded Minisign format");

const updater = read("src-tauri/src/app_update.rs");
assert(updater.includes("https://github.com/lh0227-tech/TempleFix/releases/latest/download/latest.json"),
  "the canonical GitHub update endpoint is missing");
assert(updater.includes('"Gitee" => matches!(host.as_str(), "gitee.com" | "www.gitee.com")'),
  "Gitee endpoints must be host-restricted");
assert(updater.includes("app.restart()"), "a successful Windows update must restart the app");
assert(updater.includes("CHECK_INTERVAL_SECS"), "automatic checks must be throttled");
assert(/candidate\s*\.update\s*\.install\(bytes\)/.test(updater),
  "downloaded update bytes must pass through Tauri signature verification before installation");

const welcome = read("src/welcome.html");
const welcomeJs = read("src/js/welcome.js");
assert(welcome.includes("选择界面语言 / Choose interface language"),
  "the first-run language gateway must remain bilingual");
assert(welcomeJs.includes('TF.invoke("save_ui_language"'),
  "the first-run language choice must be persisted immediately without overwriting other settings");

const settings = read("src/settings.html");
const settingsJs = read("src/js/settings.js");
for (const id of ["auto_check_updates", "check_app_update", "install_app_update", "app_update_progress"]) {
  assert(settings.includes(`id="${id}"`), `settings update control is missing: ${id}`);
}
for (const command of ["get_app_update_status", "check_app_update", "install_app_update"]) {
  assert(settingsJs.includes(`TF.invoke("${command}"`), `settings command is missing: ${command}`);
}

const workflow = read(".github/workflows/release.yml");
assert(workflow.includes("workflow_dispatch:"), "release workflow must require a manual start");
assert(!/^\s+push:\s*$/m.test(workflow), "release workflow must not upload on an ordinary push");
assert(workflow.includes("releaseDraft: true"), "release workflow must create a draft first");
assert(workflow.includes("tauri-apps/tauri-action@v1"), "release workflow must use the official Tauri action");
assert(workflow.includes("validate_updater_release.ps1"),
  "release workflow must fail closed when signing or Gitee configuration is missing");

console.log("Update contract OK: bilingual onboarding, signed dual-source updates, manual draft release.");
