#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - 本地编译与 Fast Release 二进制一键上传发布脚本
# ==============================================================================
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO] ${NC}$1"; }
log_success() { echo -e "${GREEN}[SUCCESS] ${NC}$1"; }
log_error() { echo -e "${RED}[ERROR] ${NC}$1"; }

# 1. 检查 GitHub CLI (gh)
if ! command -v gh &> /dev/null; then
    log_error "未检测到 GitHub CLI (gh)！"
    echo "请先安装 gh (如: sudo apt install gh 或 brew install gh) 并运行 'gh auth login' 授权登录。"
    exit 1
fi

# 2. 读取版本号与 Tag
VERSION=$(jq -r .version src-tauri/tauri.conf.json 2>/dev/null || echo "0.1.1")
TAG="${1:-v${VERSION}}"
TARGET_FILE="livedownloader-server-linux-amd64"

log_info "准备发布 Linux amd64 二进制到 GitHub Release Tag: [ ${TAG} ]"

# 3. 寻找编译出的二进制文件
BIN_SRC=""
if [ -f "src-tauri/target/x86_64-unknown-linux-musl/release/LiveDownloader" ]; then
    BIN_SRC="src-tauri/target/x86_64-unknown-linux-musl/release/LiveDownloader"
elif [ -f "src-tauri/target/release/LiveDownloader" ]; then
    BIN_SRC="src-tauri/target/release/LiveDownloader"
elif [ -f "./LiveDownloader" ]; then
    BIN_SRC="./LiveDownloader"
elif [ -f "./${TARGET_FILE}" ]; then
    BIN_SRC="./${TARGET_FILE}"
fi

# 如果未找到已编译的文件，自动在本地编译
if [ -z "$BIN_SRC" ] || [ ! -f "$BIN_SRC" ]; then
    log_info "未检测到预编译好的二进制文件，开始在本地调用 cargo 快速编译..."
    cd src-tauri
    cargo build --release --no-default-features --features server
    cd ..
    BIN_SRC="src-tauri/target/release/LiveDownloader"
fi

if [ ! -f "$BIN_SRC" ]; then
    log_error "未能找到任何可用的二进制文件！"
    exit 1
fi

cp "$BIN_SRC" "./${TARGET_FILE}"
chmod +x "./${TARGET_FILE}"

log_success "准备就绪: ./${TARGET_FILE} ($(du -h ./${TARGET_FILE} | cut -f1))"

# 4. 发布/上传至 GitHub Release
log_info "正在直接上传至 GitHub Release (${TAG})..."

if gh release view "${TAG}" &>/dev/null; then
    gh release upload "${TAG}" "./${TARGET_FILE}" --clobber
    log_success "成功！二进制 ./${TARGET_FILE} 已覆盖更新至已有 Release: ${TAG}"
else
    log_info "发布 Tag ${TAG} 尚不存在，自动新建 Release..."
    gh release create "${TAG}" "./${TARGET_FILE}" \
        --title "LiveDownloader ${TAG}" \
        --notes "LiveDownloader Linux Server Binary (${TAG})"
    log_success "成功！已新建 Release ${TAG} 并上传二进制文件！"
fi

rm -f "./${TARGET_FILE}"
