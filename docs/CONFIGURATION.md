# AI代理平台配置参数完整指南

本文档详细描述了AI代理平台所有可配置参数，包括后端服务、前端应用、Docker部署等各个方面的配置选项。

## 目录

- [1. 后端配置 (.toml 文件)](#1-后端配置-toml-文件)
  - [1.1 双端口服务器配置](#11-双端口服务器配置-dual_port)
  - [1.2 管理服务配置](#12-管理服务配置-dual_portmanagement)  
  - [1.3 代理服务配置](#13-代理服务配置-dual_portproxy)
  - [1.4 数据库配置](#14-数据库配置-database)
  - [1.5 缓存配置](#15-缓存配置-cache)
  - [1.6 Redis配置](#16-redis配置-redis)
- [2. 前端配置](#2-前端配置)
  - [2.1 构建配置](#21-构建配置-viteconfigts)
  - [2.2 运行时配置](#22-运行时配置-publicconfigjs)
  - [2.3 环境变量配置](#23-环境变量配置)
- [3. Docker部署配置](#3-docker部署配置)
  - [3.1 Docker Compose配置](#31-docker-compose配置)
  - [3.2 容器环境变量](#32-容器环境变量)
- [4. 系统环境变量](#4-系统环境变量)
- [5. 命令行参数](#5-命令行参数)
- [6. 配置文件优先级](#6-配置文件优先级)
- [7. 配置示例](#7-配置示例)

---

## 1. 后端配置 (.toml 文件)

后端配置采用TOML格式，支持多环境配置文件：
- `config/config.toml` - 默认配置
- `config/config.dev.toml` - 开发环境配置  
- `config/config.prod.toml` - 生产环境配置
- `deploy/config/config.dev.toml` - Docker开发环境配置
- `deploy/config/config.prod.toml` - Docker生产环境配置

### 1.1 双端口服务器配置 `[dual_port]`

AI代理平台采用双端口分离架构，管理API和代理服务分别运行在不同端口。

```toml
[dual_port]
workers = 4  # 工作线程数
```

**参数详解:**

| 参数 | 类型 | 默认值 | 范围 | 说明 |
|------|------|--------|------|------|
| `workers` | `usize` | CPU核心数 | 1-128 | Pingora服务器工作线程数量，建议设置为CPU核心数的1-2倍 |

### 1.2 管理服务配置 `[dual_port.management]`

管理服务负责提供管理API、统计界面、用户管理等功能。

```toml
[dual_port.management]
enabled = true
route_prefixes = ["/api", "/admin", "/"]

[dual_port.management.http]
host = "127.0.0.1"  # 监听地址
port = 9090         # 监听端口

[dual_port.management.access_control]
allowed_ips = ["127.0.0.1/32", "::1/128", "10.0.0.0/8"]
denied_ips = []
require_auth = true
auth_methods = ["ApiKey", "JWT"]

[dual_port.management.cors]
enabled = true
origins = ["*"]  # 开发环境，生产环境应指定具体域名
```

**参数详解:**

#### 基础配置
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enabled` | `bool` | `true` | 是否启用管理接口 |
| `route_prefixes` | `Vec<String>` | `["/api", "/admin", "/"]` | 管理API路由前缀列表 |

#### HTTP监听配置 `[dual_port.management.http]`
| 参数 | 类型 | 默认值 | 范围 | 说明 |
|------|------|--------|------|------|
| `host` | `String` | `"127.0.0.1"` | 有效IP地址 | 管理API监听地址，生产环境可设为 `"0.0.0.0"` |
| `port` | `u16` | `9090` | 1-65535 | 管理API监听端口 |

#### 访问控制配置 `[dual_port.management.access_control]`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `allowed_ips` | `Vec<String>` | `["127.0.0.1/32", "::1/128"]` | 允许访问的IP地址范围（CIDR格式） |
| `denied_ips` | `Vec<String>` | `[]` | 禁止访问的IP地址范围（CIDR格式） |
| `require_auth` | `bool` | `false`(开发) / `true`(生产) | 是否需要认证才能访问 |
| `auth_methods` | `Vec<String>` | `["ApiKey"]` | 支持的认证方式：`ApiKey`, `JWT`, `BasicAuth`, `ClientCert` |

#### CORS配置 `[dual_port.management.cors]`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enabled` | `bool` | `true` | 是否启用CORS支持 |
| `origins` | `Vec<String>` | `["*"]`(开发) | 允许的跨域源地址，生产环境应指定具体域名 |

### 1.3 代理服务配置 `[dual_port.proxy]`

代理服务负责转发AI API请求，实现负载均衡、认证、限流等功能。

```toml
[dual_port.proxy]
enabled = true
route_prefixes = ["/v1", "/proxy"]

[dual_port.proxy.http]  
host = "0.0.0.0"
port = 8080

[dual_port.proxy.load_balancing]
strategy = "RoundRobin"
health_check_interval = 30
failure_threshold = 3
recovery_threshold = 2
```

**参数详解:**

#### 基础配置
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enabled` | `bool` | `true` | 是否启用代理服务 |
| `route_prefixes` | `Vec<String>` | `["/v1", "/proxy"]` | 代理服务路由前缀列表 |

#### HTTP监听配置 `[dual_port.proxy.http]`
| 参数 | 类型 | 默认值 | 范围 | 说明 |
|------|------|--------|------|------|
| `host` | `String` | `"0.0.0.0"` | 有效IP地址 | 代理服务监听地址 |
| `port` | `u16` | `8080` | 1-65535 | 代理服务监听端口 |

#### 负载均衡配置 `[dual_port.proxy.load_balancing]`
| 参数 | 类型 | 默认值 | 可选值 | 说明 |
|------|------|--------|-------|------|
| `strategy` | `String` | `"RoundRobin"` | `RoundRobin`, `WeightedRoundRobin`, `LeastConnections`, `IpHash`, `Random`, `HealthBest` | 负载均衡策略 |
| `health_check_interval` | `u64` | `30` | 5-300 | 健康检查间隔（秒） |
| `failure_threshold` | `u32` | `3` | 1-10 | 连续失败多少次标记为不健康 |
| `recovery_threshold` | `u32` | `2` | 1-10 | 连续成功多少次标记为健康 |

**负载均衡策略说明:**
- `RoundRobin`: 轮询调度
- `WeightedRoundRobin`: 加权轮询调度  
- `LeastConnections`: 最少连接数调度
- `IpHash`: 基于客户端IP哈希调度
- `Random`: 随机调度
- `HealthBest`: 健康度最优调度

### 1.4 数据库配置 `[database]`

系统使用SQLite数据库存储用户数据、API密钥、统计信息等。

```toml
[database]
url = "sqlite://./data/api_proxy.db"
max_connections = 20
connect_timeout = 30  
query_timeout = 60
```

**参数详解:**

| 参数 | 类型 | 默认值 | 范围 | 说明 |
|------|------|--------|------|------|
| `url` | `String` | `"sqlite://./data/api_proxy.db"` | SQLite URL | 数据库连接字符串 |
| `max_connections` | `u32` | `20` | 1-100 | 数据库连接池最大连接数 |
| `connect_timeout` | `u64` | `30` | 5-300 | 数据库连接超时时间（秒） |
| `query_timeout` | `u64` | `60` | 10-600 | 数据库查询超时时间（秒） |

**SQLite URL格式:**
- 相对路径: `sqlite://./data/db.db`
- 绝对路径: `sqlite:///opt/data/db.db` 
- 内存数据库: `sqlite::memory:`

### 1.5 缓存配置 `[cache]`

缓存系统用于提升API响应速度，支持内存缓存和Redis缓存。

```toml
[cache]
cache_type = "memory"      # 缓存类型
memory_max_entries = 10000 # 内存缓存最大条目数
default_ttl = 300          # 默认过期时间
enabled = true             # 是否启用缓存
```

**参数详解:**

| 参数 | 类型 | 默认值 | 可选值/范围 | 说明 |
|------|------|--------|-------------|------|
| `cache_type` | `String` | `"memory"` | `memory`, `redis` | 缓存后端类型 |
| `memory_max_entries` | `usize` | `10000` | 100-100000 | 内存缓存最大条目数（仅memory类型有效） |
| `default_ttl` | `u64` | `300` | 60-86400 | 默认缓存过期时间（秒） |
| `enabled` | `bool` | `true` | - | 是否启用缓存系统 |

### 1.6 Redis配置 `[redis]`

Redis用作缓存后端和会话存储。

```toml
[redis]
url = "redis://127.0.0.1:6379/0"
pool_size = 20
host = "127.0.0.1"
port = 6379
database = 0
password = ""              # 可选
connection_timeout = 10
default_ttl = 3600
max_connections = 20
```

**参数详解:**

| 参数 | 类型 | 默认值 | 范围 | 说明 |
|------|------|--------|------|------|
| `url` | `String` | `"redis://127.0.0.1:6379/0"` | Redis URL | Redis连接字符串 |
| `pool_size` | `u32` | `20` | 1-100 | Redis连接池大小 |
| `host` | `String` | `"127.0.0.1"` | 有效主机名/IP | Redis服务器地址 |
| `port` | `u16` | `6379` | 1-65535 | Redis服务器端口 |
| `database` | `u8` | `0` | 0-15 | Redis数据库编号 |
| `password` | `Option<String>` | `None` | - | Redis连接密码（可选） |
| `connection_timeout` | `u64` | `10` | 1-60 | 连接超时时间（秒） |
| `default_ttl` | `u64` | `3600` | 60-86400 | 默认缓存TTL（秒） |
| `max_connections` | `u32` | `20` | 1-100 | 最大连接数 |

**Redis URL格式:**
- 无密码: `redis://localhost:6379/0`
- 有密码: `redis://:password@localhost:6379/0`
- 用户认证: `redis://username:password@localhost:6379/0`

### 1.7 说明

**注意:** 旧版本配置中的 `[trace]`、`[services]` 和 `[tls]` 配置段已被移除。

- **请求追踪功能** 现在通过 UnifiedTraceSystem 自动启用，无需额外配置
- **服务控制** 现在通过 `dual_port.enabled_services` 配置段管理
- **TLS配置** 计划在后续版本中重新设计实现

---

## 2. 前端配置

前端基于React 18 + TypeScript + ESBuild构建，支持多种配置方式。

### 2.1 构建配置 (scripts/build.mjs)

```typescript
export default defineConfig({
  plugins: [vue()],
  server: {
    host: '0.0.0.0',
    port: 3001,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:9090',
        changeOrigin: true
      }
    }
  },
  build: {
    target: 'es2015',
    outDir: 'dist',
    sourcemap: false,
    minify: 'terser'
  }
})
```

**参数详解:**

#### 开发服务器配置 `server`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `host` | `string` | `"0.0.0.0"` | 开发服务器监听地址 |
| `port` | `number` | `3001` | 开发服务器端口 |
| `proxy` | `object` | API代理配置 | 开发环境API代理设置 |

#### 构建配置 `build`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `target` | `string` | `"es2015"` | 编译目标版本 |
| `outDir` | `string` | `"dist"` | 构建输出目录 |
| `sourcemap` | `boolean` | `false` | 是否生成源码映射 |
| `minify` | `string` | `"terser"` | 代码压缩工具 |

### 2.2 运行时配置 (public/config.js)

```javascript
window.APP_CONFIG = {
  // API配置
  api: {
    baseURL: '/api',
    timeout: 30000,
    retryCount: 3,
    retryDelay: 1000
  },
  
  // WebSocket配置
  websocket: {
    url: '/ws',
    reconnectInterval: 5000,
    maxReconnectAttempts: 5,
    pingInterval: 30000
  },
  
  // 应用配置
  app: {
    name: 'AI代理平台',
    version: '1.0.0',
    debug: false
  }
}
```

**参数详解:**

#### API配置 `api`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `baseURL` | `string` | `"/api"` | API请求基础URL |
| `timeout` | `number` | `30000` | 请求超时时间（毫秒） |
| `retryCount` | `number` | `3` | 请求失败重试次数 |
| `retryDelay` | `number` | `1000` | 重试间隔时间（毫秒） |

#### WebSocket配置 `websocket`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `url` | `string` | `"/ws"` | WebSocket连接URL |
| `reconnectInterval` | `number` | `5000` | 重连间隔时间（毫秒） |
| `maxReconnectAttempts` | `number` | `5` | 最大重连尝试次数 |
| `pingInterval` | `number` | `30000` | 心跳检测间隔（毫秒） |

#### 应用配置 `app`
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `name` | `string` | `"AI代理平台"` | 应用名称 |
| `version` | `string` | `"1.0.0"` | 应用版本号 |
| `debug` | `boolean` | `false` | 是否启用调试模式 |

### 2.3 环境变量配置

前端支持通过环境变量覆盖配置，构建时自动注入到应用中。

**.env 文件示例:**
```bash
# 开发环境配置
VITE_API_BASE_URL=/api
VITE_WS_URL=/ws
VITE_APP_VERSION=1.0.0
VITE_LOG_LEVEL=info
VITE_DEBUG=false

# 生产环境特定配置
VITE_API_BASE_URL=https://api.yourdomain.com
VITE_WS_URL=wss://ws.yourdomain.com
```

**环境变量说明:**

| 变量名 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `API_BASE_URL` | `string` | `"/api"` | API基础URL，生产环境通常为完整域名 |
| `WS_URL` | `string` | `"/ws"` | WebSocket URL |
| `APP_VERSION` | `string` | `"1.0.0"` | 应用版本号 |
| `REACT_APP_LOG_LEVEL` | `string` | `"info"` | 前端日志级别 |
| `DEBUG` | `boolean` | `false` | 调试模式开关 |

---

## 3. Docker部署配置

### 3.1 Docker Compose配置

**docker-compose.yaml 主要配置段:**

```yaml
services:
  backend:
    build: 
      context: .
      dockerfile: deploy/Dockerfile.backend
    ports:
      - "8080:8080"   # Pingora代理服务
      - "9090:9090"   # Axum管理API
    volumes:
      - backend_data:/app/data         # 数据持久化
      - backend_logs:/app/logs         # 日志存储
      - ./certs:/app/certs:ro          # TLS证书（只读）
      - ./deploy/config:/app/config:ro # 配置文件（只读）
    environment:
      - RUST_LOG=info
      - API_PROXY_CONFIG_PATH=/app/config/config.prod.toml
    depends_on:
      - redis
    restart: unless-stopped
    
  frontend:
    build:
      context: ./frontend  
      dockerfile: ../deploy/Dockerfile.frontend
    ports:
      - "3000:80"
    environment:
      - VITE_API_BASE_URL=/api
      - VITE_WS_URL=/ws
      - VITE_APP_VERSION=1.0.0
    depends_on:
      - backend
    restart: unless-stopped
    
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: >
      redis-server
      --appendonly yes
      --maxmemory 256mb
      --maxmemory-policy allkeys-lru
    restart: unless-stopped

volumes:
  backend_data:
  backend_logs:
  redis_data:
```

**参数详解:**

#### 后端服务配置
| 配置项 | 说明 |
|--------|------|
| `ports: "8080:8080"` | Pingora代理服务端口映射 |
| `ports: "9090:9090"` | Axum管理API端口映射 |
| `volumes: backend_data:/app/data` | 数据库文件持久化 |
| `volumes: backend_logs:/app/logs` | 日志文件持久化 |
| `volumes: ./certs:/app/certs:ro` | TLS证书挂载（只读） |
| `volumes: ./deploy/config:/app/config:ro` | 配置文件挂载（只读） |

#### 前端服务配置
| 配置项 | 说明 |
|--------|------|
| `ports: "3000:80"` | 前端Web服务端口映射 |
| `environment: VITE_API_BASE_URL` | API基础URL环境变量 |

#### Redis服务配置
| 配置项 | 说明 |
|--------|------|
| `ports: "6379:6379"` | Redis服务端口映射 |
| `command: --appendonly yes` | 启用AOF持久化 |
| `command: --maxmemory 256mb` | 限制最大内存使用 |
| `command: --maxmemory-policy allkeys-lru` | 内存淘汰策略 |

### 3.2 容器环境变量

#### 后端容器环境变量
```bash
# 日志配置
RUST_LOG=info                                    # Rust日志级别
RUST_BACKTRACE=1                                 # 错误堆栈跟踪

# 应用配置
API_PROXY_CONFIG_PATH=/app/config/config.prod.toml  # 配置文件路径
API_PROXY_DATA_DIR=/app/data                     # 数据目录
DATABASE_URL=sqlite:///app/data/api-proxy.db     # 数据库URL

# 安全配置
PROXY_CONFIG_KEY=your-32-byte-hex-key            # 配置加密密钥
```

#### 前端容器环境变量
```bash
# API配置
VITE_API_BASE_URL=/api                          # API基础URL
VITE_WS_URL=/ws                                 # WebSocket URL

# 应用信息
VITE_APP_VERSION=1.0.0                          # 应用版本
VITE_LOG_LEVEL=info                             # 日志级别
VITE_DEBUG=false                                # 调试模式
```

---

## 4. 系统环境变量

系统环境变量具有最高配置优先级，可以覆盖配置文件中的设置。

### 4.1 后端环境变量

```bash
# === 系统配置 ===
RUST_LOG=debug                                  # Rust日志级别：trace,debug,info,warn,error
RUST_BACKTRACE=full                             # 错误堆栈：0,1,full
RUST_LOG_STYLE=auto                             # 日志样式：auto,always,never

# === 应用配置 ===
API_PROXY_CONFIG_PATH=/etc/api-proxy/config.toml   # 配置文件路径
API_PROXY_DATA_DIR=/var/lib/api-proxy           # 数据存储目录
API_PROXY_LOG_DIR=/var/log/api-proxy            # 日志存储目录
API_PROXY_CERT_DIR=/etc/api-proxy/certs         # TLS证书目录

# === 数据库配置 ===
DATABASE_URL=sqlite:///var/lib/api-proxy/db.db  # 数据库连接URL
DATABASE_MAX_CONNECTIONS=50                     # 最大连接数
DATABASE_CONNECT_TIMEOUT=30                     # 连接超时（秒）
DATABASE_QUERY_TIMEOUT=60                       # 查询超时（秒）

# === Redis配置 ===  
REDIS_URL=redis://redis.internal:6379/0         # Redis连接URL
REDIS_PASSWORD=your-redis-password               # Redis密码
REDIS_MAX_CONNECTIONS=50                        # 最大连接数
REDIS_CONNECTION_TIMEOUT=10                     # 连接超时（秒）

# === 服务配置 ===
PROXY_HOST=0.0.0.0                             # 代理服务监听地址
PROXY_PORT=8080                                # 代理服务端口
MANAGEMENT_HOST=127.0.0.1                      # 管理API监听地址
MANAGEMENT_PORT=9090                           # 管理API端口

# === 安全配置 ===
PROXY_CONFIG_KEY=0123456789abcdef...            # 配置加密密钥（64位十六进制）
JWT_SECRET=your-jwt-secret-key                  # JWT签名密钥
API_KEY_SALT=your-api-key-salt                  # API密钥加密盐

# === 追踪配置 ===
TRACE_ENABLED=true                              # 是否启用追踪
TRACE_LEVEL=2                                   # 追踪级别：0,1,2
TRACE_SAMPLING_RATE=1.0                         # 采样率：0.0-1.0
```

### 4.2 前端环境变量

```bash
# === 构建时环境变量 ===
VITE_API_BASE_URL=https://api.yourdomain.com    # API基础URL
VITE_WS_URL=wss://ws.yourdomain.com             # WebSocket URL
VITE_APP_NAME=AI代理平台                         # 应用名称
VITE_APP_VERSION=1.0.0                          # 应用版本
VITE_APP_BUILD_TIME=2024-01-01T00:00:00Z        # 构建时间

# === 功能开关 ===
VITE_ENABLE_WEBSOCKET=true                      # 是否启用WebSocket
VITE_ENABLE_ANALYTICS=false                     # 是否启用统计分析
VITE_ENABLE_DEBUG_PANEL=false                   # 是否显示调试面板

# === 开发配置 ===
VITE_LOG_LEVEL=info                             # 日志级别
VITE_DEBUG=false                                # 调试模式
VITE_MOCK_API=false                             # 是否使用Mock API
```

---

## 5. 命令行参数

系统支持通过命令行参数覆盖配置文件设置。

### 5.1 服务启动参数

```bash
# 基础启动命令
./api-proxy --config config.prod.toml

# 完整参数示例
./api-proxy \
  --config /path/to/config.toml \
  --host 0.0.0.0 \
  --port 8080 \
  --https-port 8443 \
  --management-port 9090 \
  --workers 8 \
  --database-url sqlite:///data/db.db \
  --log-level info \
  --enable-trace \
  --trace-level 2 \
  --trace-sampling-rate 1.0
```

**参数详解:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `--config <PATH>` | `String` | 指定配置文件路径 |
| `--host <HOST>` | `String` | 代理服务监听地址 |
| `--port <PORT>` | `u16` | 代理服务端口 |
| `--https-port <PORT>` | `u16` | HTTPS服务端口 |
| `--management-port <PORT>` | `u16` | 管理API端口 |
| `--workers <NUM>` | `usize` | 工作线程数 |
| `--database-url <URL>` | `String` | 数据库连接URL |
| `--log-level <LEVEL>` | `String` | 日志级别 |
| `--enable-trace` | `flag` | 启用请求追踪 |
| `--disable-trace` | `flag` | 禁用请求追踪 |
| `--trace-level <LEVEL>` | `i32` | 追踪级别（0-2） |
| `--trace-sampling-rate <RATE>` | `f64` | 追踪采样率（0.0-1.0） |

### 5.2 管理工具参数

```bash
# 数据库迁移
./api-proxy migrate --database-url sqlite:///data/db.db

# 用户管理
./api-proxy user create --email admin@example.com --password secret
./api-proxy user list
./api-proxy user delete --id 1

# API密钥管理
./api-proxy apikey generate --user-id 1 --provider openai
./api-proxy apikey list --user-id 1
./api-proxy apikey revoke --key-id 1

# 统计信息
./api-proxy stats --from 2024-01-01 --to 2024-01-31
./api-proxy health --check-all
```

---

## 6. 配置文件优先级

配置系统采用分层覆盖机制，优先级从高到低：

1. **环境变量** (最高优先级)
   - `DATABASE_URL`, `REDIS_URL` 等

2. **命令行参数**
   - `--port`, `--host`, `--workers` 等

3. **环境特定配置文件**
   - `config.dev.toml` (开发环境)
   - `config.prod.toml` (生产环境)
   - `config.test.toml` (测试环境)

4. **通用配置文件**
   - `config.toml` (默认配置)

5. **代码默认值** (最低优先级)
   - 硬编码在程序中的默认值

**示例:**
```bash
# 配置文件中设置
[dual_port.proxy.http]
port = 8080

# 环境变量覆盖
export PROXY_PORT=8081

# 命令行参数覆盖
./api-proxy --port 8082

# 最终生效: 8082 (命令行参数优先级最高)
```

---

## 7. 配置示例

### 7.1 开发环境完整配置

**config.dev.toml:**
```toml
# 开发环境配置 - 注重开发便利性

[dual_port]
workers = 2  # 开发环境使用较少线程

[dual_port.management]
enabled = true
route_prefixes = ["/api", "/admin", "/"]

[dual_port.management.http]
host = "0.0.0.0"  # 允许外部访问以便测试
port = 9090

[dual_port.management.access_control]
allowed_ips = ["0.0.0.0/0"]  # 开发环境允许所有IP
denied_ips = []
require_auth = false         # 开发环境关闭认证
auth_methods = ["ApiKey"]

[dual_port.management.cors]
enabled = true
origins = ["*"]  # 开发环境允许所有跨域请求

[dual_port.proxy]
enabled = true
route_prefixes = ["/v1", "/proxy"]

[dual_port.proxy.http]
host = "0.0.0.0"
port = 8080

[dual_port.proxy.load_balancing]
strategy = "RoundRobin"
health_check_interval = 60  # 开发环境较长间隔
failure_threshold = 5
recovery_threshold = 2

[database]
url = "sqlite://./data/dev.db"
max_connections = 5     # 开发环境较少连接
connect_timeout = 30
query_timeout = 60

[cache]
cache_type = "memory"   # 开发环境使用内存缓存
memory_max_entries = 1000
default_ttl = 60        # 开发环境较短TTL便于测试
enabled = true

[redis]
url = "redis://127.0.0.1:6379/1"  # 使用数据库1避免冲突
pool_size = 5
host = "127.0.0.1"
port = 6379
database = 1
connection_timeout = 5
default_ttl = 300
max_connections = 5

[trace]
enabled = true
default_trace_level = 2  # 开发环境收集完整数据
sampling_rate = 1.0      # 100%采样便于调试
max_batch_size = 50
flush_interval = 5       # 快速刷新便于查看
timeout_seconds = 30
async_write = true
enable_phases = true
enable_health_metrics = true
enable_performance_metrics = true

[services]
management = true
proxy = true
health_check = true
monitoring = true
```

### 7.2 生产环境完整配置

**config.prod.toml:**
```toml
# 生产环境配置 - 注重性能和安全

[dual_port]
workers = 16  # 生产环境使用更多线程

[dual_port.management]
enabled = true
route_prefixes = ["/api", "/admin"]

[dual_port.management.http]
host = "127.0.0.1"  # 生产环境限制内网访问
port = 9090

[dual_port.management.access_control]
allowed_ips = ["127.0.0.1/32", "10.0.0.0/8", "172.16.0.0/12"]
denied_ips = []
require_auth = true      # 生产环境启用认证
auth_methods = ["ApiKey", "JWT"]

[dual_port.management.cors]
enabled = true
origins = ["https://admin.yourdomain.com", "https://dashboard.yourdomain.com"]

[dual_port.proxy]
enabled = true
route_prefixes = ["/v1", "/proxy"]

[dual_port.proxy.http]
host = "0.0.0.0"
port = 8080

[dual_port.proxy.load_balancing]
strategy = "HealthBest"  # 生产环境使用健康度最优
health_check_interval = 15
failure_threshold = 2
recovery_threshold = 3

[database]
url = "sqlite:///var/lib/api-proxy/prod.db"
max_connections = 50    # 生产环境更多连接
connect_timeout = 10    # 更短超时时间
query_timeout = 30

[cache]
cache_type = "redis"    # 生产环境使用Redis缓存
default_ttl = 3600     # 较长TTL提高性能
enabled = true

[redis]
url = "redis://redis.internal:6379/0"
pool_size = 50         # 生产环境大连接池
host = "redis.internal"
port = 6379
database = 0
connection_timeout = 5
default_ttl = 3600
max_connections = 50

[trace]
enabled = true
default_trace_level = 2  # 生产环境也收集完整数据
sampling_rate = 1.0      # 100%采样确保数据完整
max_batch_size = 200     # 更大批量提高性能
flush_interval = 10
timeout_seconds = 60
async_write = true
enable_phases = true
enable_health_metrics = true
enable_performance_metrics = true

[services]
management = true
proxy = true
health_check = true
monitoring = true

[tls]
cert_path = "/etc/api-proxy/certs"
acme_email = "admin@yourdomain.com"
domains = ["api.yourdomain.com", "proxy.yourdomain.com"]
```

### 7.3 Docker部署配置

**docker-compose.prod.yaml:**
```yaml
version: '3.8'

services:
  backend:
    build: 
      context: .
      dockerfile: deploy/Dockerfile.backend
    ports:
      - "8080:8080"
      - "9090:9090"
    volumes:
      - /var/lib/api-proxy:/app/data
      - /var/log/api-proxy:/app/logs
      - /etc/api-proxy/certs:/app/certs:ro
      - /etc/api-proxy/config:/app/config:ro
    environment:
      # 系统环境
      - RUST_LOG=info
      - RUST_BACKTRACE=1
      
      # 应用配置
      - API_PROXY_CONFIG_PATH=/app/config/config.prod.toml
      - API_PROXY_DATA_DIR=/app/data
      - API_PROXY_LOG_DIR=/app/logs
      
      # 数据库配置
      - DATABASE_URL=sqlite:///app/data/prod.db
      - DATABASE_MAX_CONNECTIONS=50
      
      # Redis配置
      - REDIS_URL=redis://redis:6379/0
      - REDIS_MAX_CONNECTIONS=50
      
      # 安全配置
      - PROXY_CONFIG_KEY=${PROXY_CONFIG_KEY}
      - JWT_SECRET=${JWT_SECRET}
      
      # 追踪配置
      - TRACE_ENABLED=true
      - TRACE_LEVEL=2
      - TRACE_SAMPLING_RATE=1.0
    depends_on:
      - redis
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 1G
          cpus: '1.0'
        reservations:
          memory: 512M
          cpus: '0.5'

  frontend:
    build:
      context: ./frontend
      dockerfile: ../deploy/Dockerfile.frontend
      args:
        - VITE_API_BASE_URL=https://api.yourdomain.com
        - VITE_WS_URL=wss://ws.yourdomain.com
        - VITE_APP_VERSION=1.0.0
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /etc/api-proxy/nginx:/etc/nginx/conf.d:ro
    depends_on:
      - backend
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    ports:
      - "127.0.0.1:6379:6379"  # 限制本地访问
    volumes:
      - redis_prod_data:/data
      - /etc/api-proxy/redis.conf:/usr/local/etc/redis/redis.conf:ro
    command: redis-server /usr/local/etc/redis/redis.conf
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 256M
        reservations:
          memory: 128M

volumes:
  redis_prod_data:
```

**.env.prod:**
```bash
# 安全配置 - 生产环境必须设置
PROXY_CONFIG_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
JWT_SECRET=your-very-secure-jwt-secret-key-here
API_KEY_SALT=your-api-key-encryption-salt

# 数据库配置
DATABASE_URL=sqlite:///var/lib/api-proxy/prod.db

# Redis配置  
REDIS_URL=redis://redis:6379/0
REDIS_PASSWORD=your-redis-password-here
```

---

## 总结

AI代理平台配置系统特点：

1. **分层配置**: 支持环境变量、命令行、配置文件多层覆盖
2. **环境隔离**: 开发、测试、生产环境独立配置
3. **安全可靠**: 支持配置加密、权限控制、输入验证
4. **灵活扩展**: 模块化设计，易于添加新的配置项
5. **监控友好**: 完整的追踪和监控配置支持
6. **容器化**: 原生支持Docker和Kubernetes部署

建议在使用时：
- 开发环境优先便利性，生产环境优先安全性
- 敏感配置使用环境变量而不是配置文件
- 定期检查配置文件权限和安全性
- 使用配置验证确保参数正确性
- 建立配置变更的版本控制和审计机制

更多详细信息，请参考项目的其他文档文件。