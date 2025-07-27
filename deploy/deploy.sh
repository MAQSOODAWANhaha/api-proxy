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

# 创建必要的目录和文件
prepare_environment() {
    log_step "准备部署环境"
    
    # 创建必要的目录
    mkdir -p "$SCRIPT_DIR/certs"
    mkdir -p "$SCRIPT_DIR/config"
    mkdir -p "$SCRIPT_DIR/ssl"
    mkdir -p "$SCRIPT_DIR/logs"
    
    # 创建环境变量文件（如果不存在）
    if [ ! -f "$ENV_FILE" ]; then
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
EOF
        log_success "环境配置文件已创建，请根据需要修改: $ENV_FILE"
    fi
    
    # 复制配置文件（如果存在）
    if [ -f "$PROJECT_ROOT/config.toml" ]; then
        cp "$PROJECT_ROOT/config.toml" "$SCRIPT_DIR/config/"
        log_info "已复制主配置文件到部署目录"
    fi
    
    log_success "环境准备完成"
}

# 构建镜像
build_images() {
    log_step "构建Docker镜像"
    
    cd "$SCRIPT_DIR"
    
    # 使用.env文件
    export $(grep -v '^#' "$ENV_FILE" | xargs)
    
    # 构建镜像
    docker compose build --no-cache
    
    log_success "镜像构建完成"
}

# 启动服务
start_services() {
    local profile="${1:-default}"
    
    log_step "启动服务 (profile: $profile)"
    
    cd "$SCRIPT_DIR"
    
    # 使用.env文件
    export $(grep -v '^#' "$ENV_FILE" | xargs)
    
    if [ "$profile" = "production" ]; then
        # 生产环境包括网关
        docker compose --profile production up -d
    else
        # 开发环境不包括网关
        docker compose up -d
    fi
    
    log_success "服务启动完成"
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
    local profile="${1:-default}"
    
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
    local host="${1:-localhost}"
    
    log_step "访问信息"
    
    echo ""
    log_info "🌐 前端管理界面:"
    echo "   http://$host:3000"
    
    echo ""
    log_info "🔧 后端API服务:"
    echo "   管理API: http://$host:9090/api"
    echo "   AI代理:  http://$host:8080/v1"
    
    echo ""
    log_info "📊 其他服务:"
    echo "   Redis:   $host:6379"
    
    if docker compose ps | grep -q "api-proxy-gateway"; then
        echo ""
        log_info "🚪 生产网关:"
        echo "   HTTP:  http://$host"
        echo "   HTTPS: https://$host (如果配置了SSL)"
    fi
    
    echo ""
    log_info "💡 常用命令:"
    echo "   查看状态: ./deploy.sh status"
    echo "   查看日志: ./deploy.sh logs [service]"
    echo "   重启服务: ./deploy.sh restart"
}

# 显示帮助信息
show_help() {
    cat << EOF
AI代理平台一键部署脚本

用法: $0 <命令> [选项]

命令:
  install              安装并启动所有服务
  install-prod         安装并启动生产环境（包含网关）
  start [profile]      启动服务 (default|production)
  stop                 停止服务
  restart [profile]    重启服务
  status               查看服务状态
  logs [service] [lines] 查看日志 (默认所有服务，100行)
  build                构建Docker镜像
  cleanup [--images]   清理资源（加--images删除镜像）
  backup               备份数据库
  restore <file>       恢复数据库
  info [host]          显示访问信息
  help                 显示此帮助信息

示例:
  $0 install                    # 开发环境安装
  $0 install-prod               # 生产环境安装
  $0 logs backend 50            # 查看后端服务最近50行日志
  $0 restart production         # 重启生产环境
  $0 backup                     # 备份数据库
  $0 info 192.168.1.100        # 显示指定主机的访问信息

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
            prepare_environment
            build_images
            start_services "default"
            show_access_info
            ;;
        "install-prod")
            check_docker
            prepare_environment
            build_images
            start_services "production"
            show_access_info
            ;;
        "start")
            check_docker
            start_services "$2"
            ;;
        "stop")
            stop_services
            ;;
        "restart")
            check_docker
            restart_services "$2"
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
            show_access_info "$2"
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