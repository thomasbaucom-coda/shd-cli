#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
#  SHD CLI — One-line installer for private repo
#
#  Usage:
#    curl -fsSL https://raw.githubusercontent.com/thomasbaucom-coda/shd-cli/main/install.sh | bash
#
#  Or if you've cloned the repo:
#    ./install.sh
#
#  Requires: gh (GitHub CLI) authenticated with repo access
# ============================================================================

REPO="thomasbaucom-coda/shd-cli"
INSTALL_DIR="${SHD_INSTALL_DIR:-$HOME/.local/bin}"
BINARY_NAME="shd"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}▸${NC} $*"; }
ok()    { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}!${NC} $*"; }
fail()  { echo -e "${RED}✗${NC} $*" >&2; exit 1; }

echo ""
echo -e "${BOLD}  SHD CLI Installer${NC}"
echo -e "  Superhuman Docs — agent-first CLI for Coda"
echo ""

# ---- Prerequisites ----

# Check for gh CLI
if ! command -v gh &>/dev/null; then
  fail "GitHub CLI (gh) is required for private repo access.
  Install it: https://cli.github.com/
  Then run: gh auth login"
fi

# Check gh is authenticated
if ! gh auth status &>/dev/null 2>&1; then
  fail "GitHub CLI is not authenticated.
  Run: gh auth login"
fi

# Check repo access
if ! gh api "repos/$REPO" --silent 2>/dev/null; then
  fail "Cannot access $REPO.
  Make sure your GitHub account has access to the repo.
  Try: gh auth refresh -s repo"
fi

ok "GitHub access verified"

# ---- Detect platform ----

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) PLATFORM="darwin" ;;
  Linux)  PLATFORM="linux" ;;
  *)      fail "Unsupported OS: $OS (need macOS or Linux)" ;;
esac

case "$ARCH" in
  arm64|aarch64) LABEL="${PLATFORM}-arm64" ;;
  x86_64|amd64)  LABEL="${PLATFORM}-x64" ;;
  *)             fail "Unsupported architecture: $ARCH" ;;
esac

info "Platform: ${PLATFORM}/${ARCH} → ${LABEL}"

# ---- Find latest release ----

LATEST_TAG=$(gh release list -R "$REPO" --limit 1 --json tagName -q '.[0].tagName' 2>/dev/null || echo "")

if [ -z "$LATEST_TAG" ]; then
  # No releases yet — build from source
  warn "No releases found. Building from source..."

  if ! command -v cargo &>/dev/null; then
    fail "No releases available and Rust is not installed.
  Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  Or ask a team member to create a release: git tag v0.1.0 && git push --tags"
  fi

  CLONE_DIR=$(mktemp -d)
  info "Cloning repo..."
  gh repo clone "$REPO" "$CLONE_DIR" -- --depth 1 --quiet
  info "Building (this takes ~30s)..."
  cargo build --release --manifest-path "$CLONE_DIR/Cargo.toml" 2>&1 | tail -2
  mkdir -p "$INSTALL_DIR"
  cp "$CLONE_DIR/target/release/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
  chmod +x "$INSTALL_DIR/$BINARY_NAME"
  rm -rf "$CLONE_DIR"
  ok "Built and installed to $INSTALL_DIR/$BINARY_NAME"
else
  # Download pre-built binary from release
  ASSET="shd-${LABEL}.tgz"
  info "Downloading ${LATEST_TAG} (${ASSET})..."

  TMP_DIR=$(mktemp -d)
  gh release download "$LATEST_TAG" -R "$REPO" -p "$ASSET" -D "$TMP_DIR" --clobber

  # Extract the binary from the npm tarball
  # npm tarballs have a package/ prefix
  tar -xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR"

  # Find the native binary in the extracted package
  NATIVE_BIN="$TMP_DIR/package/bin/shd-native"
  if [ ! -f "$NATIVE_BIN" ]; then
    # Try alternate locations
    NATIVE_BIN=$(find "$TMP_DIR" -name "shd-native" -type f 2>/dev/null | head -1)
  fi

  if [ -z "$NATIVE_BIN" ] || [ ! -f "$NATIVE_BIN" ]; then
    fail "Could not find shd-native binary in release archive"
  fi

  mkdir -p "$INSTALL_DIR"
  cp "$NATIVE_BIN" "$INSTALL_DIR/$BINARY_NAME"
  chmod +x "$INSTALL_DIR/$BINARY_NAME"
  rm -rf "$TMP_DIR"
  ok "Installed ${LATEST_TAG} to $INSTALL_DIR/$BINARY_NAME"
fi

# ---- Verify install ----

if ! command -v "$BINARY_NAME" &>/dev/null; then
  # Binary installed but not on PATH
  SHELL_NAME=$(basename "$SHELL")
  case "$SHELL_NAME" in
    zsh)  RC_FILE="$HOME/.zshrc" ;;
    bash) RC_FILE="$HOME/.bashrc" ;;
    *)    RC_FILE="$HOME/.profile" ;;
  esac

  if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    warn "$INSTALL_DIR is not on your PATH"
    echo ""
    echo -e "  Add it by running:"
    echo -e "  ${BOLD}echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> $RC_FILE && source $RC_FILE${NC}"
    echo ""
  fi
else
  VERSION=$("$BINARY_NAME" --version 2>/dev/null || echo "installed")
  ok "shd is on your PATH ($VERSION)"
fi

# ---- Auth ----

echo ""
echo -e "${BOLD}  Next: Authenticate${NC}"
echo ""

# Check if already authenticated
if "$INSTALL_DIR/$BINARY_NAME" auth status &>/dev/null 2>&1; then
  ok "Already authenticated"
  echo ""
  echo -e "  You're all set! Try: ${BOLD}shd whoami${NC}"
else
  echo -e "  Run this to log in:"
  echo ""
  echo -e "  ${BOLD}shd auth login${NC}"
  echo ""
  echo -e "  This will open ${BLUE}coda.io/account${NC} to generate a token."
  echo -e "  Choose ${BOLD}MCP-scoped${NC} token for full access."
  echo ""
  echo -e "  Or set it directly:"
  echo -e "  ${BOLD}export CODA_API_TOKEN=your-token-here${NC}"
fi

echo ""
echo -e "${BOLD}  Quick start:${NC}"
echo -e "  shd discover                    # see all tools"
echo -e "  shd discover --compact whoami   # compact schema"
echo -e "  shd whoami                      # test your connection"
echo -e "  shd doc_scaffold --json @blueprint.json  # build a doc"
echo ""
