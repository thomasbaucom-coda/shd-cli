#!/usr/bin/env node

/**
 * Post-install script for @thomasbaucom-coda/shd npm package.
 *
 * Resolution order:
 * 1. Pre-built binary already in bin/ (from platform-specific tarball)
 * 2. Download from GitHub Releases (requires `gh` CLI)
 * 3. Build from source (requires Rust toolchain)
 */

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const BIN_DIR = path.join(__dirname, "bin");
const EXT = os.platform() === "win32" ? ".exe" : "";
const NATIVE_NAME = `shd-native${EXT}`;
const NATIVE_PATH = path.join(BIN_DIR, NATIVE_NAME);
const REPO = "thomasbaucom-coda/shd-cli";

// 1. If native binary already exists (pre-built distribution), done
if (fs.existsSync(NATIVE_PATH)) {
  const stats = fs.statSync(NATIVE_PATH);
  if (stats.size > 1000) {
    console.log(`[shd] Native binary found at ${NATIVE_PATH}`);
    process.exit(0);
  }
}

// Determine platform label for GitHub Release assets
function getPlatformLabel() {
  const platform = os.platform();
  const arch = os.arch();
  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "darwin-x64";
  if (platform === "linux" && arch === "x64") return "linux-x64";
  if (platform === "linux" && arch === "arm64") return "linux-arm64";
  return null;
}

// 2. Try downloading from GitHub Releases via gh CLI
function tryDownloadFromRelease() {
  const label = getPlatformLabel();
  if (!label) return false;

  try {
    // Check if gh is available and authenticated
    execSync("gh auth status", { stdio: "ignore" });
  } catch {
    return false;
  }

  const pkg = require("./package.json");
  const version = `v${pkg.version}`;
  const asset = `shd-${label}.tgz`;
  const tmpDir = os.tmpdir();
  const tmpTgz = path.join(tmpDir, asset);

  console.log(`[shd] Downloading ${asset} from GitHub Releases...`);
  try {
    execSync(
      `gh release download ${version} -R ${REPO} -p '${asset}' -D '${tmpDir}' --clobber`,
      { stdio: "inherit" }
    );

    // Extract the native binary from the tarball
    execSync(`tar -xzf '${tmpTgz}' -C '${tmpDir}' package/bin/shd-native`, {
      stdio: "inherit",
    });

    const extracted = path.join(tmpDir, "package", "bin", "shd-native");
    if (fs.existsSync(extracted)) {
      fs.mkdirSync(BIN_DIR, { recursive: true });
      fs.copyFileSync(extracted, NATIVE_PATH);
      fs.chmodSync(NATIVE_PATH, 0o755);
      console.log(`[shd] Installed pre-built binary for ${label}`);

      // Cleanup
      try {
        fs.unlinkSync(tmpTgz);
        fs.rmSync(path.join(tmpDir, "package"), { recursive: true });
      } catch {}

      return true;
    }
  } catch (err) {
    console.log(`[shd] Could not download from release: ${err.message}`);
  }

  return false;
}

if (tryDownloadFromRelease()) {
  process.exit(0);
}

// 3. Try building from source
const ROOT = path.join(__dirname, "..");
const CARGO_TOML = path.join(ROOT, "Cargo.toml");

if (!fs.existsSync(CARGO_TOML)) {
  console.error(
    "[shd] No pre-built binary, no GitHub Release available, and no Cargo.toml found.\n" +
      "Install options:\n" +
      "  1. Install gh CLI and authenticate: gh auth login\n" +
      "  2. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n" +
      "  3. Download manually from: https://github.com/" +
      REPO +
      "/releases"
  );
  process.exit(1);
}

console.log("[shd] Building from source...");

try {
  execSync("cargo build --release", {
    cwd: ROOT,
    stdio: "inherit",
  });

  const CARGO_BINARY = os.platform() === "win32" ? "shd.exe" : "shd";
  const builtBinary = path.join(ROOT, "target", "release", CARGO_BINARY);

  if (!fs.existsSync(builtBinary)) {
    console.error(
      `[shd] Build succeeded but binary not found at ${builtBinary}`
    );
    process.exit(1);
  }

  fs.mkdirSync(BIN_DIR, { recursive: true });
  fs.copyFileSync(builtBinary, NATIVE_PATH);
  fs.chmodSync(NATIVE_PATH, 0o755);
  console.log(`[shd] Built and installed to ${NATIVE_PATH}`);
} catch (err) {
  console.error(
    "[shd] Failed to build from source. Make sure Rust is installed:\n" +
      "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n"
  );
  process.exit(1);
}
