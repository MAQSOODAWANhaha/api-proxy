#!/bin/bash

# AI代理平台一键部署脚本
# 支持前后端统一部署和Caddy反向代理

set -e

# ================================
# 配置变量
# ================================
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yaml"
ENV_FILE="$SCRIPT_DIR/.env.production"

# TLS证书配置
TLS_MODE="${TLS_MODE:-auto}"  # auto|selfsigned
# DOMAIN 直接从环境变量读取，CERT_EMAIL 在需要时处理

# IP模式配置 (将在函数定义后初始化)
LOCAL_IP="${LOCAL_IP:-}"

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

# 生成安全的随机密钥
generate_secure_key() {
    if command -v openssl &> /dev/null; then
        openssl rand -base64 32
    elif command -v head &> /dev/null && [ -r /dev/urandom ]; then
        head -c 32 /dev/urandom | base64
    else
        # 备用方法：使用date和随机数生成
        echo -n "$(date +%s)$(shuf -i 1000-9999 -n 1)$(hostname)" | sha256sum | cut -d' ' -f1 | head -c 32 | base64
    fi
}

# 从现有环境文件中提取值
extract_env_value() {
    local key="$1"
    local env_file="$2"
    
    if [[ -f "$env_file" ]]; then
        grep "^${key}=" "$env_file" 2>/dev/null | cut -d'=' -f2- | head -1
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

# 交互式选择TLS配置 (简化版)
interactive_tls_setup() {
    log_step "TLS证书配置选择"
    
    echo ""
    echo -e "${BLUE}请选择TLS证书类型:${NC}"
    echo "1) 自签名证书 (测试环境，基于IP地址)"
    echo "2) 域名证书 (生产环境，需要有效域名)"
    echo ""
    
    while true; do
        read -p "请选择 (1 或 2): " cert_choice
        case $cert_choice in
            1)
                TLS_MODE="selfsigned"
                log_info "已选择：自签名证书模式"
                
                # 获取并确认IP地址
                auto_ip=$(get_local_ip)
                echo ""
                echo -e "${BLUE}IP地址配置:${NC}"
                if [[ -n "$auto_ip" ]]; then
                    echo "检测到本机IP: $auto_ip"
                    read -p "使用此IP？(y/n，默认y): " use_auto_ip
                    if [[ "$use_auto_ip" != "n" && "$use_auto_ip" != "N" ]]; then
                        LOCAL_IP="$auto_ip"
                    fi
                fi
                
                if [[ -z "$LOCAL_IP" ]]; then
                    while true; do
                        read -p "请输入IP地址: " manual_ip
                        if [[ "$manual_ip" =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
                            LOCAL_IP="$manual_ip"
                            break
                        else
                            log_error "IP地址格式无效，请重新输入"
                        fi
                    done
                else
                    # 只有当使用自动检测的IP时，才询问是否需要额外IP
                    echo ""
                    echo -e "${YELLOW}提示：如果需要外网访问，建议添加外网IP到证书中${NC}"
                    echo "例如：如果您的外网IP是 3.92.178.170，请在下面输入"
                    read -p "需要添加额外IP吗？(多个IP用逗号分隔，回车跳过): " extra_ips
                    if [[ -n "$extra_ips" ]]; then
                        EXTRA_IPS="$extra_ips"
                        log_info "额外IP: $EXTRA_IPS"
                    fi
                fi
                
                log_success "将使用自签名证书，主IP: $LOCAL_IP"
                log_info "配置摘要: TLS_MODE=$TLS_MODE, LOCAL_IP=$LOCAL_IP"
                break
                ;;
            2)
                TLS_MODE="auto"
                echo ""
                echo -e "${BLUE}域名配置:${NC}"
                
                # 域名是必填项，必须验证
                while true; do
                    read -p "请输入域名 (必填): " user_domain
                    if [[ -n "$user_domain" && "$user_domain" != "example.com" ]]; then
                        DOMAIN="$user_domain"
                        break
                    else
                        log_error "域名不能为空且不能是示例域名，请输入真实域名"
                    fi
                done
                
                # 邮箱验证
                while true; do
                    read -p "请输入证书申请邮箱 (默认: admin@$DOMAIN): " user_email
                    if [[ -n "$user_email" ]]; then
                        # 简单邮箱格式验证
                        if [[ "$user_email" =~ ^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$ ]]; then
                            CERT_EMAIL="$user_email"
                            break
                        else
                            log_error "邮箱格式无效，请重新输入"
                        fi
                    else
                        CERT_EMAIL="admin@$DOMAIN"
                        break
                    fi
                done
                
                log_success "将使用域名证书，域名: $DOMAIN，邮箱: $CERT_EMAIL"
                log_info "配置摘要: TLS_MODE=$TLS_MODE, DOMAIN=$DOMAIN, CERT_EMAIL=$CERT_EMAIL"
                break
                ;;
            *)
                log_error "无效选择，请输入 1 或 2"
                ;;
        esac
    done
}

# 获取本地IP地址
get_local_ip() {
    local local_ip=""
    
    # 优先使用环境变量 LOCAL_IP
    if [[ -n "$LOCAL_IP" ]]; then
        echo "$LOCAL_IP"
        return
    fi
    
    # 优先使用环境变量 DEPLOY_IP（向后兼容）
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
    
    # 只返回检测到的IP，不使用默认值
    echo "$local_ip"
}

# 验证并确保获取到有效的本机IP地址
ensure_local_ip() {
    if [[ -z "$LOCAL_IP" ]]; then
        LOCAL_IP=$(get_local_ip)
    fi
    
    # 如果自动检测失败或IP格式无效，强制要求用户输入
    while [[ -z "$LOCAL_IP" || ! "$LOCAL_IP" =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; do
        if [[ -n "$LOCAL_IP" ]]; then
            log_error "检测到无效的IP地址格式: $LOCAL_IP"
        else
            log_warning "无法自动检测本机IP地址"
        fi
        
        echo -e "${YELLOW}请手动输入本机IP地址（例如：192.168.1.100）${NC}"
        read -p "本机IP地址: " manual_ip
        
        if [[ "$manual_ip" =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
            LOCAL_IP="$manual_ip"
            log_success "使用手动输入的IP地址: $LOCAL_IP"
            break
        else
            log_error "输入的IP地址格式无效，请重新输入"
        fi
    done
    
    log_info "确认使用IP地址: $LOCAL_IP"
}

# ================================
# TLS证书管理函数
# ================================


# 生成基于IP的自签名证书（简化版）
generate_ip_self_signed_cert() {
    log_step "生成基于IP的自签名TLS证书"
    
    local cert_dir="$SCRIPT_DIR/certs"
    local cert_file="$cert_dir/server.crt"
    local key_file="$cert_dir/server.key"
    
    log_info "主要IP地址: $LOCAL_IP"
    
    # 检查是否已存在有效证书
    if [[ -f "$cert_file" && -f "$key_file" ]]; then
        if openssl x509 -in "$cert_file" -checkend 604800 -noout &>/dev/null; then
            log_success "现有证书仍然有效（7天内不会过期）"
            return 0
        fi
    fi
    
    # 确保证书目录存在
    mkdir -p "$cert_dir"
    
    # 简化的证书生成 - 使用OpenSSL直接生成
    log_info "生成简化自签名证书..."
    
    # 创建Subject Alternative Name扩展
    local san_ext=""
    san_ext="DNS:localhost,DNS:*.localhost,IP:127.0.0.1"
    
    # 添加主要IP
    if [[ -n "$LOCAL_IP" ]]; then
        san_ext="$san_ext,IP:$LOCAL_IP"
        log_info "  添加主要IP: $LOCAL_IP"
    fi
    
    # 检测内网IP地址
    local internal_ips
    internal_ips=$(hostname -I 2>/dev/null | xargs -n1 | grep -E '^(10\.|192\.168\.|172\.1[6-9]\.|172\.2[0-9]\.|172\.3[0-1]\.)' | head -2)
    for ip in $internal_ips; do
        if [[ "$ip" != "$LOCAL_IP" ]]; then
            san_ext="$san_ext,IP:$ip"
            log_info "  添加内网IP: $ip"
        fi
    done
    
    log_info "证书将支持以下访问方式:"
    log_info "  - https://localhost:8443"
    if [[ -n "$LOCAL_IP" ]]; then
        log_info "  - https://$LOCAL_IP:8443"
    fi
    
    # 使用OpenSSL一步生成自签名证书
    openssl req -x509 -newkey rsa:2048 -keyout "$key_file" -out "$cert_file" \
        -days 365 -nodes \
        -subj "/C=CN/ST=Cloud/L=Internet/O=AI Proxy Platform/OU=Development/CN=${LOCAL_IP:-localhost}" \
        -addext "subjectAltName=$san_ext" \
        -addext "keyUsage=keyEncipherment,dataEncipherment,digitalSignature" \
        -addext "extendedKeyUsage=serverAuth"
    
    # 设置权限
    chmod 600 "$key_file"
    chmod 644 "$cert_file"
    
    log_success "简化自签名证书生成完成: $cert_file"
    log_info "证书有效期: 365天"
    
    # 显示证书详情
    log_info "证书详情:"
    openssl x509 -in "$cert_file" -text -noout | grep -A 5 "Subject Alternative Name" 2>/dev/null || log_warning "无法读取SAN信息"
}

# 简单检查域名证书是否存在且有效
check_existing_cert() {
    local domain="$1"
    
    # 检查Caddy容器是否存在
    if ! docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q caddy &>/dev/null; then
        return 1  # 容器不存在，需要申请证书
    fi
    
    local container_id
    container_id=$(docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q caddy)
    
    if [[ -n "$container_id" ]]; then
        # 检查证书文件是否存在
        local cert_files
        cert_files=$(docker exec "$container_id" find /data/caddy/certificates -name "*${domain}*" -name "*.crt" 2>/dev/null)
        
        if [[ -n "$cert_files" ]]; then
            # 找到证书文件，检查有效期
            local cert_file
            cert_file=$(echo "$cert_files" | head -1)
            
            # 检查证书是否在30天内过期
            if docker exec "$container_id" openssl x509 -in "$cert_file" -checkend 2592000 -noout &>/dev/null 2>&1; then
                log_success "发现有效证书，跳过申请避免Let's Encrypt速率限制"
                return 0  # 证书有效
            else
                log_warning "现有证书即将过期，需要续期"
            fi
        fi
    fi
    
    return 1  # 需要申请证书
}

# 配置Caddy证书模式（只负责生成Caddyfile）
setup_caddy_tls() {
    log_step "生成Caddy配置文件"
    
    local caddyfile="$SCRIPT_DIR/Caddyfile"
    local cert_dir="$SCRIPT_DIR/certs"
    
    # 简单读取环境变量（环境文件已在prepare_environment中生成）
    if [ -f "$ENV_FILE" ]; then
        set -a
        source "$ENV_FILE"
        set +a
        log_info "读取环境配置: TLS_MODE=$TLS_MODE"
    else
        log_error "未找到环境配置文件: $ENV_FILE"
        return 1
    fi
    
    case "$TLS_MODE" in
        "selfsigned")
            log_info "使用IP地址自签名证书模式"
            log_info "生成自签名证书Caddyfile: LOCAL_IP=$LOCAL_IP"
            generate_ip_self_signed_cert
            
            # 创建基于IP的自签名证书Caddyfile
            # 初始化LOCAL_IP（如果还没有初始化）
            if [[ -z "$LOCAL_IP" ]]; then
                LOCAL_IP=$(get_local_ip)
            fi
            
            cat > "$caddyfile" << EOF
# 简化的Caddy配置文件 - 直接端口转发

# ================================
# 全局选项
# ================================
{
    auto_https disable_redirects
    admin :2019
    log {
        level INFO
    }
}

# ================================
# 443端口 HTTPS -> 9090端口
# ================================
:443 {
    tls /etc/caddy/certs/server.crt /etc/caddy/certs/server.key
    
    reverse_proxy proxy:9090 {
        header_up Host {http.request.host}
        header_up X-Real-IP {http.request.remote.host}
        header_up X-Forwarded-For {http.request.remote.host}
        header_up X-Forwarded-Proto {http.request.scheme}
    }
    
    log {
        output file /var/log/caddy/443.log
    }
}

# ================================
# 8443端口 HTTPS -> 8080端口
# ================================
:8443 {
    tls /etc/caddy/certs/server.crt /etc/caddy/certs/server.key
    
    reverse_proxy proxy:8080 {
        header_up Host {http.request.host}
        header_up X-Real-IP {http.request.remote.host}
        header_up X-Forwarded-For {http.request.remote.host}
        header_up X-Forwarded-Proto {http.request.scheme}
    }
    
    log {
        output file /var/log/caddy/8443.log
    }
}

# ================================
# 80端口 HTTP -> 9090端口
# ================================
:80 {
    reverse_proxy proxy:9090 {
        header_up Host {http.request.host}
        header_up X-Real-IP {http.request.remote.host}
        header_up X-Forwarded-For {http.request.remote.host}
        header_up X-Forwarded-Proto {http.request.scheme}
    }
    
    log {
        output file /var/log/caddy/80.log
    }
}
EOF
            ;;
            
        "auto"|"")
            log_info "使用自动域名证书模式（Let's Encrypt）"
            log_info "生成域名证书Caddyfile: DOMAIN=$DOMAIN, CERT_EMAIL=$CERT_EMAIL"
            
            # 检查现有证书，避免重复申请
            if check_existing_cert "$DOMAIN"; then
                log_info "检测到有效证书，Caddy将复用现有证书"
            else
                log_info "未发现有效证书，Caddy将申请新证书（注意Let's Encrypt速率限制）"
            fi
            
            cat > "$caddyfile" << EOF
# AI代理平台 Caddy 配置文件 - 智能证书管理

# ================================
# 全局选项
# ================================
{
    # 自动HTTPS
    auto_https disable_redirects
    
    # 证书申请邮箱
    email $CERT_EMAIL
    
    # 管理端点
    admin :2019
    
    # 日志级别
    log {
        level INFO
    }
    
    # Let's Encrypt配置
    acme_ca https://acme-v02.api.letsencrypt.org/directory
}

# ================================
# 主域名 HTTPS (443端口) - 智能证书管理
# ================================
$DOMAIN {
    # 健康检查端点
    handle /health {
        respond "OK - Smart TLS Management" 200
    }
    
    # 管理API和前端 - 转发到9090端口
    handle /* {
        reverse_proxy proxy:9090 {
            header_up Host {http.request.host}
            header_up X-Real-IP {http.request.remote.host}
            header_up X-Forwarded-For {http.request.remote.host}
            header_up X-Forwarded-Proto {http.request.scheme}
        }
    }
    
    # 访问日志
    log {
        output file /var/log/caddy/domain.log {
            roll_size 100mb
            roll_keep 10
        }
        format json
    }
}

# ================================
# 8443端口 HTTPS 转发 - 智能证书管理 (AI代理服务)
# ================================
$DOMAIN:8443 {
    # 健康检查端点
    handle /health {
        respond "OK - Port 8443 Smart TLS" 200
    }
    
    # AI代理服务 - 转发到8080端口
    handle /* {
        reverse_proxy proxy:8080 {
            header_up Host {http.request.host}
            header_up X-Real-IP {http.request.remote.host}
            header_up X-Forwarded-For {http.request.remote.host}
            header_up X-Forwarded-Proto {http.request.scheme}
        }
    }
}

# ================================
# HTTP重定向到HTTPS
# ================================
http://$DOMAIN {
    redir https://$DOMAIN{uri} permanent
}
EOF
            ;;
            
            
        *)
            log_error "不支持的TLS模式: $TLS_MODE"
            log_info "支持的模式: auto, selfsigned"
            return 1
            ;;
    esac
    
    log_success "Caddy配置文件生成完成: $TLS_MODE 模式"
}

# 查看证书状态
show_cert_status() {
    log_step "TLS证书状态检查"
    
    local cert_dir="$SCRIPT_DIR/certs"
    local cert_file=""
    
    # 根据TLS模式选择正确的证书文件
    if [[ "$TLS_MODE" == "selfsigned" ]]; then
        cert_file="$cert_dir/server.crt"
    else
        cert_file="$cert_dir/$DOMAIN.crt"
    fi
    
    echo ""
    log_info "当前配置:"
    echo "  TLS模式: $TLS_MODE"
    if [[ "$TLS_MODE" == "selfsigned" ]]; then
        echo "  本机IP: $LOCAL_IP"
    else
        echo "  域名: $DOMAIN"
        echo "  证书邮箱: $CERT_EMAIL"
    fi
    
    echo ""
    if [[ -f "$cert_file" ]]; then
        log_info "本地证书文件: $cert_file"
        
        # 检查证书有效期
        local expiry_date
        expiry_date=$(openssl x509 -in "$cert_file" -noout -enddate 2>/dev/null | cut -d= -f2)
        if [[ -n "$expiry_date" ]]; then
            echo "  有效期至: $expiry_date"
            
            # 检查是否即将过期
            if openssl x509 -in "$cert_file" -checkend 604800 -noout &>/dev/null; then
                log_success "证书有效（7天内不会过期）"
            else
                log_warning "证书即将在7天内过期！"
            fi
        fi
        
        # 显示证书详情
        local subject
        subject=$(openssl x509 -in "$cert_file" -noout -subject 2>/dev/null | cut -d= -f2-)
        [[ -n "$subject" ]] && echo "  主体: $subject"
        
        # 显示SAN列表
        local sans
        sans=$(openssl x509 -in "$cert_file" -noout -text 2>/dev/null | grep -A1 "Subject Alternative Name" | tail -n1 | sed 's/.*DNS:/DNS:/g')
        [[ -n "$sans" ]] && echo "  SAN: $sans"
    else
        log_warning "未找到本地证书文件"
    fi
    
    echo ""
    log_info "Caddy证书状态:"
    if docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q caddy &>/dev/null; then
        local container_id
        container_id=$(docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q caddy)
        if [[ -n "$container_id" ]]; then
            echo "  Caddy管理API: http://localhost:2019"
            echo ""
            log_info "Caddy证书信息:"
            docker exec "$container_id" curl -s http://localhost:2019/config/apps/tls/certificates 2>/dev/null | \
                python3 -m json.tool 2>/dev/null || echo "  无法获取证书信息"
        fi
    else
        log_warning "Caddy服务未运行"
    fi
}

# 强制更新证书
renew_certificates() {
    log_step "强制更新TLS证书"
    
    case "$TLS_MODE" in
        "selfsigned")
            log_info "重新生成自签名证书"
            # 删除旧证书强制重新生成
            rm -f "$SCRIPT_DIR/certs/server.crt" "$SCRIPT_DIR/certs/server.key"
            generate_ip_self_signed_cert
            ;;
            
        "auto"|"")
            log_info "强制更新Let's Encrypt证书"
            if docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q caddy &>/dev/null; then
                log_info "通过Caddy API触发证书更新"
                local container_id
                container_id=$(docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q caddy)
                docker exec "$container_id" curl -X POST http://localhost:2019/load \
                    -H "Content-Type: application/json" \
                    -d '{"apps":{"tls":{"automation":{"policies":[{"management":{"module":"acme"},"subjects":["'$DOMAIN'"]}]}}}}'
                log_success "证书更新请求已发送"
            else
                log_error "Caddy服务未运行，无法更新证书"
                return 1
            fi
            ;;
            
    esac
    
    # 重启Caddy服务以加载新证书
    log_info "重启Caddy服务以加载新证书"
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" restart caddy
    
    log_success "证书更新完成"
}


# 重新生成安全密钥（谨慎操作）
regenerate_security_secrets() {
    log_step "重新生成安全密钥"
    
    echo ""
    log_warning "⚠️  重要提醒：此操作将重新生成JWT_SECRET和API_KEY_SECRET"
    echo ""
    echo "这将导致："
    echo "• 所有用户的JWT token失效，需要重新登录"
    echo "• 所有API密钥需要重新验证"
    echo "• 当前进行的API调用可能中断"
    echo ""
    
    read -p "确定要继续吗？(输入 'YES' 确认): " confirm
    
    if [[ "$confirm" != "YES" ]]; then
        log_info "操作已取消"
        return 0
    fi
    
    log_info "生成新的安全密钥..."
    
    # 生成新密钥
    local new_jwt_secret=$(generate_secure_key)
    local new_api_key_secret=$(generate_secure_key)
    
    # 更新环境文件
    if [[ -f "$ENV_FILE" ]]; then
        # 备份原文件
        cp "$ENV_FILE" "${ENV_FILE}.backup.$(date +%Y%m%d-%H%M%S)"
        log_info "已备份原环境文件"
        
        # 更新密钥
        sed -i "s/^JWT_SECRET=.*/JWT_SECRET=${new_jwt_secret}/" "$ENV_FILE"
        sed -i "s/^API_KEY_SECRET=.*/API_KEY_SECRET=${new_api_key_secret}/" "$ENV_FILE"
        
        log_success "安全密钥已更新"
        
        # 重启服务
        log_info "重启服务以应用新密钥"
        docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" restart proxy
        
        log_success "密钥重新生成完成，所有用户需要重新登录"
    else
        log_error "未找到环境配置文件: $ENV_FILE"
        return 1
    fi
}

# 切换TLS模式
switch_tls_mode() {
    local new_mode="$1"
    
    if [[ -z "$new_mode" ]]; then
        log_error "请指定TLS模式: auto, selfsigned"
        return 1
    fi
    
    case "$new_mode" in
        "auto"|"selfsigned")
            log_step "切换TLS模式: $TLS_MODE -> $new_mode"
            
            # 更新环境变量
            TLS_MODE="$new_mode"
            
            # 更新环境文件
            if grep -q "^TLS_MODE=" "$ENV_FILE" 2>/dev/null; then
                sed -i "s/^TLS_MODE=.*/TLS_MODE=$new_mode/" "$ENV_FILE"
            else
                echo "TLS_MODE=$new_mode" >> "$ENV_FILE"
            fi
            
            # 重新配置Caddy
            setup_caddy_tls
            
            # 重启服务
            log_info "重启服务以应用新的TLS配置"
            docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" restart caddy
            
            log_success "TLS模式已切换到: $new_mode"
            ;;
        *)
            log_error "不支持的TLS模式: $new_mode"
            log_info "支持的模式: auto, selfsigned"
            return 1
            ;;
    esac
}

# 创建必要的目录和文件
prepare_environment() {
    log_step "准备部署环境"
    
    # 创建必要的目录
    mkdir -p "$SCRIPT_DIR/certs"
    mkdir -p "$SCRIPT_DIR/config"
    mkdir -p "$SCRIPT_DIR/logs/caddy"
    
    # 设置配置文件
    CONFIG_SOURCE="config.prod.toml"
    log_info "使用生产环境配置: $CONFIG_SOURCE"
    
    # 检查配置文件是否存在
    if [ ! -f "$SCRIPT_DIR/config/$CONFIG_SOURCE" ]; then
        log_warning "配置文件 $CONFIG_SOURCE 不存在"
    fi
    
    # 第1步：交互式收集用户输入
    interactive_tls_setup
    
    # 第2步：在生成新环境文件之前，先保存现有的安全密钥 - 关键修复点
    log_info "处理安全密钥配置"
    
    # 检查是否存在旧的环境文件以保持密钥持久化
    local existing_jwt_secret
    local existing_api_key_secret
    
    if [[ -f "$ENV_FILE" ]]; then
        existing_jwt_secret=$(extract_env_value "JWT_SECRET" "$ENV_FILE")
        existing_api_key_secret=$(extract_env_value "API_KEY_SECRET" "$ENV_FILE")
        log_info "发现现有环境文件，将保持密钥持久化"
    fi
    
    # 如果没有现有密钥，生成新的安全密钥
    if [[ -z "$existing_jwt_secret" ]]; then
        existing_jwt_secret=$(generate_secure_key)
        log_success "生成新的JWT_SECRET"
    else
        log_info "保持现有JWT_SECRET不变"
    fi
    
    if [[ -z "$existing_api_key_secret" ]]; then
        existing_api_key_secret=$(generate_secure_key)
        log_success "生成新的API_KEY_SECRET"
    else
        log_info "保持现有API_KEY_SECRET不变"
    fi

    # 第3步：根据用户输入生成环境文件（现在安全密钥已经准备好）
    log_info "根据用户配置生成环境文件: $ENV_FILE"
    cat > "$ENV_FILE" << EOF
# AI代理平台环境配置 - 根据用户输入自动生成

# ================================
# 基础配置
# ================================
COMPOSE_PROJECT_NAME=api-proxy
CONFIG_FILE=config.prod.toml

# ================================
# TLS证书配置 (用户交互式选择结果)
# ================================
TLS_MODE=${TLS_MODE}
EOF

    # 根据TLS模式添加相应的用户配置
    if [[ "$TLS_MODE" == "selfsigned" ]]; then
        log_info "添加自签名证书配置到环境文件"
        cat >> "$ENV_FILE" << EOF
LOCAL_IP=${LOCAL_IP}
EOF
    else
        log_info "添加域名证书配置到环境文件"
        cat >> "$ENV_FILE" << EOF
DOMAIN=${DOMAIN}
CERT_EMAIL=${CERT_EMAIL}
EOF
    fi

    # 添加通用配置
    cat >> "$ENV_FILE" << EOF

# ================================
# 日志配置
# ================================
RUST_LOG=info
RUST_BACKTRACE=1

# ================================
# 数据库配置
# ================================
DATABASE_URL=sqlite:///app/data/api-proxy.db

# ================================
# 安全配置 (持久化密钥，避免用户会话丢失)
# ================================
JWT_SECRET=${existing_jwt_secret}
API_KEY_SECRET=${existing_api_key_secret}

# ================================
# 前端配置
# ================================
VITE_API_BASE_URL=/api
VITE_WS_URL=/ws
VITE_APP_VERSION=1.0.0

# ================================
# 监控配置
# ================================
ENABLE_METRICS=true
METRICS_PORT=9091

# ================================
# 版本标识
# ================================
VERSION=1.0.0
BUILD_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
EOF

    log_success "环境文件生成完成: $ENV_FILE"
    
    # 第3步：验证生成的环境文件
    log_info "验证环境文件内容:"
    if [[ "$TLS_MODE" == "selfsigned" ]]; then
        log_info "  模式: 自签名证书"
        log_info "  IP地址: $LOCAL_IP"
    else
        log_info "  模式: 域名证书"
        log_info "  域名: $DOMAIN"
        log_info "  邮箱: $CERT_EMAIL"
    fi
    
    # 第4步：生成Caddy配置
    setup_caddy_tls
    
    log_success "环境配置完成"
}

# 启动服务
start_services() {
    log_step "启动统一服务"
    
    cd "$SCRIPT_DIR"
    
    # 加载环境变量
    if [ -f "$ENV_FILE" ]; then
        set -a
        source "$ENV_FILE"
        set +a
        log_info "启动服务栈：统一代理服务 + Caddy反向代理"
        docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d
    else
        log_error "未找到环境配置文件: $ENV_FILE"
        log_info "请先运行 ./deploy.sh install"
        exit 1
    fi
    
    log_success "服务启动完成"
}


# 停止服务
stop_services() {
    log_step "停止服务"
    
    cd "$SCRIPT_DIR"
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" down
    
    log_success "服务已停止"
}

# 重启服务
restart_services() {
    log_step "重启服务"
    
    stop_services
    start_services
    
    log_success "服务重启完成"
}

# 查看服务状态
show_status() {
    log_step "服务状态"
    
    cd "$SCRIPT_DIR"
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps
    
    echo ""
    log_info "服务健康状态:"
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" exec proxy curl -f http://localhost:9090/api/health 2>/dev/null && log_success "统一代理服务正常" || log_warning "统一代理服务异常"
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" exec caddy wget --quiet --tries=1 --spider http://localhost:2019/config/ 2>/dev/null && log_success "Caddy代理正常" || log_warning "Caddy代理异常"
}

# 查看日志
show_logs() {
    local service="$1"
    local lines="${2:-100}"
    
    cd "$SCRIPT_DIR"
    
    if [ -n "$service" ]; then
        log_step "查看 $service 服务日志 (最近 $lines 行)"
        docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" logs --tail="$lines" -f "$service"
    else
        log_step "查看所有服务日志 (最近 $lines 行)"
        docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" logs --tail="$lines" -f
    fi
}

# 清理资源
cleanup() {
    log_step "清理Docker资源"
    
    cd "$SCRIPT_DIR"
    
    # 停止并删除容器
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" down --volumes --remove-orphans
    
    # 删除镜像（可选）
    if [ "$1" = "--images" ]; then
        docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" down --rmi all
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
            docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" exec proxy cp /app/data/api-proxy.db "/tmp/$(basename "$backup_file")"
            docker cp "$(docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q proxy):/tmp/$(basename "$backup_file")" "$backup_file"
            log_success "数据库已备份到: $backup_file"
            ;;
        "restore")
            local backup_file="$2"
            if [ -z "$backup_file" ] || [ ! -f "$backup_file" ]; then
                log_error "请指定有效的备份文件"
                exit 1
            fi
            log_step "恢复数据库"
            docker cp "$backup_file" "$(docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" ps -q proxy):/app/data/api-proxy.db"
            docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" restart proxy
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
    
    if [[ "$TLS_MODE" == "selfsigned" ]]; then
        echo -e "${BLUE}🌍 自签名证书模式 (测试环境):${NC}"
        echo -e "  📱 管理面板:  ${GREEN}https://$LOCAL_IP${NC} ${YELLOW}← 主要访问入口 (443端口)${NC}"
        echo -e "  🔧 管理面板:  ${GREEN}https://$LOCAL_IP/dashboard${NC}"
        echo -e "  🤖 API接口:   ${GREEN}https://$LOCAL_IP/api${NC}"
        echo -e "  🚀 AI代理服务: ${GREEN}https://$LOCAL_IP:8443${NC} ${YELLOW}← AI代理专用端口${NC}"
        echo -e "  🏠 本地访问:  ${GREEN}https://localhost${NC}"
        echo ""
        echo -e "${YELLOW}⚠️  注意事项:${NC}"
        echo "  • 浏览器会提示证书不受信任，点击"高级"→"继续访问"即可"
        echo "  • 自签名证书仅供测试使用，生产环境请使用域名证书"
    else
        echo -e "${BLUE}🌍 域名证书模式 (生产环境):${NC}"
        echo -e "  📱 主域名:    ${GREEN}https://$DOMAIN${NC} ${YELLOW}← 主要访问入口${NC}"
        echo -e "  🔧 管理面板:  ${GREEN}https://$DOMAIN/dashboard${NC}"
        echo -e "  🤖 API接口:   ${GREEN}https://$DOMAIN/api${NC}"
        echo -e "  🚀 AI代理服务: ${GREEN}https://$DOMAIN:8443${NC}"
        echo ""
        echo -e "${YELLOW}📌 证书信息:${NC}"
        echo "  • 域名: $DOMAIN"
        echo "  • 邮箱: $CERT_EMAIL"
        echo "  • 自动续期: Let's Encrypt"
    fi
    
    echo ""
    echo -e "${YELLOW}📌 服务架构特点:${NC}"
    echo "  • 统一后端服务：9090端口（前端静态文件 + API）"
    echo "  • AI代理服务：8080端口（专用AI代理转发）"
    echo "  • Caddy反向代理：443端口(管理) + 8443端口(AI代理)"
    echo "  • 自动HTTPS和SSL证书管理"
    echo ""
    echo -e "${BLUE}🔧 直接访问（调试用）:${NC}"
    echo "  • 统一服务: http://localhost:9090"
    echo "  • API健康检查: http://localhost:9090/api/health"
    echo ""
    echo -e "${BLUE}⚙️ 管理命令:${NC}"
    echo -e "  📊 查看状态: ${GREEN}./deploy.sh status${NC}"
    echo -e "  📋 查看日志: ${GREEN}./deploy.sh logs [proxy|caddy]${NC}"
    echo -e "  ⏹️  停止服务: ${GREEN}./deploy.sh stop${NC}"
    echo -e "  🔄 重启服务: ${GREEN}./deploy.sh restart${NC}"
    echo ""
    echo -e "${GREEN}==================================================${NC}"
}

# 显示帮助信息
show_help() {
    cat << EOF
AI代理平台统一部署脚本

用法: $0 <命令> [选项]

核心命令:
  install              安装和启动统一代理服务
  start                启动所有服务
  stop                 停止所有服务
  restart              重启服务

管理命令:
  status               查看服务运行状态
  logs [service]       查看服务日志 (proxy|caddy)
  cleanup [--images]   清理Docker资源
  backup               备份数据库
  restore <file>       恢复数据库
  help                 显示此帮助信息

TLS证书管理:
  cert-status          查看当前证书状态
  cert-renew           手动更新证书
  cert-selfsign        生成自签名证书（开发用）
  cert-mode <mode>     切换证书模式 (auto|selfsigned)

安全密钥管理:
  regenerate-secrets   重新生成JWT和API密钥（⚠️ 谨慎使用）

服务架构:
  统一代理服务：
    • 前后端合并部署，9090端口提供完整服务
    • 包含前端静态文件和后端API
    • 8080端口重定向到根路径
    • 支持健康检查和监控

  Caddy反向代理：
    • 自动HTTPS和SSL证书管理
    • 域名 example.com 路由到统一服务
    • 443端口：主域名访问
    • 8443端口：备用访问端口


环境变量:
  DOMAIN=<domain>      指定主域名（默认：example.com）
  LOCAL_IP=<ip>        指定本机IP地址（自动检测或手动设置，默认：自动检测）
  TLS_MODE=<mode>      TLS证书模式（auto|selfsigned，默认：auto）
  CERT_EMAIL=<email>   Let's Encrypt证书申请邮箱

使用示例:
  ./deploy.sh install              # 完整安装部署
  ./deploy.sh logs proxy           # 查看统一服务日志
  ./deploy.sh logs caddy           # 查看Caddy代理日志
  ./deploy.sh restart              # 重启所有服务
  ./deploy.sh backup               # 备份数据库

TLS证书管理示例:
  ./deploy.sh cert-status          # 查看证书状态
  ./deploy.sh cert-mode selfsigned # 切换到自签名证书（开发环境）
  ./deploy.sh cert-mode auto       # 切换到自动证书（生产环境）
  ./deploy.sh cert-renew           # 手动更新证书
  
智能安装特性:
  ./deploy.sh install              # 智能检测内网+外网IP，自动生成证书
  # 无需手动设置环境变量，脚本会自动检测和配置所有IP地址

访问地址:
  • https://[本机IP]               # IP地址访问（自签名证书模式，需要设置LOCAL_IP环境变量）
  • https://localhost              # 本地访问
  • https://localhost:8443         # 备用端口
  • http://[本机IP]                # HTTP访问（开发模式）
  • 域名模式: https://example.com # 域名访问（auto证书模式）

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
            start_services
            show_access_info
            ;;
        "start")
            check_docker
            start_services
            ;;
        "stop")
            stop_services
            ;;
        "restart")
            check_docker
            restart_services
            ;;
        "status")
            show_status
            ;;
        "logs")
            show_logs "$2" "$3"
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
            show_access_info
            ;;
        "cert-status")
            show_cert_status
            ;;
        "cert-renew")
            renew_certificates
            ;;
        "cert-selfsign")
            TLS_MODE="selfsigned"
            ensure_local_ip
            generate_ip_self_signed_cert
            ;;
        "cert-mode")
            switch_tls_mode "$2"
            ;;
        "regenerate-secrets")
            regenerate_security_secrets
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