"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const root = path.resolve(__dirname, "..");
const rust = fs.readFileSync(path.join(root, "src-tauri", "src", "lib.rs"), "utf8");
const pages = [
  ["settings.html", "settings.js"],
  ["welcome.html", "welcome.js"],
  ["popup.html", "popup.js"],
  ["overlay.html", "overlay.js"],
  ["result_overlay.html", "result_overlay.js"],
];
let idCount = 0;
let referenceCount = 0;
const allCommands = new Set();
const failures = [];

for (const [htmlName, scriptName] of pages) {
  const html = fs.readFileSync(path.join(root, "src", htmlName), "utf8");
  const script = fs.readFileSync(path.join(root, "src", "js", scriptName), "utf8");
  const idList = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
  const dynamicIds = [...script.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
  const ids = new Set([...idList, ...dynamicIds]);
  const duplicateIds = idList.filter((id, index) => idList.indexOf(id) !== index);
  const referencedIds = new Set([...script.matchAll(/\$\("([^"]+)"\)/g)].map((match) => match[1]));
  const missingIds = [...referencedIds].filter((id) => !ids.has(id));
  [...script.matchAll(/TF\.invoke\("([^"]+)"/g)].forEach((match) => allCommands.add(match[1]));
  if (duplicateIds.length) failures.push(`${htmlName}: duplicate ids ${duplicateIds.join(", ")}`);
  if (missingIds.length) failures.push(`${scriptName}: missing ids ${missingIds.join(", ")}`);
  idCount += ids.size;
  referenceCount += referencedIds.size;
}

const missingCommands = [...allCommands].filter((command) => !rust.includes(command));
if (missingCommands.length) failures.push(`Missing Rust commands: ${missingCommands.join(", ")}`);

const i18nScript = fs.readFileSync(path.join(root, "src", "js", "i18n.js"), "utf8");
const uiTranslationsScript = fs.readFileSync(
  path.join(root, "src", "js", "ui-translations.js"),
  "utf8"
);
const translationKeys = new Set();
for (const [htmlName, scriptName] of pages) {
  const html = fs.readFileSync(path.join(root, "src", htmlName), "utf8");
  const script = fs.readFileSync(path.join(root, "src", "js", scriptName), "utf8");
  [...html.matchAll(/data-i18n(?:-placeholder|-title)?="([^"]+)"/g)].forEach((match) => {
    translationKeys.add(match[1]);
  });
  [...script.matchAll(/(?:\btr|TFI18n\.t)\("([^"]+)"/g)].forEach((match) => {
    if (!match[1].endsWith("_")) translationKeys.add(match[1]);
  });
}
const i18nContext = {
  document: { documentElement: {}, querySelectorAll: () => [] },
  window: { navigator: { language: "en" } },
};
vm.runInNewContext(i18nScript, i18nContext);
vm.runInNewContext(uiTranslationsScript, i18nContext);
for (const language of ["zh-CN", "en", "ja", "fr", "de", "es", "pt-BR"]) {
  i18nContext.window.TFI18n.setLanguage(language);
  const missingTranslationKeys = [...translationKeys].filter(
    (key) => !i18nContext.window.TFI18n.hasTranslation(language, key)
  );
  if (missingTranslationKeys.length) {
    failures.push(`${language}: missing translations ${missingTranslationKeys.join(", ")}`);
  }
}

if (failures.length) {
  failures.forEach((failure) => console.error(failure));
  process.exit(1);
}

console.log(
  `Frontend contract OK: ${pages.length} pages, ${idCount} ids, ${referenceCount} JS references, ${allCommands.size} commands.`
);
