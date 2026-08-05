#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - Caddy HTTPS 反向代理自动卸载与清理脚本
# ==============================================================================
# 使用方法:
#   chmod +x uninstall_caddy_https.sh
#   sudo ./uninstall_caddy_https.sh
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

is_termux() {
    [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ] || [[ "${PREFIX:-}" == *"com.termux"* ]]
}

# 1. 检查 root 权限
if ! is_termux && [ "$EUID" -ne 0 ]; then
    log_error "请使用 root 权限运行此脚本 (例如: sudo $0)"
    exit 1
fi

log_info "正在清理 Caddy 反向代理配置..."

if is_termux; then
    PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
    CADDYFILE_DIR="${PREFIX}/etc/caddy"
else
    CADDYFILE_DIR="/etc/caddy"
fi

CADDYFILE_PATH="${CADDYFILE_DIR}/Caddyfile"

# 2. 查找并还原备份，或者清空配置
LATEST_BAK=$(ls -t "${CADDYFILE_DIR}"/Caddyfile.bak.* 2>/dev/null | head -n 1 || true)

if [ -n "$LATEST_BAK" ] && [ -f "$LATEST_BAK" ]; then
    log_info "检测到最近的备份文件: ${LATEST_BAK}，正在还原..."
    cp "$LATEST_BAK" "$CADDYFILE_PATH"
    log_success "已还原至备份文件！"
else
    if [ -f "$CADDYFILE_PATH" ]; then
        log_info "未找到旧的 Caddyfile 备份，清空现有的 LiveDownloader Caddy 配置..."
        rm -f "$CADDYFILE_PATH"
        touch "$CADDYFILE_PATH"
    fi
fi

# 3. 停止或重载 Caddy 服务
log_info "重载 / 停止 Caddy 服务..."
if command -v systemctl &> /dev/null && systemctl is-system-running &> /dev/null 2>&1; then
    if [ -f "$CADDYFILE_PATH" ] && [ -s "$CADDYFILE_PATH" ]; then
        systemctl reload caddy || systemctl restart caddy
        log_info "Caddy 已根据还原的配置重载。"
    else
        systemctl stop caddy || true
        log_info "Caddy 服务已停止。"
    fi
elif command -v caddy &> /dev/null; then
    if [ -f "$CADDYFILE_PATH" ] && [ -s "$CADDYFILE_PATH" ]; then
        caddy reload --config "$CADDYFILE_PATH" || true
    else
        caddy stop || true
    fi
fi

log_success "===================================================="
log_success "  Caddy 反向代理与 HTTPS 配置已成功卸载清理！"
log_success "===================================================="
