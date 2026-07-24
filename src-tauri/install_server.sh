#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - 多架构 (x86_64 / ARM64 / ARMv7) 服务器端安装与更新脚本
# ==============================================================================
# 支持架构：
#  - x86_64 / amd64 (标准 64 位 PC / 云服务器)
#  - arm64 / aarch64 (Cortex-A53 / A72 64位，树莓派4/5, 树莓派OS 64位, Oracle ARM)
#  - armv7 / armhf (Cortex-A7 / A53 32位，香橙派, 树莓派 32位)
#
# 使用方法:
#   chmod +x install_server.sh
#   sudo ./install_server.sh            # 交互式菜单
#   sudo ./install_server.sh install    # 直接安装
#   sudo ./install_server.sh update     # 直接更新
#   sudo ./install_server.sh uninstall  # 直接卸载
# ==============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO] ${NC}$1"; }
log_success() { echo -e "${GREEN}[SUCCESS] ${NC}$1"; }
log_warn() { echo -e "${YELLOW}[WARN] ${NC}$1"; }
log_error() { echo -e "${RED}[ERROR] ${NC}$1"; }

DEST_BIN="/usr/bin/livedownloader"
DEST_ALIAS="/usr/bin/ld-server"
SYSTEMD_PATH="/etc/systemd/system/livedownloader.service"
WORK_DIR="/var/lib/livedownloader"

# GitHub 仓库地址 (用于自动下载对应架构的预编译二进制)
GITHUB_REPO="mega-mage/LiveDownloader"

check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "请使用 root 权限运行此脚本 (例如: sudo $0)"
        exit 1
    fi
}

# 自动检测 CPU 架构
detect_arch() {
    local raw_arch
    raw_arch="$(uname -m)"
    case "$raw_arch" in
        x86_64|amd64)
            ARCH="amd64"
            ;;
        aarch64|arm64|armv8*)
            ARCH="arm64"
            ;;
        armv7*|armv6*|armhf|arm)
            ARCH="armv7"
            ;;
        *)
            log_warn "未自动识别的架构: ${raw_arch}，回退使用 amd64"
            ARCH="amd64"
            ;;
    esac
    log_info "检测到服务器 CPU 硬件架构: ${raw_arch} (对应二进制后缀: ${ARCH})"
}

# 寻找或获取服务端的二进制文件
obtain_binary() {
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$SCRIPT_DIR"

    detect_arch

    # 1. 查找当前目录或上级目录、target 目录是否存在对应架构的预编译二进制
    CANDIDATES=(
        "./livedownloader-server-linux-${ARCH}"
        "../livedownloader-server-linux-${ARCH}"
        "./LiveDownloader"
        "../LiveDownloader"
        "./target/release/LiveDownloader"
    )

    for cand in "${CANDIDATES[@]}"; do
        if [ -f "$cand" ]; then
            log_success "找到预编译二进制文件: ${cand}"
            FOUND_BIN="$(realpath "$cand")"
            return 0
        fi
    done

    # 2. 尝试从 GitHub Releases 下载当前架构的预编译二进制
    log_info "未在本地找到预编译文件，尝试从 GitHub Release 下载 [${ARCH}] 架构二进制..."
    DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/latest/download/livedownloader-server-linux-${ARCH}"
    TMP_BIN="./livedownloader-server-linux-${ARCH}"

    DOWNLOAD_SUCCESS=false
    if command -v curl &> /dev/null; then
        if curl -fsSL -o "$TMP_BIN" "$DOWNLOAD_URL"; then
            DOWNLOAD_SUCCESS=true
        fi
    elif command -v wget &> /dev/null; then
        if wget -q -O "$TMP_BIN" "$DOWNLOAD_URL"; then
            DOWNLOAD_SUCCESS=true
        fi
    fi

    if [ "$DOWNLOAD_SUCCESS" = true ] && [ -f "$TMP_BIN" ] && [ -s "$TMP_BIN" ]; then
        chmod +x "$TMP_BIN"
        log_success "成功从 GitHub Release 下载 [${ARCH}] 架构预编译二进制！"
        FOUND_BIN="$(realpath "$TMP_BIN")"
        return 0
    else
        log_warn "下载预编译文件失败（可能尚无 Release 版本或网络无法连接 GitHub）。"
    fi

    # 3. 回退到本地 Cargo 编译
    log_info "准备使用本地 Cargo 进行源码编译..."
    if ! command -v cargo &> /dev/null; then
        log_warn "未检测到 Cargo，尝试加载环境变量..."
        if [ -f "$HOME/.cargo/env" ]; then
            source "$HOME/.cargo/env"
        elif [ -f "/root/.cargo/env" ]; then
            source "/root/.cargo/env"
        fi
    fi

    if ! command -v cargo &> /dev/null; then
        log_error "无法获取 [${ARCH}] 预编译文件，且本地未安装 Cargo，无法继续！"
        log_error "解决方法：请手动将 GitHub Release 中对应架构的二进制放置在脚本目录（命名为 livedownloader-server-linux-${ARCH}），或在服务器上安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    log_info "Cargo 版本: $(cargo --version)"
    log_info "正在进行本地编译 (--no-default-features --features server)..."
    cargo build --release --no-default-features --features server

    LOCAL_TARGET="target/release/LiveDownloader"
    if [ -f "$LOCAL_TARGET" ]; then
        log_success "本地编译成功！"
        FOUND_BIN="$(realpath "$LOCAL_TARGET")"
        return 0
    else
        log_error "本地编译产物未找到！"
        exit 1
    fi
}

# 1. 安装
do_install() {
    check_root
    obtain_binary

    log_info "正在将二进制文件安装至 ${DEST_BIN}..."
    install -m 755 "$FOUND_BIN" "$DEST_BIN"
    ln -sf "$DEST_BIN" "$DEST_ALIAS"

    log_info "配置 systemd 服务 ${SYSTEMD_PATH}..."
    cat <<EOF > "$SYSTEMD_PATH"
[Unit]
Description=LiveDownloader Backend Service
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=${WORK_DIR}
ExecStart=${DEST_BIN} --server --port 10730
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    mkdir -p "$WORK_DIR"
    systemctl daemon-reload || true

    log_success "===================================================="
    log_success "  LiveDownloader 服务端 [${ARCH}] 安装完成！"
    log_success "===================================================="
    echo -e "${CYAN}启动服务:${NC} systemctl start livedownloader"
    echo -e "${CYAN}开机自启:${NC} systemctl enable livedownloader"
    echo -e "${CYAN}查看状态:${NC} systemctl status livedownloader"
}

# 2. 更新
do_update() {
    check_root

    log_info "开始更新流程..."

    # 清理掉旧的临时下载文件，确保获取最新版本的二进制
    rm -f ./livedownloader-server-linux-*

    if [ -d "../.git" ] || [ -d ".git" ]; then
        log_info "检测到 Git 仓库，尝试 git pull..."
        git pull || log_warn "Git pull 失败，将使用现有代码或下载最新 Release..."
    fi

    obtain_binary

    log_info "停止现有 LiveDownloader 服务..."
    systemctl stop livedownloader || true

    log_info "替换二进制文件..."
    install -m 755 "$FOUND_BIN" "$DEST_BIN"
    ln -sf "$DEST_BIN" "$DEST_ALIAS"

    log_info "重启 LiveDownloader 服务..."
    systemctl daemon-reload || true
    systemctl restart livedownloader || systemctl start livedownloader

    log_success "===================================================="
    log_success "  LiveDownloader 服务 [${ARCH}] 更新完成并已重新启动！"
    log_success "===================================================="
    systemctl status livedownloader --no-pager || true
}

# 3. 卸载
do_uninstall() {
    check_root

    log_warn "确定要卸载 LiveDownloader 吗？"
    read -p "请输入 [y/N] 确认卸载: " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        log_info "取消卸载。"
        exit 0
    fi

    log_info "停止并禁用 systemd 服务..."
    systemctl stop livedownloader || true
    systemctl disable livedownloader || true

    if [ -f "$SYSTEMD_PATH" ]; then
        log_info "删除 systemd 服务配置文件..."
        rm -f "$SYSTEMD_PATH"
        systemctl daemon-reload || true
    fi

    log_info "删除二进制文件与快捷链接..."
    rm -f "$DEST_BIN" "$DEST_ALIAS"

    log_warn "是否要清理数据和配置目录 (${WORK_DIR})？"
    read -p "请输入 [y/N] (默认保留数据): " clean_data
    if [[ "$clean_data" == "y" || "$clean_data" == "Y" ]]; then
        rm -rf "$WORK_DIR"
        log_info "已清理工作目录 ${WORK_DIR}。"
    else
        log_info "已保留工作目录 ${WORK_DIR} 中的配置与视频数据。"
    fi

    log_success "===================================================="
    log_success "  LiveDownloader 卸载完成！"
    log_success "===================================================="
}

# 菜单选择逻辑
ACTION="$1"
if [ -z "$ACTION" ]; then
    echo -e "${CYAN}====================================================${NC}"
    echo -e "${CYAN}    LiveDownloader 服务端管理脚本 (多架构支持)      ${NC}"
    echo -e "${CYAN}====================================================${NC}"
    echo -e " 1) 安装 (Install)"
    echo -e " 2) 更新 (Update)"
    echo -e " 3) 卸载 (Uninstall)"
    echo -e " 4) 退出 (Exit)"
    echo -e "${CYAN}====================================================${NC}"
    read -p "请输入选项数字 [1-4]: " CHOICE

    case "$CHOICE" in
        1) ACTION="install" ;;
        2) ACTION="update" ;;
        3) ACTION="uninstall" ;;
        4) exit 0 ;;
        *) log_error "无效选项！"; exit 1 ;;
    esac
fi

case "$ACTION" in
    install)
        do_install
        ;;
    update)
        do_update
        ;;
    uninstall)
        do_uninstall
        ;;
    *)
        log_error "未知指令: ${ACTION}。可用参数: install | update | uninstall"
        exit 1
        ;;
esac
