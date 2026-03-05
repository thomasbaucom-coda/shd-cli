#!/usr/bin/env node

/**
 * Post-install script for shd-cli npm package.
 *
 * Checks for a pre-built native binary, and if missing, builds from source
 * using cargo. For distribution, place pre-built binaries at bin/coda-native.
 */

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const BIN_DIR = path.join(__dirname, "bin");
const EXT = os.platform() === "win32" ? ".exe" : "";
const NATIVE_NAME = `coda-native${EXT}`;
const NATIVE_PATH = path.join(BIN_DIR, NATIVE_NAME);

// If native binary already exists (pre-built distribution), done
if (fs.existsSync(NATIVE_PATH)) {
  const stats = fs.statSync(NATIVE_PATH);
  if (stats.size > 1000) {
    console.log(`[shd-cli] Native binary found at ${NATIVE_PATH}`);
    process.exit(0);
  }
}

// Try building from source
const ROOT = path.join(__dirname, "..");
const CARGO_TOML = path.join(ROOT, "Cargo.toml");

if (!fs.existsSync(CARGO_TOML)) {
  console.error(
    "[shd-cli] No pre-built binary and no Cargo.toml found.\n" +
    "Either place a binary at npm/bin/coda-native or install from the repo root."
  );
  process.exit(1);
}

console.log("[shd-cli] Building from source...");

try {
  execSync("cargo build --release", {
    cwd: ROOT,
    stdio: "inherit",
  });

  const CARGO_BINARY = os.platform() === "win32" ? "coda.exe" : "coda";
  const builtBinary = path.join(ROOT, "target", "release", CARGO_BINARY);

  if (!fs.existsSync(builtBinary)) {
    console.error(`[shd-cli] Build succeeded but binary not found at ${builtBinary}`);
    process.exit(1);
  }

  fs.mkdirSync(BIN_DIR, { recursive: true });
  fs.copyFileSync(builtBinary, NATIVE_PATH);
  fs.chmodSync(NATIVE_PATH, 0o755);
  console.log(`[shd-cli] Built and installed to ${NATIVE_PATH}`);
} catch (err) {
  console.error(
    "[shd-cli] Failed to build from source. Make sure Rust is installed:\n" +
    "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n"
  );
  process.exit(1);
}
