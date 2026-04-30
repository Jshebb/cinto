#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const path = require("node:path");

const packages = {
  "linux-x64": "@cinto/linux-x64",
  "linux-arm64": "@cinto/linux-arm64",
  "darwin-x64": "@cinto/darwin-x64",
  "darwin-arm64": "@cinto/darwin-arm64",
  "win32-x64": "@cinto/win32-x64"
};

const key = `${process.platform}-${process.arch}`;
const packageName = packages[key];

if (!packageName) {
  console.error(`cinto: unsupported platform ${key}`);
  process.exit(1);
}

let packageJson;
try {
  packageJson = require.resolve(`${packageName}/package.json`);
} catch (error) {
  console.error(`cinto: missing optional package ${packageName}`);
  console.error("Try reinstalling with optional dependencies enabled.");
  process.exit(1);
}

const binaryName = process.platform === "win32" ? "cinto.exe" : "cinto";
const binary = path.join(path.dirname(packageJson), "bin", binaryName);
const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  console.error(`cinto: failed to run ${binary}: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
