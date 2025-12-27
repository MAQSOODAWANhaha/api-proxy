#!/usr/bin/env bash
#
# AI Proxy Platform - Standalone 部署脚本
# 目标：用户只需要本脚本 + docker-compose.yaml 即可部署（无需 git clone 项目源码）。
#
# 证书方案：
# - selfsigned：使用 Caddy `tls internal`（最佳实践：不依赖宿主机 openssl，不需要管理证书文件）
# - auto：域名自动证书（Let's Encrypt），仍由 Caddy 负责申请与续期
#
# 运行时环境变量（最小化）：
# - JWT_SECRET（必须，>=32位）

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.yaml"
ENV_FILE="${SCRIPT_DIR}/.env"
CADDYFILE="${SCRIPT_DIR}/Caddyfile"

IMAGE_PROXY_DEFAULT="gghtrt520/api-proxy:latest"
CONTAINER_NAME_DOCKER_RUN="api-proxy"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC} $*"; }

usage() {
  cat <<'EOF'
用法:
  ./deploy.sh install [--tls selfsigned|auto] [--domain example.com] [--email you@example.com]
  ./deploy.sh up | down | restart | status | logs [proxy|caddy] | update
  ./deploy.sh print-docker-run [--image <image>]

说明:
  - install: 生成/复用 JWT_SECRET，生成 Caddyfile，然后启动 docker compose
  - up/down/...: 运维命令（基于同目录 docker-compose.yaml）
  - print-docker-run: 输出“单条 docker run”HTTP 部署命令（不含 Caddy/TLS）

TLS 模式（默认 selfsigned）:
  - selfsigned: Caddy `tls internal` 自签（访问 https://<IP>/ 与 https://<IP>:8443/ 会提示不受信任）
  - auto: 域名自动证书（需要公网可达的 80/443 端口与正确 DNS 解析）

最低依赖:
  - Docker + docker compose 插件
  - 环境变量 JWT_SECRET（>=32位；install 会自动生成 .env）
EOF
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    log_error "缺少命令: $cmd"
    exit 1
  fi
}

check_docker() {
  require_cmd docker
  if ! docker compose version >/dev/null 2>&1; then
    log_error "未检测到 docker compose 插件，请安装 docker-compose-plugin"
    exit 1
  fi
}

generate_secret() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -base64 48 | tr -d '\n'
    return 0
  fi
  head -c 48 /dev/urandom | base64 | tr -d '\n'
}

ensure_jwt_secret() {
  # 优先：当前 shell 已提供 JWT_SECRET
  if [[ -n "${JWT_SECRET:-}" ]]; then
    if [[ ${#JWT_SECRET} -lt 32 ]]; then
      log_error "JWT_SECRET 长度不足（需要 >=32），当前长度=${#JWT_SECRET}"
      exit 1
    fi
    return 0
  fi

  # 其次：复用已有 .env
  if [[ -f "$ENV_FILE" ]]; then
    # shellcheck disable=SC1090
    set -a
    source "$ENV_FILE"
    set +a
    if [[ -n "${JWT_SECRET:-}" ]]; then
      if [[ ${#JWT_SECRET} -lt 32 ]]; then
        log_error "已有 .env 中 JWT_SECRET 长度不足（需要 >=32），当前长度=${#JWT_SECRET}"
        exit 1
      fi
      log_info "复用已有 .env 中的 JWT_SECRET"
      return 0
    fi
  fi

  log_info "生成新的 JWT_SECRET 并写入 .env"
  local secret
  secret="$(generate_secret)"
  if [[ ${#secret} -lt 32 ]]; then
    log_error "生成 JWT_SECRET 失败（长度不足）"
    exit 1
  fi
  cat >"$ENV_FILE" <<EOF
# 自动生成：只用于 docker compose 变量注入（最小化依赖）
JWT_SECRET=${secret}
EOF

  # shellcheck disable=SC1090
  set -a
  source "$ENV_FILE"
  set +a
}

write_caddyfile_selfsigned() {
  cat >"$CADDYFILE" <<'EOF'
{
  # 保持默认 admin 仅容器内可访问（不对外暴露）
  auto_https disable_redirects
}

# 443 -> 管理服务(9090)
:443 {
  tls internal
  reverse_proxy proxy:9090
}

# 8443 -> 代理服务(8080)
:8443 {
  tls internal
  reverse_proxy proxy:8080
}

# HTTP -> HTTPS（同时满足域名/自签两种模式下的使用习惯）
:80 {
  redir https://{host}{uri}
}
EOF
}

write_caddyfile_auto() {
  local domain="$1"
  local email="$2"

  cat >"$CADDYFILE" <<EOF
{
  email ${email}
}

${domain} {
  reverse_proxy proxy:9090
}

${domain}:8443 {
  reverse_proxy proxy:8080
}

:80 {
  redir https://{host}{uri}
}
EOF
}

compose() {
  (cd "$SCRIPT_DIR" && docker compose -f "$COMPOSE_FILE" "$@")
}

install() {
  local tls_mode="selfsigned"
  local domain=""
  local email=""

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --tls)
        tls_mode="${2:-}"
        shift 2
        ;;
      --domain)
        domain="${2:-}"
        shift 2
        ;;
      --email)
        email="${2:-}"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        log_error "未知参数: $1"
        usage
        exit 1
        ;;
    esac
  done

  check_docker
  ensure_jwt_secret

  if [[ ! -f "$COMPOSE_FILE" ]]; then
    log_error "未找到 docker-compose.yaml：$COMPOSE_FILE"
    exit 1
  fi

  case "$tls_mode" in
    selfsigned)
      log_info "TLS 模式：selfsigned（Caddy tls internal）"
      write_caddyfile_selfsigned
      ;;
    auto)
      if [[ -z "$domain" ]]; then
        read -r -p "请输入域名（例如 api.example.com）: " domain
      fi
      if [[ -z "$domain" ]]; then
        log_error "auto 模式必须提供 --domain 或交互输入域名"
        exit 1
      fi
      if [[ -z "$email" ]]; then
        read -r -p "请输入邮箱（用于 Let's Encrypt 通知，可回车用默认）: " email
      fi
      email="${email:-admin@${domain}}"
      log_info "TLS 模式：auto（域名自动证书）domain=${domain}, email=${email}"
      write_caddyfile_auto "$domain" "$email"
      ;;
    *)
      log_error "不支持的 --tls: $tls_mode（仅支持 selfsigned|auto）"
      exit 1
      ;;
  esac

  log_info "启动服务..."
  compose up -d

  log_success "部署完成"
  echo ""
  echo -e "${BLUE}访问方式（A 方案：443->9090，8443->8080）${NC}"
  echo "  - 管理/前端: https://<IP或域名>/dashboard"
  echo "  - 管理 API:  https://<IP或域名>/api"
  echo "  - 代理服务:  https://<IP或域名>:8443"
  echo ""
  if [[ "$tls_mode" == "selfsigned" ]]; then
    echo -e "${YELLOW}提示：selfsigned 使用内部CA，自签证书浏览器会提示不受信任（测试/内网场景推荐）。${NC}"
  else
    echo -e "${YELLOW}提示：auto 模式需要 80/443 端口公网可达，并确保域名 DNS 指向该机器。${NC}"
  fi
}

print_docker_run() {
  local image="$IMAGE_PROXY_DEFAULT"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --image)
        image="${2:-}"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        log_error "未知参数: $1"
        usage
        exit 1
        ;;
    esac
  done

  cat <<EOF
docker run -d --name ${CONTAINER_NAME_DOCKER_RUN} --restart unless-stopped \\
  -e JWT_SECRET="<替换为你的强随机字符串(>=32位)>" \\
  -p 9090:9090 -p 8080:8080 \\
  -v api_proxy_data:/app/data \\
  -v api_proxy_logs:/app/logs \\
  ${image}
EOF
}

main() {
  local cmd="${1:-help}"
  shift || true

  case "$cmd" in
    install) install "$@" ;;
    up) check_docker; ensure_jwt_secret; compose up -d ;;
    down) check_docker; compose down ;;
    restart) check_docker; ensure_jwt_secret; compose restart ;;
    status) check_docker; compose ps ;;
    logs)
      check_docker
      local svc="${1:-}"
      if [[ -n "$svc" ]]; then
        compose logs -f --tail=200 "$svc"
      else
        compose logs -f --tail=200
      fi
      ;;
    update)
      check_docker
      log_info "拉取最新镜像..."
      compose pull
      log_info "重启服务..."
      ensure_jwt_secret
      compose up -d
      log_success "更新完成"
      ;;
    print-docker-run) print_docker_run "$@" ;;
    help|-h|--help) usage ;;
    *)
      log_error "未知命令: $cmd"
      usage
      exit 1
      ;;
  esac
}

main "$@"

