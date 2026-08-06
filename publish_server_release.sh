#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - Fast Release 发布脚本 (支持交互式选择)
# ==============================================================================
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO] ${NC}$1"; }
log_success() { echo -e "${GREEN}[SUCCESS] ${NC}$1"; }
log_warn() { echo -e "${YELLOW}[WARN] ${NC}$1"; }
log_error() { echo -e "${RED}[ERROR] ${NC}$1"; }

# 1. 检查 GitHub CLI (gh)
if ! command -v gh &> /dev/null; then
    log_error "未检测到 GitHub CLI (gh)！"
    echo "请先安装 gh (如: sudo apt install gh 或 brew install gh) 并运行 'gh auth login' 授权登录。"
    exit 1
fi

VERSION=$(jq -r .version src-tauri/tauri.conf.json 2>/dev/null || echo "0.1.1")
TAG="${1:-v${VERSION}}"
TARGET_FILE="livedownloader-server-linux-amd64"

echo -e "${CYAN}==================================================${NC}"
echo -e "${GREEN}  LiveDownloader Linux Server Release 发布工具${NC}"
echo -e "${CYAN}  目标发布 Tag: [ ${TAG} ]${NC}"
echo -e "${CYAN}==================================================${NC}"

# 交互式菜单选项
echo "请选择构建/上传方式:"
echo "  1) [默认] 寻找本地已有二进制直接上传 (不重新编译)"
echo "  2) 使用 'cargo zigbuild' 交叉编译 Linux amd64 (x86_64-unknown-linux-musl) 并上传"
echo "  3) 使用原生 'cargo build' 本地编译并上传"
read -p "请输入选项 [1-3] (按 Enter 默认为 1): " CHOICE

CHOICE="${CHOICE:-1}"

if [ "$CHOICE" = "2" ]; then
    log_info "正在使用 cargo zigbuild 交叉编译 Linux amd64..."
    cd src-tauri
    cargo zigbuild --target x86_64-unknown-linux-musl --release --no-default-features --features server
    cd ..
elif [ "$CHOICE" = "3" ]; then
    log_info "正在使用 cargo build 编译 Linux Server 二进制..."
    cd src-tauri
    cargo build --release --no-default-features --features server
    cd ..
fi

# 寻找编译出的二进制文件
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

if [ -z "$BIN_SRC" ] || [ ! -f "$BIN_SRC" ]; then
    log_error "未能找到任何可用的二进制文件！请先编译后再试。"
    exit 1
fi

cp "$BIN_SRC" "./${TARGET_FILE}"
chmod +x "./${TARGET_FILE}"

log_success "准备就绪文件: ./${TARGET_FILE} ($(du -h ./${TARGET_FILE} | cut -f1))"

log_info "正在上传至 GitHub Release (${TAG})..."
if gh release view "${TAG}" &>/dev/null; then
    gh release upload "${TAG}" "./${TARGET_FILE}" --clobber
    log_success "二进制 ./${TARGET_FILE} 已成功覆盖更新至 Release: ${TAG}"
else
    log_info "Release ${TAG} 尚不存在，自动新建..."
    gh release create "${TAG}" "./${TARGET_FILE}" \
        --title "LiveDownloader ${TAG}" \
        --notes "LiveDownloader Linux Server Binary (${TAG})"
    log_success "已新建 Release ${TAG} 并成功上传二进制！"
fi

rm -f "./${TARGET_FILE}"
