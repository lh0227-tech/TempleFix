"use strict";

const { spawn } = require("child_process");
const fs = require("fs");
const http = require("http");
const path = require("path");

const projectRoot = path.resolve(__dirname, "..");
const packagePath = path.join(
  projectRoot,
  "rapidocr-addon",
  "release",
  "TempleFix_RapidOCR_Addon_3.9.2-1.zip"
);
const componentPath = path.join(projectRoot, "rapidocr-addon", "build", "package");
const port = 38177;

const server = http.createServer((request, response) => {
  if (request.url !== "/TempleFix_RapidOCR_Addon_3.9.2-1.zip") {
    response.writeHead(404).end("Not found");
    return;
  }
  response.writeHead(200, {
    "Content-Type": "application/octet-stream",
    "Content-Length": fs.statSync(packagePath).size,
    "Cache-Control": "no-store",
  });
  fs.createReadStream(packagePath).pipe(response);
});

server.listen(port, "127.0.0.1", () => {
  const child = spawn("cargo.exe", ["test", "--", "--test-threads=1"], {
    cwd: path.join(projectRoot, "src-tauri"),
    env: {
      ...process.env,
      TEMPLEFIX_RAPIDOCR_COMPONENT_DIR: componentPath,
      TEMPLEFIX_RAPIDOCR_PACKAGE: packagePath,
      TEMPLEFIX_RAPIDOCR_DOWNLOAD_TEST_URL:
        "http://127.0.0.1:" + port + "/TempleFix_RapidOCR_Addon_3.9.2-1.zip",
    },
    stdio: "inherit",
  });
  child.on("error", (error) => {
    console.error(error);
    server.close(() => process.exit(1));
  });
  child.on("exit", (code) => {
    server.close(() => process.exit(code === 0 ? 0 : code || 1));
  });
});
