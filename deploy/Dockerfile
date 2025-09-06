# AI代理平台 - 前后端统一部署 Dockerfile
# 多阶段构建：前端构建 + 后端构建 + 统一运行

# ================================
# 前端构建阶段
# ================================
FROM node:18-alpine as frontend-builder

LABEL maintainer="AI Proxy Team"
LABEL description="AI代理平台 - 前后端统一部署版本"

# 设置工作目录
WORKDIR /app/web

# 复制前端package配置文件
COPY web/package*.json ./

# 安装依赖
RUN npm ci --only=production=false

# 复制前端源代码
COPY web/ ./

# 构建前端项目（生产环境构建）
ENV NODE_ENV=production
RUN npm run build

# ================================
# 后端构建阶段
# ================================
FROM rust:1.88.0-slim as backend-builder

# 设置工作目录
WORKDIR /app

# 安装必要的系统依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    cmake \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# 复制Cargo配置文件
COPY Cargo.toml Cargo.lock ./
COPY entity/Cargo.toml ./entity/
COPY migration/Cargo.toml ./migration/

# 复制后端源代码
COPY src ./src/
COPY entity/src ./entity/src/
COPY migration/src ./migration/src/

# 构建后端项目（发布模式）
RUN cargo build --release

# ================================
# 统一运行阶段
# ================================
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建应用用户
RUN useradd -r -s /bin/false -m -d /app apiproxy

# 设置工作目录
WORKDIR /app

# 从后端构建阶段复制二进制文件
COPY --from=backend-builder /app/target/release/api-proxy /app/api-proxy

# 从前端构建阶段复制静态文件
COPY --from=frontend-builder /app/web/dist /app/static

# 创建必要的目录
RUN mkdir -p /app/data /app/certs /app/logs /app/config

# 复制配置文件
COPY ../config/ ./config/

# 设置权限
RUN chown -R apiproxy:apiproxy /app
RUN chmod +x /app/api-proxy

# 切换到非root用户
USER apiproxy

# 暴露端口
# 9090: 统一服务端口（API + 静态文件）
# 8080: 重定向端口（可选）
EXPOSE 9090 8080

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9090/api/health || exit 1

# 设置环境变量
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV API_PROXY_CONFIG_PATH=/app/config/config.prod.toml
ENV API_PROXY_DATA_DIR=/app/data
ENV API_PROXY_STATIC_DIR=/app/static

# 启动命令
CMD ["./api-proxy"]