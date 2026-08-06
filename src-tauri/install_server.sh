#!/usr/bin/env bash
# ==============================================================================
# LiveDownloader - 多架构 (x86_64 / ARM64 / ARMv7) 服务器端安装与运维管理脚本
# ==============================================================================
# 支持架构：
#  - x86_64 / amd64 (标准 64 位 PC / 云服务器)
#  - arm64 / aarch64 (Cortex-A53 / A72 64位，树莓派4/5, 树莓派OS 64位, Oracle ARM)
#  - armv7 / armhf (Cortex-A7 / A53 32位，香橙派, 树莓派 32位)
#
# 使用方法:
#   chmod +x install_server.sh
#   sudo ./install_server.sh            # 交互式管理菜单
#   sudo ./install_server.sh install    # 直接安装
#   sudo ./install_server.sh update     # 直接更新
#   sudo ./install_server.sh start      # 启动服务
#   sudo ./install_server.sh stop       # 停止服务
#   sudo ./install_server.sh restart    # 重启服务
#   sudo ./install_server.sh status     # 查看状态
#   sudo ./install_server.sh logs       # 查看运行日志
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

is_termux() {
    [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ] || [[ "${PREFIX:-}" == *"com.termux"* ]]
}

if [ -n "${SUDO_USER:-}" ] && [ "${SUDO_USER}" != "root" ]; then
    REAL_USER="${SUDO_USER}"
    if [ -d "/home/${REAL_USER}" ]; then
        REAL_HOME="/home/${REAL_USER}"
    elif [ -d "/Users/${REAL_USER}" ]; then
        REAL_HOME="/Users/${REAL_USER}"
    else
        REAL_HOME="${HOME:-/root}"
    fi
else
    REAL_USER="$(whoami)"
    REAL_HOME="${HOME:-/root}"
fi

if is_termux; then
    PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
    DEST_BIN="${PREFIX}/bin/livedownloader"
    DEST_ALIAS="${PREFIX}/bin/ld-server"
elif [ "$EUID" -eq 0 ]; then
    DEST_BIN="/usr/bin/livedownloader"
    DEST_ALIAS="/usr/bin/ld-server"
else
    DEST_BIN="${REAL_HOME}/.local/bin/livedownloader"
    DEST_ALIAS="${REAL_HOME}/.local/bin/ld-server"
fi

WORK_DIR="${REAL_HOME}/.livedownloader"
SYSTEMD_PATH="/etc/systemd/system/livedownloader.service"
DEFAULT_PORT="10730"
PID_FILE="${WORK_DIR}/livedownloader.pid"
LOG_FILE="${WORK_DIR}/livedownloader.log"
PORT_FILE="${WORK_DIR}/livedownloader.port"

# GitHub 仓库地址
GITHUB_REPO="mega-mage/LiveDownloader"

# GitHub 加速镜像前缀（网络不佳时备选）
MIRRORS=(
    ""
    "https://ghproxy.net/"
    "https://mirror.ghproxy.com/"
    "https://ghp.ci/"
)

has_systemd() {
    if is_termux; then
        return 1
    fi
    if command -v systemctl &> /dev/null && systemctl is-system-running &> /dev/null 2>&1; then
        return 0
    elif [ -d /run/systemd/system ]; then
        return 0
    fi
    return 1
}

check_root() {
    # 支持非 root 用户直接安装运行 (将安装至 ~/.local/bin)
    return 0
}

# 检查并自动安装 FFmpeg 依赖
check_ffmpeg() {
    if command -v ffmpeg &> /dev/null; then
        log_success "检测到 FFmpeg 已安装: $(ffmpeg -version | head -n 1)"
        return 0
    fi

    log_warn "未检测到系统安装 FFmpeg！LiveDownloader 录制视频流依赖 FFmpeg。"
    read -p "是否尝试通过包管理器自动安装 FFmpeg？ [Y/n]: " inst_ff
    inst_ff=${inst_ff:-Y}
    if [[ "$inst_ff" =~ ^[Yy]$ ]]; then
        log_info "正在尝试自动安装 FFmpeg..."
        if is_termux; then
            if command -v pkg &> /dev/null; then
                pkg install -y ffmpeg
            elif command -v apt-get &> /dev/null; then
                apt-get update && apt-get install -y ffmpeg
            fi
        elif command -v apt-get &> /dev/null; then
            apt-get update && apt-get install -y ffmpeg
        elif command -v dnf &> /dev/null; then
            dnf install -y ffmpeg
        elif command -v yum &> /dev/null; then
            yum install -y ffmpeg
        elif command -v pacman &> /dev/null; then
            pacman -Sy --noconfirm ffmpeg
        elif command -v apk &> /dev/null; then
            apk add ffmpeg
        elif command -v zypper &> /dev/null; then
            zypper install -y ffmpeg
        else
            log_error "未能识别的包管理器，请手动安装 ffmpeg 后再运行本服务！"
        fi
    else
        log_warn "跳过 FFmpeg 安装，请确保稍后手动配置或放置 FFmpeg 可执行文件。"
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

    # 2. 尝试从 GitHub Releases 下载当前架构的预编译二进制（支持加速镜像重试）
    log_info "未在本地找到预编译文件，尝试从 GitHub Release 下载 [${ARCH}] 架构二进制..."
    TMP_BIN="./livedownloader-server-linux-${ARCH}"
    RAW_URL="https://github.com/${GITHUB_REPO}/releases/latest/download/livedownloader-server-linux-${ARCH}"

    DOWNLOAD_SUCCESS=false
    for mirror in "${MIRRORS[@]}"; do
        DOWNLOAD_URL="${mirror}${RAW_URL}"
        log_info "尝试下载链接: ${DOWNLOAD_URL}"
        
        if command -v curl &> /dev/null; then
            if curl -fsSL --connect-timeout 10 -o "$TMP_BIN" "$DOWNLOAD_URL"; then
                DOWNLOAD_SUCCESS=true
                break
            fi
        elif command -v wget &> /dev/null; then
            if wget -q --timeout=10 -O "$TMP_BIN" "$DOWNLOAD_URL"; then
                DOWNLOAD_SUCCESS=true
                break
            fi
        fi
    done

    if [ "$DOWNLOAD_SUCCESS" = true ] && [ -f "$TMP_BIN" ] && [ -s "$TMP_BIN" ]; then
        chmod +x "$TMP_BIN"
        log_success "成功从 GitHub Release 下载 [${ARCH}] 架构预编译二进制！"
        FOUND_BIN="$(realpath "$TMP_BIN")"
        return 0
    else
        log_warn "从 GitHub/镜像源下载预编译文件失败（可能网络受限或尚无对应 Release）。"
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
        if is_termux; then
            log_error "解决方法 (Termux): 请运行 'pkg install rust clang' 安装 Rust 工具链后再试。"
        else
            log_error "解决方法：请手动将 GitHub Release 中对应架构的二进制放置在脚本目录（命名为 livedownloader-server-linux-${ARCH}），或在服务器上安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        fi
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

# 配置防火墙端口提示与自动放行
configure_firewall() {
    local port="$1"
    if command -v ufw &> /dev/null && ufw status | grep -q "active"; then
        log_info "检测到 UFW 防火墙启用，尝试放行端口 ${port}/tcp..."
        ufw allow "${port}/tcp" || true
    elif command -v firewall-cmd &> /dev/null && systemctl is-active --quiet firewalld 2>/dev/null; then
        log_info "检测到 Firewalld 启用，尝试放行端口 ${port}/tcp..."
        firewall-cmd --permanent --add-port="${port}/tcp" || true
        firewall-cmd --reload || true
    fi
}

get_saved_port() {
    if [ -f "$PORT_FILE" ]; then
        cat "$PORT_FILE"
    else
        echo "$DEFAULT_PORT"
    fi
}

start_daemon_process() {
    local port="${1:-$(get_saved_port)}"
    if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        log_warn "LiveDownloader 服务已在运行中 (PID: $(cat "$PID_FILE"))"
        return 0
    fi
    log_info "正在后台启动 LiveDownloader (用户: ${REAL_USER}, 端口: ${port})..."
    mkdir -p "$WORK_DIR"
    chown -R "${REAL_USER}" "$WORK_DIR" 2>/dev/null || true

    cd "$WORK_DIR"
    if [ "$(whoami)" = "root" ] && [ -n "${REAL_USER}" ] && [ "${REAL_USER}" != "root" ]; then
        su - "${REAL_USER}" -c "cd '$WORK_DIR' && nohup '$DEST_BIN' --server --port '$port' > '$LOG_FILE' 2>&1 &"
    else
        nohup "$DEST_BIN" --server --port "$port" > "$LOG_FILE" 2>&1 &
    fi
    local new_pid=$!
    echo "$new_pid" > "$PID_FILE"
    echo "$port" > "$PORT_FILE"
    chown "${REAL_USER}" "$PID_FILE" "$PORT_FILE" 2>/dev/null || true
    sleep 1
    if kill -0 "$new_pid" 2>/dev/null || pgrep -u "${REAL_USER}" -f "livedownloader.*--server" >/dev/null; then
        log_success "服务已以用户 [${REAL_USER}] 身份在后台成功启动！"
    else
        log_error "服务启动失败，请检查日志: ${LOG_FILE}"
        if [ -f "$LOG_FILE" ]; then
            tail -n 20 "$LOG_FILE"
        fi
        return 1
    fi
}

stop_daemon_process() {
    if [ -f "$PID_FILE" ]; then
        local pid
        pid="$(cat "$PID_FILE")"
        if kill -0 "$pid" 2>/dev/null; then
            log_info "正在停止进程 (PID: ${pid})..."
            kill "$pid" 2>/dev/null || true
            for i in {1..10}; do
                if ! kill -0 "$pid" 2>/dev/null; then
                    break
                fi
                sleep 0.5
            done
            if kill -0 "$pid" 2>/dev/null; then
                kill -9 "$pid" 2>/dev/null || true
            fi
        fi
        rm -f "$PID_FILE"
        log_success "服务已停止！"
    else
        log_info "未发现正在运行的服务进程。"
    fi
}

# 1. 安装
do_install() {
    check_root
    check_ffmpeg
    obtain_binary

    PORT="${SERVER_PORT:-$DEFAULT_PORT}"
    if [ -t 0 ] && [ -z "$SERVER_PORT" ]; then
        read -p "请输入服务器监听端口 (默认 ${DEFAULT_PORT}): " input_port
        PORT="${input_port:-$DEFAULT_PORT}"
    fi

    log_info "正在将二进制文件安装至 ${DEST_BIN}..."
    mkdir -p "$(dirname "$DEST_BIN")"
    mkdir -p "$WORK_DIR"
    echo "$PORT" > "$PORT_FILE"

    # Termux 环境特殊配置：自动探测并配置手机公共下载目录
    if is_termux; then
        log_info "检测到当前处于 Termux 环境，正在检查存储访问权限..."
        if [ ! -d "/sdcard/Download" ] && [ ! -d "${HOME}/storage/downloads" ]; then
            log_warn "未检测到手机存储读写权限，尝试申请存储权限 (termux-setup-storage)..."
            if command -v termux-setup-storage &> /dev/null; then
                termux-setup-storage || true
                sleep 2
            fi
        fi

        TERMUX_SAVE_PATH="/sdcard/Download"
        if [ -d "/sdcard/Download" ]; then
            TERMUX_SAVE_PATH="/sdcard/Download"
        elif [ -d "${HOME}/storage/downloads" ]; then
            TERMUX_SAVE_PATH="${HOME}/storage/downloads"
        fi
        if [ -z "${TERMUX_SAVE_PATH:-}" ]; then
            TERMUX_SAVE_PATH="/sdcard/Download"
        fi
        log_success "Termux 模式默认保存目录设定为手机公共下载文件夹: ${TERMUX_SAVE_PATH}"
    fi

    # 自动生成/优化 config.toml
    CONFIG_FILE="${WORK_DIR}/config.toml"
    if [ ! -f "$CONFIG_FILE" ]; then
        log_info "生成初始配置文件 ${CONFIG_FILE}..."
        default_sp="${HOME}/downloads"
        if is_termux; then
            default_sp="${TERMUX_SAVE_PATH:-/sdcard/Download}"
        fi
        mkdir -p "$default_sp" 2>/dev/null || true
        cat <<EOF > "$CONFIG_FILE"
[settings]
language = "zh_cn"
save_path = "${default_sp}"
folder_by_author = false
folder_by_time = false
folder_by_title = false
filename_by_title = false
video_save_type = "ts"
video_record_quality = "原画"
use_proxy = false
delay_default = 300
split_mode = "time"
split_time_secs = 1200
split_size_mb = 1024
split_video_bitrate_kbps = 8000

[cookies]

[push]
tg_auto_upload = false
EOF
    else
        default_sp="${HOME}/downloads"
        if is_termux; then
            default_sp="${TERMUX_SAVE_PATH:-/sdcard/Download}"
        fi
        if grep -q 'save_path = ""' "$CONFIG_FILE" 2>/dev/null || grep -q 'save_path = "\./downloads"' "$CONFIG_FILE" 2>/dev/null; then
            log_info "自动修正/升级配置：将保存路径调整为默认下载文件夹 (${default_sp})..."
            sed -i "s|save_path = \".*\"|save_path = \"${default_sp}\"|" "$CONFIG_FILE" || true
        fi
        # 清理可能存在的空 [[rooms]] 段落，避免 TOML 解析缺少 url 报错
        if grep -q '^\[\[rooms\]\]$' "$CONFIG_FILE" 2>/dev/null; then
            sed -i '/^\[\[rooms\]\]$/d' "$CONFIG_FILE" || true
        fi
    fi

    chown -R "${REAL_USER}:${REAL_USER}" "$WORK_DIR" 2>/dev/null || true
    if [ -d "${REAL_HOME}/downloads" ]; then
        chown -R "${REAL_USER}:${REAL_USER}" "${REAL_HOME}/downloads" 2>/dev/null || true
    fi
    install -m 755 "$FOUND_BIN" "$DEST_BIN"
    ln -sf "$DEST_BIN" "$DEST_ALIAS"

    if has_systemd; then
        log_info "配置 systemd 服务 ${SYSTEMD_PATH} (监听端口: ${PORT})..."
        cat <<EOF > "$SYSTEMD_PATH"
[Unit]
Description=LiveDownloader Backend Service
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${REAL_USER}
WorkingDirectory=${WORK_DIR}
Environment="HOME=${REAL_HOME}"
ExecStart=${DEST_BIN} --server --port ${PORT}
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
        systemctl daemon-reload || true
        systemctl enable livedownloader
        systemctl restart livedownloader
    else
        log_info "检测到非 systemd 环境 (Termux/无 systemd)，使用后台进程守护模式启动..."
        stop_daemon_process
        start_daemon_process "$PORT"
    fi

    configure_firewall "$PORT"

    log_success "===================================================="
    log_success "  LiveDownloader 服务端 [${ARCH}] 安装完成并自动启动！"
    log_success "===================================================="
    echo -e "${CYAN}服务端口:${NC} ${PORT}"
    echo -e "${CYAN}数据目录:${NC} ${WORK_DIR}"
    if has_systemd; then
        echo -e "${CYAN}启动服务:${NC} systemctl start livedownloader"
        echo -e "${CYAN}停止服务:${NC} systemctl stop livedownloader"
        echo -e "${CYAN}查看状态:${NC} systemctl status livedownloader"
        echo -e "${CYAN}实时日志:${NC} journalctl -u livedownloader -f"
    else
        echo -e "${CYAN}启动服务:${NC} $0 start"
        echo -e "${CYAN}停止服务:${NC} $0 stop"
        echo -e "${CYAN}查看状态:${NC} $0 status"
        echo -e "${CYAN}实时日志:${NC} $0 logs"
    fi
}

# 2. 更新
do_update() {
    check_root

    log_info "开始更新流程..."

    # 清理旧临时文件
    rm -f ./livedownloader-server-linux-*

    if [ -d "../.git" ] || [ -d ".git" ]; then
        log_info "检测到 Git 仓库，尝试 git pull..."
        git pull || log_warn "Git pull 失败，将使用现有代码或下载最新 Release..."
    fi

    obtain_binary

    log_info "停止现有 LiveDownloader 服务..."
    if has_systemd; then
        systemctl stop livedownloader || true
    else
        stop_daemon_process
    fi

    log_info "替换二进制文件..."
    mkdir -p "$(dirname "$DEST_BIN")"
    install -m 755 "$FOUND_BIN" "$DEST_BIN"
    ln -sf "$DEST_BIN" "$DEST_ALIAS"

    log_info "重启 LiveDownloader 服务..."
    if has_systemd; then
        systemctl daemon-reload || true
        systemctl restart livedownloader || systemctl start livedownloader
    else
        start_daemon_process
    fi

    log_success "===================================================="
    log_success "  LiveDownloader 服务 [${ARCH}] 更新完成并已重新启动！"
    log_success "===================================================="
    if has_systemd; then
        systemctl status livedownloader --no-pager || true
    else
        do_status
    fi
}

# 3. 服务控制函数
do_start() {
    check_root
    if has_systemd; then
        log_info "正在启动 LiveDownloader 服务..."
        systemctl start livedownloader
        log_success "服务已启动！"
        systemctl status livedownloader --no-pager || true
    else
        start_daemon_process
    fi
}

do_stop() {
    check_root
    if has_systemd; then
        log_info "正在停止 LiveDownloader 服务..."
        systemctl stop livedownloader
        log_success "服务已停止！"
    else
        stop_daemon_process
    fi
}

do_restart() {
    check_root
    if has_systemd; then
        log_info "正在重启 LiveDownloader 服务..."
        systemctl restart livedownloader
        log_success "服务已重启！"
        systemctl status livedownloader --no-pager || true
    else
        stop_daemon_process
        start_daemon_process
    fi
}

do_status() {
    if has_systemd; then
        systemctl status livedownloader --no-pager || true
    else
        if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
            log_success "LiveDownloader 服务正在运行中 (PID: $(cat "$PID_FILE"), 端口: $(get_saved_port))"
        else
            log_warn "LiveDownloader 服务未在运行。"
        fi
    fi
}

do_logs() {
    if has_systemd; then
        journalctl -u livedownloader -f -n 100
    else
        if [ -f "$LOG_FILE" ]; then
            log_info "展示最新日志 (${LOG_FILE}):"
            tail -n 100 -f "$LOG_FILE"
        else
            log_error "未找到日志文件: ${LOG_FILE}"
        fi
    fi
}

# 4. 卸载
do_uninstall() {
    check_root

    log_warn "确定要卸载 LiveDownloader 吗？"
    read -p "请输入 [y/N] 确认卸载: " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        log_info "取消卸载。"
        exit 0
    fi

    log_info "停止服务..."
    if has_systemd; then
        systemctl stop livedownloader || true
        systemctl disable livedownloader || true

        if [ -f "$SYSTEMD_PATH" ]; then
            log_info "删除 systemd 服务配置文件..."
            rm -f "$SYSTEMD_PATH"
            systemctl daemon-reload || true
        fi
    else
        stop_daemon_process
    fi

    log_info "删除二进制文件与快捷链接..."
    rm -f "$DEST_BIN" "$DEST_ALIAS"

    log_info "清理配置文件与工作目录 (${WORK_DIR} 及 ~/.config/livedownloader)..."
    rm -rf "$WORK_DIR"
    rm -rf "${HOME}/.config/livedownloader" "${HOME}/.config/LiveDownloader"
    rm -rf "${HOME}/.livedownloader"
    if [ -n "${SUDO_USER:-}" ]; then
        rm -rf "/home/${SUDO_USER}/.config/livedownloader" "/home/${SUDO_USER}/.config/LiveDownloader"
        rm -rf "/home/${SUDO_USER}/.livedownloader"
    fi
    rm -rf "/root/.config/livedownloader" "/root/.config/LiveDownloader" "/root/.livedownloader"
    log_success "已彻底删除所有配置文件及工作目录！"

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
    echo -e " 3) 启动服务 (Start)"
    echo -e " 4) 停止服务 (Stop)"
    echo -e " 5) 重启服务 (Restart)"
    echo -e " 6) 查看状态 (Status)"
    echo -e " 7) 查看日志 (Logs)"
    echo -e " 8) 卸载 (Uninstall)"
    echo -e " 9) 退出 (Exit)"
    echo -e "${CYAN}====================================================${NC}"
    read -p "请输入选项数字 [1-9]: " CHOICE

    case "$CHOICE" in
        1) ACTION="install" ;;
        2) ACTION="update" ;;
        3) ACTION="start" ;;
        4) ACTION="stop" ;;
        5) ACTION="restart" ;;
        6) ACTION="status" ;;
        7) ACTION="logs" ;;
        8) ACTION="uninstall" ;;
        9) exit 0 ;;
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
    start)
        do_start
        ;;
    stop)
        do_stop
        ;;
    restart)
        do_restart
        ;;
    status)
        do_status
        ;;
    logs)
        do_logs
        ;;
    uninstall)
        do_uninstall
        ;;
    *)
        log_error "未知指令: ${ACTION}。可用参数: install | update | start | stop | restart | status | logs | uninstall"
        exit 1
        ;;
esac
