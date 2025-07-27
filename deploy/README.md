# AI代理平台 - 容器化部署文档

本目录包含AI代理平台的完整容器化部署方案，支持开发和生产环境的一键部署。

## 📋 目录结构

```
deploy/
├── README.md                 # 部署说明文档
├── docker-compose.yaml       # Docker Compose配置文件
├── Dockerfile.backend        # 后端服务Dockerfile
├── Dockerfile.frontend       # 前端服务Dockerfile
├── nginx.conf                # 前端Nginx配置
├── nginx-gateway.conf        # 生产环境网关配置
├── deploy.sh                 # 一键部署脚本
└── .env                      # 环境变量配置（运行后生成）
```

## 🚀 快速开始

### 前置要求

- Docker Engine 20.0+
- Docker Compose 2.0+
- 至少2GB可用内存
- 至少5GB可用磁盘空间

### 一键安装

```bash
# 进入部署目录
cd deploy

# 给脚本添加执行权限
chmod +x deploy.sh

# 开发环境安装
./deploy.sh install

# 生产环境安装（包含网关）
./deploy.sh install-prod
```

### 访问应用

安装完成后，可通过以下地址访问：

- **前端管理界面**: http://localhost:3000
- **后端管理API**: http://localhost:9090/api
- **AI代理服务**: http://localhost:8080/v1
- **生产网关** (仅生产模式): http://localhost

## 🔧 详细配置

### 环境变量配置

首次运行会自动创建 `.env` 文件，包含以下主要配置：

```env
# 应用配置
COMPOSE_PROJECT_NAME=api-proxy

# 端口配置
FRONTEND_PORT=3000
BACKEND_API_PORT=9090
BACKEND_PROXY_PORT=8080
REDIS_PORT=6379

# 安全配置（生产环境请修改）
JWT_SECRET=your-jwt-secret
API_KEY_SECRET=your-api-key-secret

# 数据库配置
DATABASE_URL=sqlite:///app/data/api-proxy.db

# TLS配置
TLS_ENABLED=false
TLS_CERT_PATH=/app/certs/cert.pem
TLS_KEY_PATH=/app/certs/key.pem
```

### 服务组件

| 服务 | 容器名 | 端口 | 描述 |
|------|--------|------|------|
| backend | api-proxy-backend | 8080, 9090 | Rust后端服务 |
| frontend | api-proxy-frontend | 3000 | Vue.js前端界面 |
| redis | api-proxy-redis | 6379 | Redis缓存服务 |
| proxy | api-proxy-gateway | 80, 443 | Nginx网关(生产) |

### 数据持久化

以下数据会持久化保存：

- **backend_data**: 后端应用数据（数据库、配置等）
- **backend_logs**: 后端日志文件
- **redis_data**: Redis数据

## 📋 常用命令

### 服务管理

```bash
# 查看服务状态
./deploy.sh status

# 启动服务
./deploy.sh start [profile]

# 停止服务
./deploy.sh stop

# 重启服务
./deploy.sh restart [profile]

# 重新构建镜像
./deploy.sh build
```

### 日志查看

```bash
# 查看所有服务日志
./deploy.sh logs

# 查看特定服务日志
./deploy.sh logs backend
./deploy.sh logs frontend
./deploy.sh logs redis

# 查看指定行数的日志
./deploy.sh logs backend 50
```

### 数据库管理

```bash
# 备份数据库
./deploy.sh backup

# 恢复数据库
./deploy.sh restore /path/to/backup.db
```

### 资源清理

```bash
# 清理容器和数据卷
./deploy.sh cleanup

# 清理容器、数据卷和镜像
./deploy.sh cleanup --images
```

## 🌐 生产环境部署

### 1. 域名和SSL配置

1. 修改 `nginx-gateway.conf` 中的 `server_name`
2. 将SSL证书放置在 `ssl/` 目录中
3. 在 `.env` 中启用TLS配置：

```env
TLS_ENABLED=true
TLS_CERT_PATH=/etc/nginx/ssl/cert.pem
TLS_KEY_PATH=/etc/nginx/ssl/key.pem
```

4. 取消 `nginx-gateway.conf` 中HTTPS配置的注释

### 2. 安全加固

1. 修改默认密钥：

```bash
# 生成新的JWT密钥
JWT_SECRET=$(openssl rand -base64 32)

# 生成新的API密钥
API_KEY_SECRET=$(openssl rand -base64 32)
```

2. 配置防火墙规则：

```bash
# 仅允许必要端口
ufw allow 80/tcp    # HTTP
ufw allow 443/tcp   # HTTPS
ufw allow 22/tcp    # SSH
```

3. 定期更新镜像：

```bash
./deploy.sh cleanup --images
./deploy.sh build
./deploy.sh restart production
```

### 3. 监控和日志

1. 配置日志轮转：

```bash
# 创建logrotate配置
sudo tee /etc/logrotate.d/api-proxy << EOF
/var/lib/docker/containers/*/*-json.log {
    daily
    rotate 7
    compress
    missingok
    notifempty
    create 0644 root root
}
EOF
```

2. 监控服务状态：

```bash
# 添加到crontab
*/5 * * * * /path/to/deploy/deploy.sh status > /dev/null || /usr/bin/systemctl restart docker
```

## 🔍 故障排除

### 常见问题

1. **端口冲突**
   ```bash
   # 检查端口占用
   netstat -tlnp | grep :3000
   
   # 修改.env中的端口配置
   FRONTEND_PORT=3001
   ```

2. **内存不足**
   ```bash
   # 检查内存使用
   docker system df
   
   # 清理未使用资源
   ./deploy.sh cleanup
   ```

3. **数据库锁定**
   ```bash
   # 重启后端服务
   docker-compose restart backend
   ```

4. **网络连接问题**
   ```bash
   # 检查网络
   docker network ls
   docker network inspect api-proxy_api-proxy-network
   ```

### 调试模式

启用详细日志进行调试：

```bash
# 修改.env
RUST_LOG=debug
RUST_BACKTRACE=full

# 重启服务
./deploy.sh restart
```

## 📊 性能优化

### 1. 资源限制

在 `docker-compose.yaml` 中添加资源限制：

```yaml
services:
  backend:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

### 2. 缓存优化

调整Redis配置：

```yaml
redis:
  command: redis-server --maxmemory 512mb --maxmemory-policy allkeys-lru
```

### 3. 并发设置

调整Nginx worker进程：

```nginx
worker_processes auto;
worker_connections 2048;
```

## 📝 开发环境

### 本地开发

```bash
# 只启动基础服务（Redis）
docker-compose up -d redis

# 本地运行后端
cd ..
cargo run

# 本地运行前端
cd frontend
npm run dev
```

### 调试容器

```bash
# 进入容器调试
docker-compose exec backend bash
docker-compose exec frontend sh

# 查看容器资源使用
docker stats
```

## 🤝 贡献

1. Fork项目
2. 创建功能分支
3. 提交更改
4. 推送分支
5. 创建Pull Request

## 📄 许可证

本项目基于MIT许可证开源。详见根目录的LICENSE文件。

## 📞 支持

如有问题或建议，请：

1. 查看此文档的故障排除部分
2. 检查项目的GitHub Issues
3. 创建新的Issue描述问题

---

**祝您使用愉快！** 🎉