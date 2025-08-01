#!/bin/bash

# AI代理平台一键部署脚本
# 支持开发和生产环境的容器化部署

set -e

# ================================
# 配置变量
# ================================
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="$SCRIPT_DIR/docker compose.yaml"
ENV_FILE="$SCRIPT_DIR/.env"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ================================
# 工具函数
# ================================
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "\n${PURPLE}==> $1${NC}"
}

# 检查命令是否存在
check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "$1 未安装或不在PATH中"
        return 1
    fi
}

# 检查Docker和Docker Compose
check_docker() {
    log_step "检查Docker环境"
    
    if ! check_command docker; then
        log_error "请先安装Docker: https://docs.docker.com/get-docker/"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker守护进程未运行，请启动Docker"
        exit 1
    fi
    
    if ! check_command docker compose && ! docker compose version &> /dev/null; then
        log_error "请安装Docker Compose: https://docs.docker.com/compose/install/"
        exit 1
    fi
    
    log_success "Docker环境检查通过"
}

# 获取本地IP地址（仅开发环境使用）
get_local_ip() {
    local local_ip=""
    
    # 优先使用环境变量
    if [[ -n "$DEPLOY_IP" ]]; then
        echo "$DEPLOY_IP"
        return
    fi
    
    # 自动检测本地IP
    if command -v hostname &> /dev/null; then
        local_ip=$(hostname -I 2>/dev/null | awk '{print $1}')
    fi
    
    if [ -z "$local_ip" ] && command -v ip &> /dev/null; then
        local_ip=$(ip route get 8.8.8.8 2>/dev/null | grep -oP 'src \K\S+')
    fi
    
    if [ -z "$local_ip" ] && command -v ifconfig &> /dev/null; then
        local_ip=$(ifconfig 2>/dev/null | grep -oP 'inet \K[\d.]+' | grep -v 127.0.0.1 | head -1)
    fi
    
    # 返回检测到的IP或默认值
    echo "${local_ip:-127.0.0.1}"
}

# 创建必要的目录和文件
prepare_environment() {
    local profile="${1:-default}"
    log_step "准备部署环境 (profile: $profile)"
    
    # 创建必要的目录
    mkdir -p "$SCRIPT_DIR/certs"
    mkdir -p "$SCRIPT_DIR/config"
    mkdir -p "$SCRIPT_DIR/ssl" 
    mkdir -p "$SCRIPT_DIR/logs"
    
    # 根据环境选择配置文件
    if [ "$profile" = "production" ]; then
        CONFIG_SOURCE="config.prod.toml"
        log_info "使用生产环境配置: $CONFIG_SOURCE"
    else
        CONFIG_SOURCE="config.dev.toml"
        log_info "使用开发环境配置: $CONFIG_SOURCE"
    fi
    
    # 检查配置文件是否存在
    if [ ! -f "$SCRIPT_DIR/config/$CONFIG_SOURCE" ]; then
        log_warning "配置文件 $CONFIG_SOURCE 不存在"
    fi
    
    # 根据环境决定前端配置
    local api_base_url=""
    local ws_url=""
    
    if [ "$profile" = "production" ]; then
        # 生产环境：使用相对路径，nginx网关自动处理
        log_info "生产环境：使用相对路径配置，通过nginx网关访问"
        api_base_url="/api"
        ws_url="/ws"
    else
        # 开发环境：检测本地IP并直接访问后端
        local local_ip=$(get_local_ip)
        log_info "开发环境：检测到本地IP: $local_ip"
        api_base_url="http://${local_ip}:9090/api"
        ws_url="ws://${local_ip}:9090/ws"
    fi
    
    # 创建或更新.env文件
    log_info "创建环境配置文件: $ENV_FILE"
    cat > "$ENV_FILE" << EOF
# AI代理平台环境配置

# 应用配置
COMPOSE_PROJECT_NAME=api-proxy
COMPOSE_FILE=docker-compose.yaml

# 端口配置
FRONTEND_PORT=3000
BACKEND_API_PORT=9090
BACKEND_PROXY_PORT=8080
REDIS_PORT=6379
GATEWAY_HTTP_PORT=80
GATEWAY_HTTPS_PORT=443

# 环境设置
RUST_LOG=info
RUST_BACKTRACE=1
NODE_ENV=production

# 数据库配置
DATABASE_URL=sqlite:///app/data/api-proxy.db

# Redis配置
REDIS_URL=redis://redis:6379

# 安全配置（请修改默认值）
JWT_SECRET=$(openssl rand -base64 32 2>/dev/null || echo "change-me-in-production")
API_KEY_SECRET=$(openssl rand -base64 32 2>/dev/null || echo "change-me-in-production")

# TLS配置
TLS_ENABLED=false
TLS_CERT_PATH=/app/certs/cert.pem
TLS_KEY_PATH=/app/certs/key.pem

# 监控配置
ENABLE_METRICS=true
METRICS_PORT=9091

# 前端配置
VITE_API_BASE_URL=$api_base_url
VITE_WS_URL=$ws_url
VITE_APP_VERSION=1.0.0
VITE_LOG_LEVEL=info
VITE_USE_MOCK=false

# 后端配置文件
CONFIG_FILE=$CONFIG_SOURCE
EOF
    
    log_success "环境配置完成: API_URL=${api_base_url}, WS_URL=${ws_url}"
}

# 构建镜像
build_images() {
    log_step "构建Docker镜像"
    
    cd "$SCRIPT_DIR"
    
    # 使用.env文件 - 确保环境变量正确加载
    if [ -f "$ENV_FILE" ]; then
        set -a  # 自动导出所有变量
        source "$ENV_FILE"
        set +a  # 关闭自动导出
        log_info "已加载环境变量: VITE_API_BASE_URL=${VITE_API_BASE_URL}, CONFIG_FILE=${CONFIG_FILE}"
        log_info "注意: 前端使用运行时配置注入，环境变量将在容器启动时注入到应用中"
    fi
    
    # 构建镜像 - 新版本支持通用构建（无需构建时环境变量）
    # 环境变量将在运行时注入，因此构建阶段不再需要传递环境变量
    docker compose build --no-cache
    
    log_success "镜像构建完成"
}

# 智能检测并启动服务
detect_and_start_services() {
    log_step "智能检测环境配置并启动服务"
    
    cd "$SCRIPT_DIR"
    
    # 加载环境变量
    if [ -f "$ENV_FILE" ]; then
        set -a
        source "$ENV_FILE"
        set +a
        
        # 智能检测：相对路径 = 生产环境，绝对路径 = 开发环境
        if [[ "$VITE_API_BASE_URL" == "/api"* ]]; then
            log_info "检测到生产环境配置 (API URL: $VITE_API_BASE_URL)"
            log_info "启动完整服务栈，包含nginx网关"
            docker compose --env-file "$ENV_FILE" --profile production up -d
        else
            log_info "检测到开发环境配置 (API URL: $VITE_API_BASE_URL)"
            log_info "启动开发服务栈，直接暴露端口"
            docker compose --env-file "$ENV_FILE" up -d
        fi
    else
        log_error "未找到环境配置文件: $ENV_FILE"
        log_info "请先运行 ./deploy.sh install 或 ./deploy.sh install-prod"
        exit 1
    fi
    
    log_success "服务启动完成"
}

# 启动服务
start_services() {
    local profile="${1:-auto}"
    
    # 如果没有明确指定profile或指定为auto，则智能检测
    if [ "$profile" = "auto" ] || [ -z "$profile" ]; then
        detect_and_start_services
    else
        log_step "启动服务 (profile: $profile)"
        
        cd "$SCRIPT_DIR"
        
        # 使用.env文件
        if [ -f "$ENV_FILE" ]; then
            set -a  # 自动导出所有变量
            source "$ENV_FILE"
            set +a  # 关闭自动导出
        fi
        
        if [ "$profile" = "production" ]; then
            # 生产环境包括网关
            docker compose --env-file "$ENV_FILE" --profile production up -d
        else
            # 开发环境不包括网关
            docker compose --env-file "$ENV_FILE" up -d
        fi
        
        log_success "服务启动完成"
    fi
}

# 停止服务
stop_services() {
    log_step "停止服务"
    
    cd "$SCRIPT_DIR"
    docker compose down
    
    log_success "服务已停止"
}

# 重启服务
restart_services() {
    local profile="${1:-auto}"
    
    log_step "重启服务"
    
    stop_services
    start_services "$profile"
    
    log_success "服务重启完成"
}

# 查看服务状态
show_status() {
    log_step "服务状态"
    
    cd "$SCRIPT_DIR"
    docker compose ps
    
    echo ""
    log_info "服务健康状态:"
    docker compose exec backend curl -f http://localhost:9090/api/health 2>/dev/null && log_success "后端API服务正常" || log_warning "后端API服务异常"
    docker compose exec frontend curl -f http://localhost/health 2>/dev/null && log_success "前端服务正常" || log_warning "前端服务异常"
    docker compose exec redis redis-cli ping 2>/dev/null && log_success "Redis服务正常" || log_warning "Redis服务异常"
}

# 查看日志
show_logs() {
    local service="$1"
    local lines="${2:-100}"
    
    cd "$SCRIPT_DIR"
    
    if [ -n "$service" ]; then
        log_step "查看 $service 服务日志 (最近 $lines 行)"
        docker compose logs --tail="$lines" -f "$service"
    else
        log_step "查看所有服务日志 (最近 $lines 行)"
        docker compose logs --tail="$lines" -f
    fi
}

# 清理资源
cleanup() {
    log_step "清理Docker资源"
    
    cd "$SCRIPT_DIR"
    
    # 停止并删除容器
    docker compose down --volumes --remove-orphans
    
    # 删除镜像（可选）
    if [ "$1" = "--images" ]; then
        docker compose down --rmi all
        log_info "已删除相关镜像"
    fi
    
    # 清理未使用的资源
    docker system prune -f
    
    log_success "清理完成"
}

# 数据库操作
database_operation() {
    local operation="$1"
    
    case "$operation" in
        "backup")
            log_step "备份数据库"
            mkdir -p "$SCRIPT_DIR/backups"
            backup_file="$SCRIPT_DIR/backups/backup-$(date +%Y%m%d-%H%M%S).db"
            docker compose exec backend cp /app/data/api-proxy.db "/app/backups/$(basename "$backup_file")"
            log_success "数据库已备份到: $backup_file"
            ;;
        "restore")
            local backup_file="$2"
            if [ -z "$backup_file" ] || [ ! -f "$backup_file" ]; then
                log_error "请指定有效的备份文件"
                exit 1
            fi
            log_step "恢复数据库"
            docker compose exec backend cp "/app/backups/$(basename "$backup_file")" /app/data/api-proxy.db
            docker compose restart backend
            log_success "数据库已恢复"
            ;;
        *)
            log_error "未知的数据库操作: $operation"
            exit 1
            ;;
    esac
}

# 显示访问信息
show_access_info() {
    log_step "部署完成"
    
    echo ""
    echo -e "${GREEN}==================== 🎉 部署成功 ====================${NC}"
    echo ""
    
    # 检测当前运行的环境
    local is_production=false
    if docker compose ps | grep -q "api-proxy-gateway"; then
        is_production=true
    fi
    
    if [ "$is_production" = true ]; then
        echo -e "${BLUE}🌍 生产环境 - 通过nginx网关访问:${NC}"
        echo -e "  📱 前端界面: ${GREEN}http://您的服务器IP${NC} ${YELLOW}← 主要访问入口${NC}"
        echo -e "  🔧 管理API:  ${GREEN}http://您的服务器IP/api${NC}"
        echo -e "  🤖 AI代理:   ${GREEN}http://您的服务器IP/v1${NC}"
        echo ""
        echo -e "${YELLOW}📌 生产环境特点:${NC}"
        echo "  • 所有请求通过80端口nginx网关统一入口"
        echo "  • 前端使用相对路径，自动适配域名"
        echo "  • 支持SSL/TLS加密（需配置证书）"
        echo "  • 适合公网部署和生产使用"
        echo ""
        echo -e "${BLUE}🔧 直接访问后端（调试用）:${NC}"
        echo "  • 管理API: http://您的服务器IP:9090/api"
        echo "  • AI代理: http://您的服务器IP:8080/v1"
    else
        # 从.env获取IP地址用于显示
        local dev_ip="localhost"
        if [ -f "$ENV_FILE" ]; then
            local api_url=$(grep "^VITE_API_BASE_URL=" "$ENV_FILE" | cut -d'=' -f2)
            if [[ "$api_url" == http://* ]]; then
                dev_ip=$(echo "$api_url" | sed 's|http://||' | sed 's|:.*||')
            fi
        fi
        
        echo -e "${BLUE}🛠️ 开发环境 - 直接端口访问:${NC}"
        echo -e "  📱 前端界面: ${GREEN}http://${dev_ip}:3000${NC} ${YELLOW}← 主要访问入口${NC}"
        echo -e "  🔧 管理API:  ${GREEN}http://${dev_ip}:9090/api${NC}"
        echo -e "  🤖 AI代理:   ${GREEN}http://${dev_ip}:8080/v1${NC}"
        echo -e "  📊 Redis:    ${GREEN}redis://${dev_ip}:6379${NC}"
        echo ""
        echo -e "${YELLOW}📌 开发环境特点:${NC}"
        echo "  • 各服务独立端口，便于调试"
        echo "  • 无网关层，直接访问后端服务"
        echo "  • 适合本地开发和测试"
    fi
    
    echo ""
    echo -e "${BLUE}⚙️ 管理命令:${NC}"
    echo -e "  📊 查看状态: ${GREEN}./deploy.sh status${NC}"
    echo -e "  📋 查看日志: ${GREEN}./deploy.sh logs [service]${NC}"
    echo -e "  ⏹️  停止服务: ${GREEN}./deploy.sh stop${NC}"
    echo -e "  🔄 重启服务: ${GREEN}./deploy.sh restart${NC}"
    echo ""
    echo -e "${BLUE}🚀 部署提示:${NC}"
    echo "  • 生产环境：零配置，nginx自动处理域名和路径"
    echo "  • 开发环境：如需外部访问，设置 DEPLOY_IP=你的IP"
    echo "  • 环境切换：重新运行对应的 install 命令即可"
    echo ""
    echo -e "${GREEN}==================================================${NC}"
}

# 显示帮助信息
show_help() {
    cat << EOF
AI代理平台一键部署脚本

用法: $0 <命令> [选项]

核心命令:
  install              开发环境 - 直接端口访问，需要IP配置
  install-prod         生产环境 - nginx网关，零IP配置
  start                智能启动 - 自动检测环境配置
  stop                 停止所有服务
  restart              重启服务（保持当前环境配置）

管理命令:
  status               查看服务运行状态
  logs [service]       查看服务日志
  build                重新构建Docker镜像
  cleanup [--images]   清理Docker资源
  backup               备份数据库
  restore <file>       恢复数据库
  help                 显示此帮助信息

环境说明:
  开发环境 (install)：
    • 各服务独立端口：前端:3000, 后端:9090, Redis:6379
    • 需要检测本地IP地址，支持外部访问
    • 无nginx网关，直接访问各服务
    • 适合：本地开发、调试、测试

  生产环境 (install-prod)：
    • 统一nginx网关入口，仅使用80/443端口
    • 前端使用相对路径，自动适配域名
    • 零IP配置，部署即用
    • 适合：公网部署、生产使用

环境变量:
  DEPLOY_IP=<IP>       指定开发环境使用的IP地址

使用示例:
  ./deploy.sh install-prod         # 生产环境，零配置部署
  ./deploy.sh install              # 开发环境，自动检测IP
  DEPLOY_IP=192.168.1.100 ./deploy.sh install  # 指定IP的开发环境
  ./deploy.sh logs backend         # 查看后端日志
  ./deploy.sh restart              # 重启（保持环境）

EOF
}

# ================================
# 主程序
# ================================
main() {
    local command="$1"
    
    case "$command" in
        "install")
            check_docker
            prepare_environment "default"
            build_images
            start_services "default"
            show_access_info
            ;;
        "install-prod")
            check_docker
            prepare_environment "production"
            build_images
            start_services "production"
            show_access_info
            ;;
        "start")
            check_docker
            detect_and_start_services
            ;;
        "stop")
            stop_services
            ;;
        "restart")
            check_docker
            restart_services "${2:-auto}"
            ;;
        "status")
            show_status
            ;;
        "logs")
            show_logs "$2" "$3"
            ;;
        "build")
            check_docker
            build_images
            ;;
        "cleanup")
            cleanup "$2"
            ;;
        "backup")
            database_operation "backup"
            ;;
        "restore")
            database_operation "restore" "$2"
            ;;
        "info")
            # 如果用户指定了IP地址，使用指定的IP；否则从.env文件读取
            local info_ip="$2"
            if [ -z "$info_ip" ] && [ -f "$ENV_FILE" ]; then
                info_ip=$(grep "^VITE_API_BASE_URL=" "$ENV_FILE" | sed 's|.*://||' | sed 's|/.*||' | sed 's|:.*||')
            fi
            show_access_info "production" "$info_ip"
            ;;
        "help"|"--help"|"-h"|"")
            show_help
            ;;
        *)
            log_error "未知命令: $command"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# 运行主程序
main "$@"