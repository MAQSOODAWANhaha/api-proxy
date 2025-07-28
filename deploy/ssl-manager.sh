#!/bin/bash

# AI代理平台SSL证书管理脚本
# 支持Let's Encrypt证书的自动申请和续期
# 作者: AI代理平台开发团队
# 域名: domain.com

set -euo pipefail

# ===========================================
# 配置变量
# ===========================================

DOMAIN="domain.com"
EMAIL="admin@zhanglei.vip"  # 请修改为实际邮箱
ACME_HOME="/opt/acme.sh"
CERT_DIR="/opt/ssl/live/$DOMAIN"
NGINX_CERT_DIR="/var/lib/docker/volumes/api-proxy_ssl_certs/_data/live/$DOMAIN"
WEBROOT_PATH="/var/lib/docker/volumes/api-proxy_webroot/_data"
COMPOSE_FILE="/workspaces/api-proxy/deploy/docker-compose.yaml"
COMPOSE_SSL_FILE="/workspaces/api-proxy/deploy/docker-compose.ssl.yaml"

# 日志配置
LOG_FILE="/var/log/ssl-manager.log"
exec 1> >(tee -a "$LOG_FILE")
exec 2> >(tee -a "$LOG_FILE" >&2)

# ===========================================
# 函数定义
# ===========================================

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    log "ERROR: $*" >&2
    exit 1
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        error "此脚本需要root权限运行"
    fi
}

install_dependencies() {
    log "安装依赖包..."
    
    # 检测系统类型
    if command -v apt-get &> /dev/null; then
        apt-get update
        apt-get install -y curl socat cron docker-compose
    elif command -v yum &> /dev/null; then
        yum update -y
        yum install -y curl socat cronie docker-compose
        systemctl enable crond
        systemctl start crond
    else
        error "不支持的系统类型"
    fi
}

install_acme() {
    if [[ ! -d "$ACME_HOME" ]]; then
        log "安装acme.sh..."
        curl https://get.acme.sh | sh -s email="$EMAIL" --home "$ACME_HOME"
        
        # 创建符号链接到PATH
        ln -sf "$ACME_HOME/acme.sh" /usr/local/bin/acme.sh
        
        # 设置默认CA为Let's Encrypt
        "$ACME_HOME/acme.sh" --set-default-ca --server letsencrypt
    else
        log "acme.sh已安装，更新到最新版本..."
        "$ACME_HOME/acme.sh" --upgrade
    fi
}

prepare_directories() {
    log "准备证书目录..."
    
    # 创建本地证书目录
    mkdir -p "$CERT_DIR"
    mkdir -p "$(dirname "$LOG_FILE")"
    
    # 创建Docker卷目录
    mkdir -p "$NGINX_CERT_DIR"
    mkdir -p "$WEBROOT_PATH/.well-known/acme-challenge"
    
    # 设置权限
    chmod 755 "$WEBROOT_PATH"
    chmod 755 "$WEBROOT_PATH/.well-known"
    chmod 755 "$WEBROOT_PATH/.well-known/acme-challenge"
}

create_ssl_compose() {
    log "创建SSL Docker Compose配置..."
    
    cat > "$COMPOSE_SSL_FILE" << 'EOF'
# SSL扩展配置 - 与主配置合并使用
# 使用方法: docker-compose -f docker-compose.yaml -f docker-compose.ssl.yaml up -d

services:
  # SSL反向代理
  proxy-ssl:
    image: docker.m.daocloud.io/nginx:alpine
    container_name: api-proxy-ssl-gateway
    restart: unless-stopped
    ports:
      - "80:80"    # HTTP (重定向和ACME验证)
      - "443:443"  # HTTPS
    volumes:
      - ./nginx-ssl.conf:/etc/nginx/nginx.conf:ro
      - ssl_certs:/etc/nginx/ssl:ro
      - webroot:/var/www/certbot:ro
    networks:
      - api-proxy-network
    depends_on:
      - frontend
      - backend
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Certbot证书管理（可选，用于调试）
  certbot:
    image: docker.m.daocloud.io/certbot/certbot:latest
    container_name: api-proxy-certbot
    volumes:
      - ssl_certs:/etc/letsencrypt
      - webroot:/var/www/certbot
    profiles:
      - certbot  # 仅在需要时启用
    command: certonly --webroot --webroot-path=/var/www/certbot --email admin@zhanglei.vip --agree-tos --no-eff-email --staging -d domain.com

# 新增卷定义
volumes:
  ssl_certs:
    driver: local
  webroot:
    driver: local

networks:
  api-proxy-network:
    external: true
EOF

    log "SSL Docker Compose配置已创建: $COMPOSE_SSL_FILE"
}

request_certificate() {
    log "申请SSL证书..."
    
    # 确保webroot存在
    prepare_directories
    
    # 使用webroot模式申请证书
    "$ACME_HOME/acme.sh" --issue \
        -d "$DOMAIN" \
        -w "$WEBROOT_PATH" \
        --keylength ec-256 \
        --server letsencrypt \
        || {
            log "证书申请失败，尝试使用DNS模式（需要手动配置DNS记录）"
            log "请在DNS中添加以下TXT记录，然后重新运行脚本："
            "$ACME_HOME/acme.sh" --issue --dns -d "$DOMAIN" --yes-I-know-dns-manual-mode-enough-go-ahead-please
            return 1
        }
}

install_certificate() {
    log "安装证书到系统..."
    
    # 安装证书到本地目录
    "$ACME_HOME/acme.sh" --install-cert -d "$DOMAIN" --ecc \
        --cert-file "$CERT_DIR/cert.pem" \
        --key-file "$CERT_DIR/privkey.pem" \
        --fullchain-file "$CERT_DIR/fullchain.pem" \
        --ca-file "$CERT_DIR/chain.pem" \
        --reloadcmd "systemctl reload nginx" || true
    
    # 复制到Docker卷目录
    cp -r "$CERT_DIR" "$NGINX_CERT_DIR/../"
    
    # 设置权限
    chmod 644 "$CERT_DIR"/*.pem
    chmod 644 "$NGINX_CERT_DIR"/*.pem || true
    
    log "证书安装完成"
}

setup_auto_renewal() {
    log "设置自动续期..."
    
    # 创建续期脚本
    cat > /usr/local/bin/ssl-renew.sh << 'RENEW_SCRIPT'
#!/bin/bash

DOMAIN="domain.com"
ACME_HOME="/opt/acme.sh"
CERT_DIR="/opt/ssl/live/$DOMAIN"
NGINX_CERT_DIR="/var/lib/docker/volumes/api-proxy_ssl_certs/_data/live/$DOMAIN"
LOG_FILE="/var/log/ssl-renewal.log"

# 续期证书
"$ACME_HOME/acme.sh" --renew -d "$DOMAIN" --ecc --force >> "$LOG_FILE" 2>&1

# 安装新证书
"$ACME_HOME/acme.sh" --install-cert -d "$DOMAIN" --ecc \
    --cert-file "$CERT_DIR/cert.pem" \
    --key-file "$CERT_DIR/privkey.pem" \
    --fullchain-file "$CERT_DIR/fullchain.pem" \
    --ca-file "$CERT_DIR/chain.pem" >> "$LOG_FILE" 2>&1

# 复制到Docker卷
cp -r "$CERT_DIR" "$NGINX_CERT_DIR/../" >> "$LOG_FILE" 2>&1

# 重载nginx配置
docker exec api-proxy-ssl-gateway nginx -s reload >> "$LOG_FILE" 2>&1

echo "[$(date)] SSL证书续期完成" >> "$LOG_FILE"
RENEW_SCRIPT

    chmod +x /usr/local/bin/ssl-renew.sh
    
    # 添加cron任务（每日检查，每月1号和15号强制续期）
    (crontab -l 2>/dev/null; echo "0 2 * * * /usr/local/bin/ssl-renew.sh") | crontab -
    (crontab -l 2>/dev/null; echo "0 3 1,15 * * /usr/local/bin/ssl-renew.sh --force") | crontab -
    
    log "自动续期设置完成，每日2点检查，每月1号和15号强制续期"
}

reload_services() {
    log "重新加载服务..."
    
    # 停止当前的proxy服务（如果在运行）
    docker-compose -f "$COMPOSE_FILE" stop proxy 2>/dev/null || true
    
    # 启动SSL代理服务
    docker-compose -f "$COMPOSE_FILE" -f "$COMPOSE_SSL_FILE" up -d proxy-ssl
    
    log "SSL服务启动完成"
}

show_status() {
    log "显示证书状态..."
    
    if [[ -f "$CERT_DIR/fullchain.pem" ]]; then
        echo "证书信息:"
        openssl x509 -in "$CERT_DIR/fullchain.pem" -text -noout | grep -E "(Subject:|Not Before|Not After)"
        echo
        echo "证书到期时间:"
        openssl x509 -in "$CERT_DIR/fullchain.pem" -noout -dates
        echo
    else
        echo "证书文件不存在: $CERT_DIR/fullchain.pem"
    fi
    
    echo "服务状态:"
    docker-compose -f "$COMPOSE_FILE" -f "$COMPOSE_SSL_FILE" ps proxy-ssl 2>/dev/null || echo "SSL代理服务未运行"
}

# ===========================================
# 主程序
# ===========================================

case "${1:-}" in
    "init")
        log "初始化SSL环境..."
        check_root
        install_dependencies
        install_acme
        prepare_directories
        create_ssl_compose
        log "SSL环境初始化完成"
        echo "下一步运行: $0 request"
        ;;
    
    "request")
        log "申请SSL证书..."
        request_certificate
        install_certificate
        setup_auto_renewal
        log "证书申请完成"
        echo "下一步运行: $0 deploy"
        ;;
    
    "deploy")
        log "部署SSL服务..."
        reload_services
        show_status
        log "SSL部署完成"
        echo
        echo "现在可以通过 https://$DOMAIN 访问服务"
        echo "HTTP请求会自动重定向到HTTPS"
        ;;
    
    "renew")
        log "手动续期证书..."
        /usr/local/bin/ssl-renew.sh
        show_status
        ;;
    
    "status")
        show_status
        ;;
    
    "remove")
        log "移除SSL配置..."
        docker-compose -f "$COMPOSE_FILE" -f "$COMPOSE_SSL_FILE" down proxy-ssl 2>/dev/null || true
        crontab -l | grep -v ssl-renew.sh | crontab - 2>/dev/null || true
        log "SSL配置已移除"
        ;;
    
    *)
        echo "AI代理平台SSL证书管理工具"
        echo "用法: $0 {init|request|deploy|renew|status|remove}"
        echo
        echo "命令说明:"
        echo "  init    - 初始化SSL环境和依赖"
        echo "  request - 申请SSL证书"
        echo "  deploy  - 部署SSL服务"
        echo "  renew   - 手动续期证书"
        echo "  status  - 查看证书和服务状态"
        echo "  remove  - 移除SSL配置"
        echo
        echo "完整部署流程:"
        echo "  1. $0 init     # 初始化环境"
        echo "  2. $0 request  # 申请证书"
        echo "  3. $0 deploy   # 部署服务"
        echo
        echo "日常维护:"
        echo "  $0 status      # 查看状态"
        echo "  $0 renew       # 手动续期"
        exit 1
        ;;
esac