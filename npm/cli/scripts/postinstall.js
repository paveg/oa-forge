#!/usr/bin/env node
// Selects the correct platform-specific binary package for oa-forge.

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const PLATFORM_MAP = {
  "darwin-arm64": "@oa-forge/cli-darwin-arm64",
  "darwin-x64": "@oa-forge/cli-darwin-x64",
  "linux-x64": "@oa-forge/cli-linux-x64",
  "linux-arm64": "@oa-forge/cli-linux-arm64",
  "win32-x64": "@oa-forge/cli-win32-x64",
};

const platformKey = `${process.platform}-${process.arch}`;
const packageName = PLATFORM_MAP[platformKey];

if (!packageName) {
  console.error(
    `oa-forge: unsupported platform ${platformKey}. ` +
      `Supported: ${Object.keys(PLATFORM_MAP).join(", ")}`
  );
  process.exit(1);
}

try {
  const binDir = path.join(__dirname, "..", "bin");
  fs.mkdirSync(binDir, { recursive: true });

  // Resolve the platform-specific package binary
  const pkgDir = path.dirname(require.resolve(`${packageName}/package.json`));
  const binaryName = process.platform === "win32" ? "oa-forge.exe" : "oa-forge";
  const sourceBin = path.join(pkgDir, "bin", binaryName);
  const targetBin = path.join(binDir, binaryName);

  if (fs.existsSync(sourceBin)) {
    fs.copyFileSync(sourceBin, targetBin);
    fs.chmodSync(targetBin, 0o755);
  } else {
    console.error(`oa-forge: binary not found at ${sourceBin}`);
    process.exit(1);
  }
} catch (err) {
  console.error(`oa-forge: failed to install binary: ${err.message}`);
  console.error(
    "You may need to install the platform package manually: " +
      `npm install ${packageName}`
  );
  process.exit(1);
}
