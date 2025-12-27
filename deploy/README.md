# Standalone 部署（无需克隆源码）

本目录提供两种“最省事”的部署方式：

1) **单条 `docker run`（纯 HTTP）**：只依赖 `JWT_SECRET`，直接暴露 `8080/9090` 两个端口。  
2) **`deploy.sh` + `docker-compose.yaml`（推荐）**：只依赖 `JWT_SECRET`，由 **Caddy** 负责 HTTPS（自签/域名自动证书），对外按 **两端口方案**暴露：`443 -> 9090`、`8443 -> 8080`。

## 方式1：单条 docker run（HTTP，最简单）

只要 1 条命令即可启动（把 `<...>` 替换成你的值）：

```bash
docker run -d --name api-proxy --restart unless-stopped \
  -e JWT_SECRET="<强随机字符串(>=32位)>" \
  -p 9090:9090 -p 8080:8080 \
  -v api_proxy_data:/app/data \
  -v api_proxy_logs:/app/logs \
  gghtrt520/api-proxy:latest
```

访问：
- 管理/前端：`http://<IP>:9090/dashboard`
- 管理 API：`http://<IP>:9090/api`
- 代理服务：`http://<IP>:8080`

## 方式2：deploy.sh + docker-compose（HTTPS，自签/域名自动证书）

### 1) 准备文件

在任意空目录中仅放入两份文件（从本仓库复制即可）：
- `deploy.sh`
- `docker-compose.yaml`

### 2) 一键安装（默认自签）

```bash
chmod +x deploy.sh
./deploy.sh install
```

### 3) 域名自动证书（生产环境）

```bash
./deploy.sh install --tls auto --domain api.example.com --email you@example.com
```

### 访问方式（A 方案：两端口）

- 管理/前端：`https://<域名或IP>/dashboard`（`443 -> 9090`）
- 管理 API：`https://<域名或IP>/api`
- 代理服务：`https://<域名或IP>:8443`（`8443 -> 8080`）

### 常用运维命令

```bash
./deploy.sh status
./deploy.sh logs proxy
./deploy.sh logs caddy
./deploy.sh restart
./deploy.sh update
./deploy.sh down
```

### 目录/文件说明（首次 install 后自动生成）

- `.env`：仅包含 `JWT_SECRET=...`（用于 docker compose 变量注入）
- `Caddyfile`：由脚本生成
- Docker volumes：
  - `proxy_data`：数据库等持久化数据
  - `proxy_logs`：日志（如容器内写入）
  - `caddy_data`/`caddy_config`：Caddy 证书/配置持久化

