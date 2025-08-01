#!/bin/bash

# 前端容器启动脚本
# 用于在容器启动时将环境变量注入到静态文件中

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_debug() {
    if [ "${DEBUG:-false}" = "true" ]; then
        echo -e "${BLUE}[DEBUG]${NC} $1"
    fi
}

# 配置变量
NGINX_ROOT="/usr/share/nginx/html"
INDEX_FILE="${NGINX_ROOT}/index.html"
CONFIG_BACKUP="${INDEX_FILE}.template"

# 创建备份（如果不存在）
create_backup() {
    if [ ! -f "$CONFIG_BACKUP" ]; then
        log_info "创建 index.html 模板备份..."
        cp "$INDEX_FILE" "$CONFIG_BACKUP"
    fi
}

# 从备份恢复原始文件
restore_from_backup() {
    if [ -f "$CONFIG_BACKUP" ]; then
        log_debug "从备份恢复原始文件..."
        cp "$CONFIG_BACKUP" "$INDEX_FILE"
    fi
}

# 验证环境变量
validate_env_vars() {
    local errors=0
    
    log_info "验证环境变量..."
    
    if [ -z "${VITE_API_BASE_URL:-}" ]; then
        log_warn "VITE_API_BASE_URL 未设置，使用默认值"
        export VITE_API_BASE_URL="/api"
    fi
    
    if [ -z "${VITE_WS_URL:-}" ]; then
        log_warn "VITE_WS_URL 未设置，使用默认值"
        export VITE_WS_URL="/ws"
    fi
    
    # 验证 URL 格式 - 支持相对路径和绝对路径
    if ! echo "$VITE_API_BASE_URL" | grep -qE '^(https?://|/)'; then
        log_error "VITE_API_BASE_URL 格式无效: $VITE_API_BASE_URL"
        errors=$((errors + 1))
    fi
    
    if ! echo "$VITE_WS_URL" | grep -qE '^(wss?://|/)'; then
        log_error "VITE_WS_URL 格式无效: $VITE_WS_URL"
        errors=$((errors + 1))
    fi
    
    if [ $errors -gt 0 ]; then
        log_error "环境变量验证失败，退出"
        exit 1
    fi
    
    log_info "环境变量验证通过"
}

# 注入环境变量到 HTML 文件
inject_env_vars() {
    log_info "注入环境变量到 index.html..."
    
    # 确保文件存在
    if [ ! -f "$INDEX_FILE" ]; then
        log_error "index.html 文件不存在: $INDEX_FILE"
        exit 1
    fi
    
    # 从备份恢复，确保替换的是原始内容
    restore_from_backup
    
    # 执行环境变量替换
    log_debug "替换 API_BASE_URL: $VITE_API_BASE_URL"
    sed -i "s|{{VITE_API_BASE_URL}}|${VITE_API_BASE_URL}|g" "$INDEX_FILE"
    
    log_debug "替换 WS_URL: $VITE_WS_URL"
    sed -i "s|{{VITE_WS_URL}}|${VITE_WS_URL}|g" "$INDEX_FILE"
    
    log_debug "替换 APP_VERSION: ${VITE_APP_VERSION:-1.0.0}"
    sed -i "s|{{VITE_APP_VERSION}}|${VITE_APP_VERSION:-1.0.0}|g" "$INDEX_FILE"
    
    log_debug "替换 LOG_LEVEL: ${VITE_LOG_LEVEL:-info}"
    sed -i "s|{{VITE_LOG_LEVEL}}|${VITE_LOG_LEVEL:-info}|g" "$INDEX_FILE"
    
    log_info "环境变量注入完成"
}

# 验证注入结果
verify_injection() {
    log_info "验证配置注入结果..."
    
    # 检查是否还有未替换的占位符
    if grep -q "{{VITE_" "$INDEX_FILE"; then
        log_warn "发现未替换的占位符:"
        grep "{{VITE_" "$INDEX_FILE" || true
    else
        log_info "所有占位符已成功替换"
    fi
    
    # 显示当前配置（仅在调试模式下）
    if [ "${DEBUG:-false}" = "true" ]; then
        log_debug "当前配置内容:"
        grep -A 1 -B 1 'name="api-base-url"' "$INDEX_FILE" || true
        grep -A 1 -B 1 'name="ws-url"' "$INDEX_FILE" || true
    fi
}

# 设置文件权限
set_permissions() {
    log_info "设置文件权限..."
    
    # 确保 nginx 用户可以读取文件
    chown -R nginx:nginx "$NGINX_ROOT" 2>/dev/null || true
    chmod -R 755 "$NGINX_ROOT" 2>/dev/null || true
    
    log_info "文件权限设置完成"
}

# 健康检查准备
prepare_health_check() {
    log_info "准备健康检查页面..."
    
    # 创建简单的健康检查页面
    cat > "${NGINX_ROOT}/health" << 'EOF'
{
  "status": "ok",
  "service": "api-proxy-frontend",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
    
    log_info "健康检查页面创建完成"
}

# 显示启动信息
show_startup_info() {
    log_info "==================================="
    log_info "AI代理平台前端容器启动完成"
    log_info "==================================="
    log_info "API Base URL: $VITE_API_BASE_URL"
    log_info "WebSocket URL: $VITE_WS_URL"
    log_info "App Version: ${VITE_APP_VERSION:-1.0.0}"
    log_info "Log Level: ${VITE_LOG_LEVEL:-info}"
    log_info "Debug Mode: ${DEBUG:-false}"
    log_info "==================================="
}

# 主函数
main() {
    log_info "开始启动前端容器..."
    
    # 检查必要的文件和目录
    if [ ! -d "$NGINX_ROOT" ]; then
        log_error "Nginx 根目录不存在: $NGINX_ROOT"
        exit 1
    fi
    
    # 执行配置注入流程
    create_backup
    validate_env_vars
    inject_env_vars
    verify_injection
    set_permissions
    prepare_health_check
    show_startup_info
    
    log_info "启动配置完成，启动 nginx..."
    
    # 启动 nginx（前台运行）
    exec nginx -g "daemon off;"
}

# 错误处理
trap 'log_error "脚本执行失败，退出码: $?"' ERR

# 如果脚本被直接调用，执行主函数
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    main "$@"
fi