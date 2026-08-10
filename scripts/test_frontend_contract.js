"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const html = fs.readFileSync(path.join(root, "src", "settings.html"), "utf8");
const script = fs.readFileSync(path.join(root, "src", "js", "settings.js"), "utf8");
const rust = fs.readFileSync(path.join(root, "src-tauri", "src", "lib.rs"), "utf8");

const idList = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
const ids = new Set(idList);
const duplicateIds = idList.filter((id, index) => idList.indexOf(id) !== index);
const referencedIds = new Set([...script.matchAll(/\$\("([^"]+)"\)/g)].map((match) => match[1]));
const missingIds = [...referencedIds].filter((id) => !ids.has(id));
const commands = new Set(
  [...script.matchAll(/TF\.invoke\("([^"]+)"/g)].map((match) => match[1])
);
const missingCommands = [...commands].filter((command) => !rust.includes(command));

if (duplicateIds.length || missingIds.length || missingCommands.length) {
  console.error("Duplicate HTML ids:", duplicateIds);
  console.error("Missing HTML ids:", missingIds);
  console.error("Missing Rust commands:", missingCommands);
  process.exit(1);
}

console.log(
  `Settings contract OK: ${ids.size} ids, ${referencedIds.size} JS references, ${commands.size} commands.`
);
