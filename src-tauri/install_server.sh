#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - 服务器端编译安装、更新与卸载脚本
# ==============================================================================
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

# 检查 root 权限
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "请使用 root 权限运行此脚本 (例如: sudo $0)"
        exit 1
    fi
}

# 检查 Rust/Cargo 环境
check_env() {
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$SCRIPT_DIR"

    if ! command -v cargo &> /dev/null; then
        log_warn "未检测到 Rust/Cargo 环境，尝试加载 cargo 环境变量..."
        if [ -f "$HOME/.cargo/env" ]; then
            source "$HOME/.cargo/env"
        elif [ -f "/root/.cargo/env" ]; then
            source "/root/.cargo/env"
        fi
    fi

    if ! command -v cargo &> /dev/null; then
        log_error "未找到 Cargo！请先安装 Rust 环境: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    log_info "Cargo 版本: $(cargo --version)"
}

# 编译项目
build_binary() {
    log_info "正在编译 LiveDownloader Server 模式 (--no-default-features --features server)..."
    cargo build --release --no-default-features --features server

    TARGET_BIN="target/release/LiveDownloader"
    if [ ! -f "$TARGET_BIN" ]; then
        log_error "编译产物未找到: ${TARGET_BIN}"
        exit 1
    fi
}

# 1. 安装
do_install() {
    check_root
    check_env
    build_binary

    log_info "正在安装二进制文件到 ${DEST_BIN}..."
    install -m 755 "target/release/LiveDownloader" "$DEST_BIN"
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
    log_success "  LiveDownloader 服务器端安装完成！"
    log_success "===================================================="
    echo -e "${CYAN}启动服务:${NC} systemctl start livedownloader"
    echo -e "${CYAN}开机自启:${NC} systemctl enable livedownloader"
    echo -e "${CYAN}查看状态:${NC} systemctl status livedownloader"
}

# 2. 更新
do_update() {
    check_root
    check_env

    log_info "开始更新流程..."

    # 如果在 git 仓库中，尝试拉取最新代码
    if [ -d "../.git" ] || [ -d ".git" ]; then
        log_info "检测到 Git 仓库，拉取最新代码..."
        git pull || log_warn "Git pull 失败，继续使用当前本地源码进行重新编译..."
    fi

    build_binary

    log_info "停止现有 LiveDownloader 服务..."
    systemctl stop livedownloader || true

    log_info "更新二进制文件..."
    install -m 755 "target/release/LiveDownloader" "$DEST_BIN"
    ln -sf "$DEST_BIN" "$DEST_ALIAS"

    log_info "重启 LiveDownloader 服务..."
    systemctl daemon-reload || true
    systemctl restart livedownloader || systemctl start livedownloader

    log_success "===================================================="
    log_success "  LiveDownloader 服务更新完成并已重新启动！"
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
    echo -e "${CYAN}    LiveDownloader 服务端管理脚本 (Rust/Axum)       ${NC}"
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
