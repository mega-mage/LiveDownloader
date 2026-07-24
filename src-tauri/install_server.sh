#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - 服务器端编译并安装二进制到 /usr/bin 自动化脚本
# ==============================================================================
# 使用方法:
#   chmod +x install_server.sh
#   sudo ./install_server.sh
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

# 1. 检查权限
if [ "$EUID" -ne 0 ]; then
    log_error "请使用 root 权限运行此脚本 (例如: sudo $0)"
    exit 1
fi

# 2. 检查并寻找项目根目录和编译环境
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if ! command -v cargo &> /dev/null; then
    log_warn "未检测到 Rust/Cargo 环境，尝试加载当前用户的 cargo 环境变量..."
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    elif [ -f "/root/.cargo/env" ]; then
        source "/root/.cargo/env"
    fi
fi

if ! command -v cargo &> /dev/null; then
    log_error "未找到 Rust 工具链 (cargo)。请先安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

log_info "使用的 Cargo 版本: $(cargo --version)"

# 3. 编译 Web Server 模式二进制文件
log_info "正在编译 LiveDownloader Server 模式 (--no-default-features --features server)..."
cargo build --release --no-default-features --features server

TARGET_BIN="target/release/LiveDownloader"
if [ ! -f "$TARGET_BIN" ]; then
    log_error "编译产物未找到: ${TARGET_BIN}"
    exit 1
fi

# 4. 安装二进制到 /usr/bin
DEST_BIN="/usr/bin/livedownloader"
DEST_ALIAS="/usr/bin/ld-server"

log_info "正在安装二进制文件到 ${DEST_BIN}..."
install -m 755 "$TARGET_BIN" "$DEST_BIN"

# 创建别名软链接 (防止与系统 ld 命令冲突)
ln -sf "$DEST_BIN" "$DEST_ALIAS"

log_success "二进制文件成功安装至:"
echo -e "  - 主程序: ${GREEN}${DEST_BIN}${NC}"
echo -e "  - 快捷链接: ${GREEN}${DEST_ALIAS}${NC}"

# 5. 检查并询问是否生成 systemd 服务
SYSTEMD_PATH="/etc/systemd/system/livedownloader.service"

log_info "创建 systemd 服务配置文件 ${SYSTEMD_PATH}..."

cat <<EOF > "$SYSTEMD_PATH"
[Unit]
Description=LiveDownloader Backend Service
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/var/lib/livedownloader
ExecStart=/usr/bin/livedownloader --server --port 10730
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

# 创建默认工作目录
mkdir -p /var/lib/livedownloader

systemctl daemon-reload || true

log_success "===================================================="
log_success "  LiveDownloader 服务器端编译并安装完成！"
log_success "===================================================="
echo -e "${CYAN}手动启动服务:${NC} livedownloader --server --port 10730"
echo -e "${CYAN}使用 Systemd 启动:${NC} systemctl start livedownloader"
echo -e "${CYAN}设置开机自启:${NC} systemctl enable livedownloader"
echo -e "${CYAN}查看服务状态:${NC} systemctl status livedownloader"
