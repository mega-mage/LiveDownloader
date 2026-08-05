#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - Caddy HTTPS 反向代理自动安装与配置脚本
# ==============================================================================
# 适用于 Debian / Ubuntu / CentOS / RHEL / Fedora 等 Linux 发行版
# 使用方法:
#   chmod +x setup_caddy_https.sh
#   sudo ./setup_caddy_https.sh [域名] [后端端口]
#   例如: sudo ./setup_caddy_https.sh live.example.com 10730
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

# 2. 获取参数或交互式输入
DOMAIN="$1"
BACKEND_PORT="${2:-10730}"

if [ -z "$DOMAIN" ]; then
    echo -e "${CYAN}请输入你要绑定的域名 (例如: live.yourdomain.com):${NC}"
    read -r DOMAIN
fi

if [ -z "$DOMAIN" ]; then
    log_error "域名不能为空！"
    exit 1
fi

log_info "目标域名: ${DOMAIN}"
log_info "LiveDownloader 后端端口: ${BACKEND_PORT}"

# 3. 检查并安装 Caddy
if ! command -v caddy &> /dev/null; then
    log_info "未检测到 Caddy，正在安装 Caddy..."

    if is_termux; then
        if command -v pkg &> /dev/null; then
            pkg install -y caddy
        elif command -v apt-get &> /dev/null; then
            apt-get update -y && apt-get install -y caddy
        fi
    elif [ -f /etc/debian_version ]; then
        # Debian / Ubuntu
        apt-get update -y && apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg --yes
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
        apt-get update -y
        apt-get install -y caddy
    elif [ -f /etc/redhat-release ]; then
        # CentOS / RHEL / Fedora
        dnf install -y 'dnf-command(copr)' || yum install -y yum-plugin-copr
        dnf copr enable -y @caddy/caddy || yum copr enable -y @caddy/caddy
        dnf install -y caddy || yum install -y caddy
    else
        log_error "未能自动识别你的 Linux 发行版，请先手动安装 Caddy。"
        exit 1
    fi

    log_success "Caddy 安装成功！"
else
    log_info "检测到系统已安装 Caddy: $(caddy version)"
fi

# 4. 配置防火墙提示
log_info "配置防火墙开放 80 和 443 端口..."
if command -v ufw &> /dev/null; then
    ufw allow 80/tcp || true
    ufw allow 443/tcp || true
    ufw reload || true
elif command -v firewall-cmd &> /dev/null && systemctl is-active --quiet firewalld 2>/dev/null; then
    firewall-cmd --permanent --add-service=http || true
    firewall-cmd --permanent --add-service=https || true
    firewall-cmd --reload || true
fi

# 5. 生成 Caddyfile 配置文件
if is_termux; then
    PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
    CADDYFILE_DIR="${PREFIX}/etc/caddy"
else
    CADDYFILE_DIR="/etc/caddy"
fi
mkdir -p "$CADDYFILE_DIR"
CADDYFILE_PATH="${CADDYFILE_DIR}/Caddyfile"
BACKUP_PATH="${CADDYFILE_DIR}/Caddyfile.bak.$(date +%Y%m%d_%H%M%S)"

if [ -f "$CADDYFILE_PATH" ]; then
    log_info "备份旧的 Caddyfile 到 ${BACKUP_PATH}..."
    cp "$CADDYFILE_PATH" "$BACKUP_PATH"
fi

log_info "正在生成新的 Caddyfile 配置..."

cat <<EOF > "$CADDYFILE_PATH"
# LiveDownloader Caddy HTTPS 反向代理配置
$DOMAIN {
    # 启用 gzip / zstd 压缩
    encode zstd gzip

    # 针对实时直播流 (/live/*) 和 代理流 (/proxy/*) 禁用响应缓冲以实现零延迟播放
    @streaming {
        path /live* /proxy* /api/videos/download* /api/video/download*
    }
    handle @streaming {
        reverse_proxy 127.0.0.1:$BACKEND_PORT {
            flush_interval -1
            header_up Host {host}
            header_up X-Real-IP {remote_host}
            header_up X-Forwarded-For {remote_host}
            header_up X-Forwarded-Proto {scheme}
        }
    }

    # 其余所有请求 (API 与 Web 服务) 的标准反向代理
    handle {
        reverse_proxy 127.0.0.1:$BACKEND_PORT {
            header_up Host {host}
            header_up X-Real-IP {remote_host}
            header_up X-Forwarded-For {remote_host}
            header_up X-Forwarded-Proto {scheme}
        }
    }
}
EOF

# 6. 验证 Caddy 配置并启动服务
log_info "校验 Caddyfile 语法..."
if caddy validate --config "$CADDYFILE_PATH"; then
    log_success "Caddyfile 语法验证通过！"
else
    log_error "Caddyfile 语法错误，正在还原备份..."
    if [ -f "$BACKUP_PATH" ]; then
        cp "$BACKUP_PATH" "$CADDYFILE_PATH"
    fi
    exit 1
fi

log_info "重载 Caddy 服务..."
if command -v systemctl &> /dev/null && systemctl is-system-running &> /dev/null 2>&1; then
    systemctl enable caddy || true
    systemctl restart caddy || systemctl start caddy
else
    caddy reload --config "$CADDYFILE_PATH" || caddy start --config "$CADDYFILE_PATH" &
fi

log_success "===================================================="
log_success "  Caddy 反向代理与 HTTPS 配置完成！"
log_success "===================================================="
echo -e "${CYAN}访问地址:${NC} https://${DOMAIN}"
echo -e "${CYAN}后端接口:${NC} https://${DOMAIN}/api/rooms"
echo -e "${YELLOW}提示: 请确保你的域名 DNS 解析已指向此服务器 IP 地址，且云服务器安全组已开放 80 和 443 端口。${NC}"
