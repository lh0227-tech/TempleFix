"use strict";

const fs = require("fs");
const http = require("http");
const path = require("path");

const packagePath = path.resolve(process.argv[2] || "");
const port = Number(process.argv[3] || 38177);
const packageName = path.basename(packagePath);

if (!fs.statSync(packagePath).isFile()) {
  throw new Error("Component package does not exist: " + packagePath);
}

http.createServer((request, response) => {
  if (request.method !== "GET" || decodeURIComponent(request.url) !== "/" + packageName) {
    response.writeHead(404).end("Not found");
    return;
  }
  const size = fs.statSync(packagePath).size;
  response.writeHead(200, {
    "Content-Type": "application/octet-stream",
    "Content-Length": size,
    "Cache-Control": "no-store",
  });
  fs.createReadStream(packagePath).pipe(response);
}).listen(port, "127.0.0.1");
