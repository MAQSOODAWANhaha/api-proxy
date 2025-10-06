# AI代理系统完整架构设计与详细设计文档

## 文档版本信息

| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| 1.0 | 2024-12 | 系统架构师 | 初始版本 |
| 2.0 | 2025-08 | 系统架构师 | 生产版本发布，数据驱动架构完成，前端技术栈更新 |

---

## 目录

1. [项目概述](#1-项目概述)
2. [系统架构设计](#2-系统架构设计)
3. [数据库详细设计](#3-数据库详细设计)
4. [核心模块详细设计](#4-核心模块详细设计)
5. [API接口详细设计](#5-api接口详细设计)
6. [安全设计](#6-安全设计)
7. [性能与监控设计](#7-性能与监控设计)
8. [部署设计](#8-部署设计)
9. [测试策略](#9-测试策略)
10. [项目实施计划](#10-项目实施计划)

---

## 1. 项目概述

### 1.1 项目目标

构建一个企业级AI服务代理平台，为用户提供统一的AI服务访问接口，支持多个主流AI服务提供商，具备负载均衡、监控统计、安全防护等完整功能。

### 1.2 核心功能

**用户管理系统**
- 用户注册、登录、权限管理
- 基于JWT的无状态认证

**API密钥管理**
- 用户对外API密钥（每种服务商类型只能创建一个）
- 内部代理商API密钥池（每种类型可创建多个，组成号池）
- 密钥的增删改查和状态管理

**智能负载均衡**
- 轮询调度：按顺序轮流分配请求到各个上游服务器
- 权重调度：根据权重比例分配请求到上游服务器
- 健康优选：优先选择健康状态最佳的上游服务器

**多AI服务商支持**
- OpenAI ChatGPT API完全兼容
- Google Gemini API支持
- Anthropic Claude API支持
- 统一接口格式转换

**监控与统计**
- 实时请求统计（成功/失败/响应时间）
- API健康状态监控
- Token使用量统计
- 错误日志记录

**安全防护**
- TLS加密传输
- 证书自动续期
- 源信息隐藏
- 请求重试机制

### 1.3 技术栈

**后端技术栈**
- **核心框架**: Rust 2024 Edition + Pingora 0.6.0 (代理服务) + Axum 0.8.4 (管理服务)
- **数据库**: SQLite + Sea-ORM 1.x
- **缓存**: Redis
- **错误处理**: thiserror + anyhow
- **TLS**: rustls + acme-lib

**前端技术栈**
- **框架**: React 18 + TypeScript
- **UI库**: shadcn/ui + Radix UI
- **构建工具**: ESBuild (通过自定义脚本 `scripts/build.mjs`)
- **状态管理**: Zustand
- **路由**: React Router
- **样式**: Tailwind CSS

### 1.4 系统特色

**双端口分离架构**: Pingora专注AI代理(8080)，Axum专注管理API(9090)
**职责清晰分离**: 代理服务专注性能，管理服务专注业务逻辑
**完全数据驱动设计**: 所有配置存储在数据库中，支持动态更新无需重启
**智能字段提取**: 基于数据库配置的TokenFieldExtractor和ModelExtractor
**源信息隐藏**: 完全隐藏客户端信息，AI服务商只能看到代理服务器信息
**高可用设计**: 支持故障自动切换和健康检查，服务间故障隔离
**灵活部署**: 支持独立扩展代理服务和管理服务

---

## 2. 系统架构设计

### 2.1 总体架构图

```mermaid
graph TD
    subgraph "用户端 (Clients)"
        Client[Web/Mobile/SDK]
    end

    subgraph "接入层 (Edge Layer)"
        ProxyServer["Pingora 代理服务<br/>:8080"]
        MgmtServer["Axum 管理服务<br/>:9090"]
    end

    subgraph "共享核心服务 (Shared Core Services)"
        AuthService["认证服务<br/>(API Key, JWT, OAuth2)"]
        Scheduler["调度器<br/>(轮询, 权重, 健康优先)"]
        HealthChecker["健康检查器"]
        TracingService["追踪与统计服务"]
    end

    subgraph "数据持久层 (Data Layer)"
        Database[(SQLite / PostgreSQL)]
        Cache[(Redis)]
    end

    subgraph "上游服务商 (Upstream Providers)"
        Provider[OpenAI, Gemini, Claude, etc.]
    end

    Client -- "AI 请求" --> ProxyServer
    Client -- "管理操作" --> MgmtServer

    ProxyServer -- "调用" --> AuthService
    ProxyServer -- "调用" --> Scheduler
    ProxyServer -- "调用" --> TracingService
    MgmtServer -- "调用" --> AuthService
    MgmtServer -- "读/写" --> Database
    MgmtServer -- "读/写" --> Cache

    Scheduler -- "依赖" --> HealthChecker
    Scheduler -- "读取" --> Database
    Scheduler -- "读取" --> Cache

    HealthChecker -- "更新" --> Database
    HealthChecker -- "更新" --> Cache
    TracingService -- "写入" --> Database

    ProxyServer -- "转发请求至" --> Provider
```

### 2.2 核心设计原则

**双端口分离原则**
- Pingora(8080)专注AI代理：负载均衡、限速、熔断、请求转发
- Axum(9090)专注管理功能：用户管理、API管理、统计查询、配置管理
- TLS证书管理统一处理，两个服务共享

**职责专业化原则**
- 代理服务追求极致性能和稳定性
- 管理服务提供丰富的业务功能
- 认证服务层统一共享
- 数据存储层统一管理

**故障隔离原则**
- 管理服务故障不影响AI代理功能
- 代理服务故障不影响管理操作
- 独立扩展和部署能力
- 分离的监控和告警体系

**安全优先原则**
- 端到端加密传输
- 完全隐藏源请求信息
- 分层安全策略：代理端口对外，管理端口内网
- 最小权限原则

**代理透明原则**
- Pingora 代理服务保持最大透明性：只做路径匹配、认证、转发
- 不解析具体 URI 参数和业务逻辑，完整保持客户端请求
- 所有 AI API 的具体处理逻辑交给上游服务商
- 避免过度复杂的路由判断，确保高性能和兼容性

### 2.3 数据流设计

**AI代理请求流程（端口8080）** - 协调器与管线模式
```
Client → Pingora(8080) → RequestFilter Pipeline → Upstream → Response → Logging & Tracing
```
**管线步骤 (Pipeline Steps)**:
1.  **认证 (Authentication)**: `AuthenticationService` 验证入口密钥 (API Key/OAuth 2.0)。
2.  **追踪启动 (Start Trace)**: `TracingService` 初始化追踪记录。
3.  **速率限制 (Rate Limit)**: 检查并应用速率限制策略。
4.  **配置加载 (Load Config)**: 从 `ProviderConfigManager` 加载服务商特定配置。
5.  **密钥选择 (Key Selection)**: `ApiKeyPoolManager` + `HealthChecker` + `Scheduler` 选择一个健康的后端密钥。
6.  **追踪更新 (Update Trace)**: `TracingService` 更新追踪记录，关联后端密钥。

**管理API请求流程（端口9090）**
```
Client → Axum(9090) → Auth → Business Logic → Database/Redis → Response
```

**双服务协同数据同步**
```
配置变更: Axum管理服务 → Database → Redis缓存 → Pingora代理服务自动更新
健康状态: Pingora代理服务 → Redis缓存 → Axum管理服务监控展示
统计数据: Pingora代理服务 → Database/Redis → Axum管理服务统计分析
```

## 2.4 Pingora 代理实现设计

### 2.4.1 代理核心逻辑 (Pipeline)

**设计原则**: `ProxyService` 的 `request_filter` 作为协调器，按顺序执行一系列独立的、可复用的处理步骤（管线模式），以完成请求的准备工作。

```rust
// src/proxy/service.rs -> request_filter()
async fn request_filter(&self, session: &mut Session, ctx: &mut ProxyContext) -> Result<bool> {
    // 预检请求直接返回
    if session.req_header().method == "OPTIONS" {
        // ... handle CORS preflight
        return Ok(true); // 终止代理
    }

    // --- 请求处理管线开始 ---

    // 步骤 1: 认证
    let auth_result = self.auth_service.authenticate_proxy_request(session, ctx).await?;
    ctx.set_auth_result(auth_result);

    // 步骤 2: 启动追踪
    self.tracing_service.start_trace(session, ctx).await?;

    // 步骤 3: 速率限制
    self.rate_limit_service.check_rate_limit(ctx).await?;

    // 步骤 4: 加载 Provider 配置
    self.provider_config_service.load_provider_config(ctx).await?;

    // 步骤 5: 选择后端 API 密钥
    let selection_result = self.api_key_selection_service.select_api_key(ctx).await?;
    ctx.set_selection_result(selection_result);

    // 步骤 6: 更新追踪信息
    self.tracing_service.update_extended_trace_info(ctx).await?;

    // --- 管线结束, 准备转发 ---
    Ok(false) // false 表示继续执行 Pingora 的生命周期 (upstream_peer, etc.)
}
```

### 2.4.2 路径匹配策略

**代理路径** (转发到 AI 服务商):
- `/v1/*` - OpenAI 兼容 API 格式
- `/proxy/{provider_name}/*` - 通用代理路径，动态识别 `provider_name`

**非代理路径处理**:
- `/api/*`, `/admin/*` - 在 `request_filter` 阶段被识别并由 Axum 处理，不会进入 Pingora 的转发逻辑。

### 2.4.3 上游选择逻辑

```rust
// src/proxy/service.rs -> upstream_peer()
async fn upstream_peer(&self, _session: &mut Session, ctx: &ProxyContext) -> Result<Box<HttpPeer>> {
    // 从上下文中获取已加载的 Provider 配置
    let provider_config = ctx.provider_config.as_ref().ok_or_else(|| ...)?;
    
    // 使用 Provider 的 base_url 构建上游地址
    let upstream_addr = format!("{}:443", provider_config.base_url);
    
    // 创建一个启用 TLS 的上游对等点
    let peer = HttpPeer::new(upstream_addr, true, provider_config.base_url.clone());
    
    Ok(Box::new(peer))
}
```

### 2.4.4 请求转发透明性

**保持完整透明**:
- 不修改请求 URI 和参数
- 不解析请求体内容
- 保持大部分原始头信息
- 仅添加或修改必要的代理头信息

**修改/添加的头信息**:
```
X-Forwarded-For: <client-ip>
X-Request-ID: <uuid>
Authorization: <upstream-api-key>  // 关键：替换用户密钥为上游密钥
Host: <provider-base-url>         // 关键：确保 Host 头正确
User-Agent: AI-Proxy/1.0
```

### 2.4.5 性能优化设计

**避免的重负载操作**:
- ❌ 复杂的 URI 解析和路由匹配
- ❌ 请求体内容解析和修改 (只在统计阶段对响应体进行)
- ❌ 同步数据库查询 (所有 DB 操作都是异步的)

**采用的高性能策略**:
- ✅ 简单字符串前缀匹配
- ✅ Redis 缓存认证结果、健康状态、配置信息
- ✅ 异步非阻塞 I/O
- ✅ 数据库和上游连接池复用

---

## 3. 数据库详细设计

### 3.1 数据库架构

**主存储：SQLite**
- 存储所有持久化数据
- 使用WAL模式提高并发性能
- 定期备份和恢复机制

**缓存层：Redis**
- 存储临时数据和热点数据
- API健康状态缓存
- 负载均衡状态缓存
- 统计数据缓存

### 3.2 数据表设计

#### 3.2.1 用户管理表

```sql
-- 用户基础信息表
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL, -- bcrypt hash
    salt VARCHAR(32) NOT NULL,           -- 密码盐值
    is_active BOOLEAN DEFAULT TRUE,
    is_admin BOOLEAN DEFAULT FALSE,
    last_login DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 用户会话管理表
CREATE TABLE user_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    token_hash VARCHAR(255) NOT NULL,    -- JWT token hash
    refresh_token_hash VARCHAR(255),     -- 刷新token hash
    expires_at DATETIME NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- 用户操作日志表
CREATE TABLE user_audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    action VARCHAR(50) NOT NULL,         -- LOGIN, CREATE_API, DELETE_KEY等
    resource_type VARCHAR(50),           -- USER, API, KEY等
    resource_id INTEGER,
    ip_address VARCHAR(45),
    user_agent TEXT,
    details JSON,                        -- 额外详情信息
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

#### 3.2.2 AI服务商配置表

```sql
-- AI服务提供商类型表
CREATE TABLE provider_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(50) UNIQUE NOT NULL,    -- 'openai', 'gemini', 'claude'
    display_name VARCHAR(100) NOT NULL,   -- 'OpenAI ChatGPT', 'Google Gemini'
    base_url VARCHAR(255) NOT NULL,       -- 'api.openai.com'
    api_format VARCHAR(50) NOT NULL,      -- 'openai', 'gemini_rest', 'anthropic'
    default_model VARCHAR(100),           -- 默认模型名称
    max_tokens INTEGER DEFAULT 4096,     -- 最大token数
    rate_limit INTEGER DEFAULT 100,      -- 每分钟请求限制
    timeout_seconds INTEGER DEFAULT 30,   -- 超时时间
    health_check_path VARCHAR(255) DEFAULT '/models', -- 健康检查路径
    -- auth_header_format功能已合并到config_json中
    is_active BOOLEAN DEFAULT TRUE,
    config_json JSON,                     -- 额外配置信息
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 初始化数据
INSERT INTO provider_types (name, display_name, base_url, api_format, default_model) VALUES
('openai', 'OpenAI ChatGPT', 'api.openai.com', 'openai', 'gpt-3.5-turbo'),
('gemini', 'Google Gemini', 'generativelanguage.googleapis.com', 'gemini_rest', 'gemini-pro'),
('claude', 'Anthropic Claude', 'api.anthropic.com', 'anthropic', 'claude-3-sonnet');
```

#### 3.2.3 用户API密钥管理表

```sql
-- 用户的内部代理商API密钥池（号池）
CREATE TABLE user_provider_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    provider_type_id INTEGER NOT NULL,
    api_key VARCHAR(255) NOT NULL,       -- 实际的AI服务商API密钥
    name VARCHAR(100) NOT NULL,          -- 用户给这个密钥起的名字
    weight INTEGER DEFAULT 1,            -- 权重（用于权重调度）
    max_requests_per_minute INTEGER DEFAULT 100, -- 每分钟最大请求数
    max_tokens_per_day INTEGER DEFAULT 1000000,  -- 每天最大token数
    used_tokens_today INTEGER DEFAULT 0,         -- 今日已使用token数
    last_used DATETIME,                  -- 最后使用时间
    is_active BOOLEAN DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    UNIQUE(user_id, provider_type_id, name), -- 同一用户同一服务商下名称不能重复
    INDEX idx_user_provider (user_id, provider_type_id),
    INDEX idx_active_keys (is_active, provider_type_id)
);

-- 用户对外服务API密钥（每个provider类型只能有一个）
CREATE TABLE user_service_apis (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    provider_type_id INTEGER NOT NULL,
    api_key VARCHAR(64) UNIQUE NOT NULL, -- 我们生成的32字节hex编码API密钥
    api_secret VARCHAR(64) NOT NULL,     -- API密钥对应的secret（用于签名验证）
    name VARCHAR(100),                   -- API名称
    description TEXT,                    -- API描述
    scheduling_strategy VARCHAR(20) DEFAULT 'round_robin', -- 调度策略
    retry_count INTEGER DEFAULT 3,       -- 失败重试次数
    timeout_seconds INTEGER DEFAULT 30,  -- 超时时间
    rate_limit INTEGER DEFAULT 1000,     -- 每分钟请求限制
    max_tokens_per_day INTEGER DEFAULT 10000000, -- 每天最大token限制
    used_tokens_today INTEGER DEFAULT 0,         -- 今日已使用token
    total_requests INTEGER DEFAULT 0,            -- 总请求数
    successful_requests INTEGER DEFAULT 0,       -- 成功请求数
    last_used DATETIME,                  -- 最后使用时间
    expires_at DATETIME,                 -- 过期时间（可选）
    is_active BOOLEAN DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    UNIQUE(user_id, provider_type_id),   -- 每个用户每种服务商只能有一个对外API
    INDEX idx_api_key (api_key),
    INDEX idx_user_provider_service (user_id, provider_type_id)
);
```

#### 3.2.4 监控统计表

```sql
-- API健康状态表
CREATE TABLE api_health_status (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_provider_key_id INTEGER NOT NULL,
    is_healthy BOOLEAN DEFAULT TRUE,
    response_time_ms INTEGER DEFAULT 0,   -- 平均响应时间
    success_rate REAL DEFAULT 1.0,       -- 成功率 (0.0-1.0)
    last_success DATETIME,               -- 最后成功时间
    last_failure DATETIME,               -- 最后失败时间
    consecutive_failures INTEGER DEFAULT 0, -- 连续失败次数
    total_checks INTEGER DEFAULT 0,      -- 总检查次数
    successful_checks INTEGER DEFAULT 0,  -- 成功检查次数
    last_error_message TEXT,             -- 最后错误信息
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_provider_key_id) REFERENCES user_provider_keys(id) ON DELETE CASCADE,
    INDEX idx_health_status (user_provider_key_id, is_healthy),
    INDEX idx_last_check (updated_at)
);

-- 请求统计表 (在 v2.0 中被 proxy_tracing 表替代)
-- CREATE TABLE request_statistics (...)

-- 统一追踪表 (v2.0)
CREATE TABLE proxy_tracing (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id VARCHAR(36) UNIQUE NOT NULL,
    user_service_api_id INTEGER,
    user_provider_key_id INTEGER,
    -- ... 更多追踪字段
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);


-- 每日统计汇总表（用于快速查询）
CREATE TABLE daily_statistics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    user_service_api_id INTEGER,
    provider_type_id INTEGER NOT NULL,
    date DATE NOT NULL,                  -- 统计日期
    total_requests INTEGER DEFAULT 0,    -- 总请求数
    successful_requests INTEGER DEFAULT 0, -- 成功请求数
    failed_requests INTEGER DEFAULT 0,   -- 失败请求数
    total_tokens INTEGER DEFAULT 0,      -- 总token使用
    avg_response_time INTEGER DEFAULT 0, -- 平均响应时间
    max_response_time INTEGER DEFAULT 0, -- 最大响应时间
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (user_service_api_id) REFERENCES user_service_apis(id),
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    UNIQUE(user_id, user_service_api_id, provider_type_id, date),
    INDEX idx_user_date (user_id, date),
    INDEX idx_service_date (user_service_api_id, date)
);
```

#### 3.2.5 系统配置表

```sql
-- TLS证书管理表
CREATE TABLE tls_certificates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain VARCHAR(255) UNIQUE NOT NULL,
    cert_type VARCHAR(20) DEFAULT 'acme', -- 'acme', 'self_signed', 'custom'
    cert_pem TEXT NOT NULL,              -- 证书内容
    key_pem TEXT NOT NULL,               -- 私钥内容
    chain_pem TEXT,                      -- 证书链
    is_auto_renew BOOLEAN DEFAULT TRUE,  -- 是否自动续期
    renew_before_days INTEGER DEFAULT 30, -- 提前多少天续期
    expires_at DATETIME NOT NULL,        -- 过期时间
    last_renewed DATETIME,               -- 最后续期时间
    acme_account_url VARCHAR(500),       -- ACME账户URL
    acme_order_url VARCHAR(500),         -- ACME订单URL
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_domain (domain),
    INDEX idx_expires (expires_at, is_auto_renew)
);

-- 系统配置表
CREATE TABLE system_configurations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key VARCHAR(100) UNIQUE NOT NULL,
    value TEXT NOT NULL,
    description TEXT,
    is_encrypted BOOLEAN DEFAULT FALSE,  -- 值是否加密存储
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 初始化系统配置
INSERT INTO system_configurations (key, value, description) VALUES
('max_concurrent_requests', '10000', '最大并发请求数'),
('default_timeout', '30', '默认超时时间（秒）'),
('health_check_interval', '30', '健康检查间隔（秒）'),
('statistics_retention_days', '90', '统计数据保留天数'),
('log_level', 'info', '日志级别'),
('enable_request_logging', 'true', '是否启用请求日志');
```

### 3.3 数据库优化策略

**索引优化**
- 为常用查询字段创建复合索引
- 定期分析查询计划，优化慢查询
- 使用部分索引减少索引大小

**分区策略**
- 按时间分区存储统计数据
- 定期清理过期数据
- 实现数据归档机制

**连接池配置**
```rust
// 数据库连接池配置
DatabaseConfig {
    max_connections: 20,
    min_connections: 5,
    connect_timeout: Duration::from_secs(10),
    idle_timeout: Some(Duration::from_secs(600)),
    max_lifetime: Some(Duration::from_secs(3600)),
}
```

---

## 4. 核心模块详细设计

### 4.1 项目结构设计

```
ai-proxy-system/
├── Cargo.toml                     # Rust项目配置
├── src/                          # 主应用源码
│   ├── main.rs                   # 程序入口
│   ├── lib.rs                    # 库入口
│   ├── config/                   # 配置管理
│   ├── auth/                     # 认证授权 (API Key, JWT, OAuth2)
│   ├── proxy/                    # Pingora代理服务 (service.rs, builder.rs)
│   ├── management/               # Axum管理API
│   ├── scheduler/                # 负载均衡调度 (pool_manager.rs, algorithms.rs)
│   ├── health/                   # 健康检查 (api_key_health.rs)
│   ├── statistics/               # 统计监控 (service.rs)
│   ├── trace/                    # 请求追踪 (immediate.rs)
│   ├── providers/                # AI服务商适配
│   ├── cache/                    # Redis缓存
│   ├── utils/                    # 工具函数
│   └── error/                    # 错误处理
├── entity/                        # Sea-ORM实体定义
├── migration/                     # 数据库迁移文件
├── web/                           # React 前端应用
│   ├── package.json
│   ├── scripts/
│   │   └── build.mjs             # ESBuild 构建脚本
│   ├── src/
│   └── ...
├── config/                       # 配置文件
└── docs/                         # 文档
```

### 4.2 Pingora统一入口服务

#### 4.2.1 主服务实现

```rust
// src/main.rs & src/dual_port_setup.rs
async fn run_dual_port_servers() -> Result<()> {
    // 1. 初始化日志
    // 2. 加载配置
    // 3. 初始化所有共享服务 (数据库, 缓存, 认证, 调度器, 健康检查器等)
    let shared_services = initialize_shared_services().await?;

    // 4. 并发启动两个服务
    tokio::select! {
        _ = ManagementServer::serve(shared_services.clone()) => {}, // Axum on :9090
        _ = PingoraProxyServer::start(shared_services.clone()) => {},   // Pingora on :8080
    }
    Ok(())
}

// src/dual_port_setup.rs
async fn initialize_shared_services() -> Result<SharedServices> {
    // ...
    let auth_manager = AuthManager::new(...);
    let health_checker = ApiKeyHealthChecker::new(...);
    let pool_manager = ApiKeyPoolManager::new(..., health_checker.clone());
    // ...
    Ok(SharedServices { ... })
}
```

#### 4.2.2 统一代理服务

```rust
// src/proxy/service.rs
pub struct ProxyService {
    // 依赖注入所有需要的共享服务
    auth_service: Arc<AuthenticationService>,
    tracing_service: Arc<TracingService>,
    // ...
}

#[async_trait]
impl ProxyHttp for ProxyService {
    type CTX = ProxyContext;
    fn new_ctx(&self) -> Self::CTX { /* ... */ }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        // 实际实现见 2.4.1 中的 Pipeline 示例
        // ...
    }

    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        // ...
    }

    async fn upstream_request_filter(&self, ..., ctx: &mut Self::CTX) -> Result<()> {
        // ...
    }

    async fn logging(&self, ..., ctx: &mut Self::CTX) {
        // 最终完成追踪和统计
        self.tracing_service.complete_trace(ctx).await;
    }
}
```

### 4.3 AI代理处理器

(逻辑被分解到 `AuthenticationService`, `ApiKeySelectionService` 等多个服务中，由 `ProxyService` 协调)

### 4.4 负载均衡调度器

```rust
// src/scheduler/pool_manager.rs
pub struct ApiKeyPoolManager {
    db: DatabaseConnection,
    cache: UnifiedCacheManager,
    health_checker: Arc<ApiKeyHealthChecker>,
}

impl ApiKeyPoolManager {
    pub async fn select_api_key_from_service_api(
        &self,
        service_api: &UserServiceApi,
    ) -> Result<ApiKeySelectionResult> {
        // 1. 从 service_api.user_provider_keys_ids 获取密钥ID列表
        // 2. 从数据库批量查询这些密钥的详情
        let candidate_keys = self.get_candidate_keys(...).await?;

        // 3. 使用健康检查器过滤掉不健康的密钥
        let healthy_keys = self.health_checker.filter_healthy_keys(candidate_keys).await;

        // 4. 根据 service_api.scheduling_strategy 选择调度算法
        let selector = ApiKeySelector::from_strategy(&service_api.scheduling_strategy);

        // 5. 执行算法选出最终密钥
        let selected_key = selector.select(healthy_keys)?;

        Ok(ApiKeySelectionResult { ... })
    }
}

// src/scheduler/algorithms.rs
enum SchedulingStrategy { RoundRobin, Weighted, HealthBest }

trait ApiKeySelector {
    fn select(&self, keys: Vec<ProviderKey>) -> Result<ProviderKey>;
}
```

### 4.5 健康检查模块

```rust
// src/scheduler/api_key_health.rs
pub struct ApiKeyHealthChecker {
    db: DatabaseConnection,
    cache: UnifiedCacheManager,
}

impl ApiKeyHealthChecker {
    // 被动健康检查：根据请求成功/失败更新状态
    pub async fn update_health_status_on_request_outcome(&self, key_id: i32, outcome: RequestOutcome) {
        // ... 更新数据库和缓存中的健康状态、响应时间、错误率等
    }

    // 主动健康检查：后台任务
    pub async fn run_background_checks(&self) {
        // 定期查询所有密钥
        // 对每个密钥执行一次轻量级的测试请求 (e.g., GET /models)
        // 根据结果更新健康状态
    }

    // 过滤健康密钥供调度器使用
    pub async fn filter_healthy_keys(&self, keys: Vec<ProviderKey>) -> Vec<ProviderKey> {
        // ... 从缓存或数据库获取健康状态，返回健康的密钥列表
    }
}
```

// 健康密钥结构
#[derive(Debug, Clone)]
pub struct HealthyKey {
    pub id: i32,
    pub api_key: String,
    pub name: String,
    pub weight: i32,
    pub response_time_ms: i32,
    pub success_rate: f64,
}

impl Clone for HealthChecker {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            redis: self.redis.clone(),
        }
    }
}
```

### 4.6 管理API模块 (Axum内嵌服务)

```rust
// src/management/mod.rs
use axum::{
    extract::{Request, State}, 
    response::Response, 
    Router, 
    http::StatusCode,
};
use hyper::body::Incoming;
use std::sync::Arc;
use crate::{AppState, proxy::ProxyContext, error::ProxyError};

pub struct ManagementService {
    router: Router,
    app_state: Arc<AppState>,
}

impl ManagementService {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let router = create_management_router(app_state.clone());
        
        Self {
            router,
            app_state,
        }
    }

    pub async fn handle_request(
        &self,
        session: &mut pingora::prelude::Session,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 将Pingora Session转换为Hyper Request
        let hyper_request = self.session_to_hyper_request(session).await?;
        
        // 处理请求
        let response = self.router.clone()
            .oneshot(hyper_request)
            .await
            .map_err(|e| ProxyError::Internal(format!("Axum router error: {}", e)))?;

        // 将响应写回Session
        self.write_response_to_session(session, response).await?;

        Ok(())
    }

    async fn session_to_hyper_request(
        &self,
        session: &pingora::prelude::Session,
    ) -> Result<Request<Incoming>, ProxyError> {
        use hyper::{Method, Request, Uri, HeaderMap};
        use http_body_util::Full;
        use bytes::Bytes;

        let method = session.req_header().method.clone();
        let uri = session.req_header().uri.clone();
        
        // 复制请求头
        let mut headers = HeaderMap::new();
        for (name, value) in session.req_header().headers.iter() {
            headers.insert(name.clone(), value.clone());
        }

        // 读取请求体
        let body = if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
            let mut body_bytes = Vec::new();
            session.read_request_body(&mut body_bytes).await
                .map_err(|e| ProxyError::Internal(format!("Failed to read request body: {}", e)))?;
            Full::new(Bytes::from(body_bytes))
        } else {
            Full::new(Bytes::new())
        };

        let mut request = Request::builder()
            .method(method)
            .uri(uri);

        // 设置请求头
        *request.headers_mut().unwrap() = headers;

        request.body(body)
            .map_err(|e| ProxyError::Internal(format!("Failed to build request: {}", e)))
    }

    async fn write_response_to_session(
        &self,
        session: &mut pingora::prelude::Session,
        response: Response,
    ) -> Result<(), ProxyError> {
        use http_body_util::BodyExt;

        let (parts, body) = response.into_parts();

        // 设置响应状态
        session.set_response_status(parts.status)
            .map_err(|e| ProxyError::Internal(format!("Failed to set response status: {}", e)))?;

        // 设置响应头
        for (name, value) in parts.headers.iter() {
            session.insert_header(name.as_str(), value.as_bytes())
                .map_err(|e| ProxyError::Internal(format!("Failed to set response header: {}", e)))?;
        }

        // 读取并写入响应体
        let body_bytes = body.collect().await
            .map_err(|e| ProxyError::Internal(format!("Failed to collect response body: {}", e)))?
            .to_bytes();

        session.write_response_body(Some(body_bytes)).await
            .map_err(|e| ProxyError::Internal(format!("Failed to write response body: {}", e)))?;

        Ok(())
    }
}

// 创建管理路由器
pub fn create_management_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", api_routes())
        .nest("/admin", admin_routes())
        .fallback(serve_frontend)
        .with_state(app_state)
        .layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(tower_http::cors::CorsLayer::permissive())
                .layer(tower_http::compression::CompressionLayer::new())
        )
}

// API路由
fn api_routes() -> Router<Arc<AppState>> {
    use crate::management::routes::*;
    
    Router::new()
        .nest("/auth", auth::routes())
        .nest("/users", users::routes())
        .nest("/providers", providers::routes())
        .nest("/apis", apis::routes())
        .nest("/keys", keys::routes())
        .nest("/statistics", statistics::routes())
        .nest("/health", health::routes())
}

// 管理员路由
fn admin_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system", axum::routing::get(admin_system_info))
        .route("/logs", axum::routing::get(admin_logs))
        .layer(crate::management::middleware::admin_auth::AdminAuthLayer)
}

// 前端文件服务
async fn serve_frontend(uri: axum::http::Uri) -> Result<Response, StatusCode> {
    static_file_service::serve_static_file(uri.path()).await
}

async fn admin_system_info(
    State(app_state): State<Arc<AppState>>,
) -> Result<axum::Json<serde_json::Value>, StatusCode> {
    // 返回系统信息
    let info = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": "TODO", // 计算运行时间
        "connections": "TODO", // 当前连接数
        "memory_usage": "TODO", // 内存使用情况
    });

    Ok(axum::Json(info))
}

async fn admin_logs(
    State(app_state): State<Arc<AppState>>,
) -> Result<String, StatusCode> {
    // 返回最近的日志
    Ok("Recent logs would be here".to_string())
}

// 静态文件服务模块
mod static_file_service {
    use axum::response::{Response, Html};
    use axum::http::{StatusCode, HeaderMap, HeaderValue};
    use std::path::Path;

    pub async fn serve_static_file(path: &str) -> Result<Response, StatusCode> {
        let path = path.trim_start_matches('/');
        let file_path = if path.is_empty() || path == "index.html" {
            "frontend/dist/index.html"
        } else {
            &format!("frontend/dist/{}", path)
        };

        // 检查文件是否存在
        if !Path::new(file_path).exists() {
            // 对于SPA应用，所有非API路由都返回index.html
            if !path.starts_with("api/") {
                return serve_static_file("index.html").await;
            }
            return Err(StatusCode::NOT_FOUND);
        }

        // 读取文件内容
        let content = tokio::fs::read(file_path).await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // 确定Content-Type
        let content_type = match Path::new(file_path).extension().and_then(|ext| ext.to_str()) {
            Some("html") => "text/html",
            Some("css") => "text/css",
            Some("js") => "application/javascript",
            Some("json") => "application/json",
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("svg") => "image/svg+xml",
            Some("ico") => "image/x-icon",
            _ => "application/octet-stream",
        };

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static(content_type));

        Ok(Response::builder()
            .status(StatusCode::OK)
            .headers(headers)
            .body(content.into())
            .unwrap())
    }
}
```

### 4.7 TLS证书管理模块

```rust
// src/tls/mod.rs
use std::sync::Arc;
use tokio::time::{interval, Duration};
use rustls::{Certificate, PrivateKey, ServerConfig};
use crate::{AppState, error::ProxyError};

pub struct TlsManager {
    app_state: Arc<AppState>,
    config: Arc<crate::config::TlsConfig>,
}

impl TlsManager {
    pub fn new(app_state: Arc<AppState>, config: Arc<crate::config::AppConfig>) -> Self {
        Self {
            app_state,
            config: config.tls.clone(),
        }
    }

    // 启动证书续期任务
    pub async fn start_renewal_task(&self) {
        let mut interval = interval(Duration::from_secs(3600)); // 每小时检查一次
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_and_renew_certificates().await {
                lerror!(
                    "tls",
                    LogStage::Renewal,
                    LogComponent::TlsManager,
                    "certificate_renewal_check_failed",
                    &format!("Certificate renewal check failed: {}", e)
                );
            }
        }
    }

    // 检查并续期即将过期的证书
    async fn check_and_renew_certificates(&self) -> Result<(), ProxyError> {
        use entity::tls_certificates::{Entity as TlsCertificates, Column};
        use sea_orm::{EntityTrait, QueryFilter};

        let expiring_certs = TlsCertificates::find()
            .filter(Column::IsAutoRenew.eq(true))
            .filter(Column::ExpiresAt.lt(chrono::Utc::now().naive_utc() + chrono::Duration::days(30)))
            .all(&*self.app_state.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?;

        linfo!(
            "tls",
            LogStage::Renewal,
            LogComponent::TlsManager,
            "found_certificates_to_renew",
            &format!("Found {} certificates to renew", expiring_certs.len())
        );

        for cert in expiring_certs {
            if let Err(e) = self.renew_certificate(&cert).await {
                lerror!(
                    "tls",
                    LogStage::Renewal,
                    LogComponent::TlsManager,
                    "failed_to_renew_certificate",
                    &format!("Failed to renew certificate for domain {}: {}", cert.domain, e)
                );            }
        }

        Ok(())
    }

    // 续期单个证书
    async fn renew_certificate(&self, cert: &entity::tls_certificates::Model) -> Result<(), ProxyError> {
        linfo!(
            "tls",
            LogStage::Renewal,
            LogComponent::TlsManager,
            "renewing_certificate",
            &format!("Renewing certificate for domain {}", cert.domain)
        );

        match cert.cert_type.as_str() {
            "acme" => self.renew_acme_certificate(cert).await,
            "self_signed" => self.renew_self_signed_certificate(cert).await,
            _ => {
                lwarn!(
                    "tls",
                    LogStage::Renewal,
                    LogComponent::TlsManager,
                    "cannot_auto_renew_certificate",
                    &format!(
                        "Cannot auto-renew certificate of type {} for domain {}",
                        cert.cert_type, cert.domain
                    )
                );                Ok(())
            }
        }
    }

    // 续期ACME证书
    async fn renew_acme_certificate(&self, cert: &entity::tls_certificates::Model) -> Result<(), ProxyError> {
        use acme_lib::{create_p256_key, Account, AuthorizeOrder, Csr, Directory, DirectoryUrl};
        use entity::tls_certificates::{ActiveModel, Column};
        use sea_orm::{ActiveModelTrait, Set, EntityTrait};

        // 创建ACME账户
        let directory = Directory::from_url(DirectoryUrl::LetsEncrypt)
            .map_err(|e| ProxyError::Internal(format!("Failed to create ACME directory: {}", e)))?;

        let account = Account::create(&directory, &self.config.acme_email, create_p256_key()?)
            .map_err(|e| ProxyError::Internal(format!("Failed to create ACME account: {}", e)))?;

        // 创建证书签名请求
        let private_key = create_p256_key()?;
        let csr = Csr::new(&private_key, &[cert.domain.clone()])
            .map_err(|e| ProxyError::Internal(format!("Failed to create CSR: {}", e)))?;

        // 申请证书
        let order = account.new_order(&cert.domain, &[])
            .map_err(|e| ProxyError::Internal(format!("Failed to create ACME order: {}", e)))?;

        let order = order.confirm_validations()
            .map_err(|e| ProxyError::Internal(format!("Failed to confirm ACME validations: {}", e)))?;

        let cert_chain = order.download_cert()
            .map_err(|e| ProxyError::Internal(format!("Failed to download certificate: {}", e)))?;

        // 解析证书获取过期时间
        let expires_at = self.parse_cert_expiry(&cert_chain.certificate())?;

        // 更新数据库中的证书
        let mut cert_model: ActiveModel = cert.clone().into();
        cert_model.cert_pem = Set(cert_chain.certificate().to_string());
        cert_model.key_pem = Set(private_key.private_key_pem());
        cert_model.chain_pem = Set(cert_chain.ca_chain().map(|chain| chain.to_string()));
        cert_model.expires_at = Set(expires_at);
        cert_model.last_renewed = Set(Some(chrono::Utc::now().naive_utc()));
        cert_model.updated_at = Set(chrono::Utc::now().naive_utc());

        cert_model.update(&*self.app_state.db).await
            .map_err(|e| ProxyError::Internal(format!("Failed to update certificate: {}", e)))?;

        linfo!(
            "tls",
            LogStage::Renewal,
            LogComponent::TlsManager,
            "certificate_renewed_successfully",
            &format!(
                "Certificate renewed successfully for domain {}: expires at {}",
                cert.domain, expires_at
            )
        );

        Ok(())
    }

    // 续期自签名证书
    async fn renew_self_signed_certificate(&self, cert: &entity::tls_certificates::Model) -> Result<(), ProxyError> {
        use rcgen::{Certificate, CertificateParams, DnType};
        use entity::tls_certificates::ActiveModel;
        use sea_orm::{ActiveModelTrait, Set};

        // 生成新的自签名证书
        let mut params = CertificateParams::new(vec![cert.domain.clone()]);
        params.distinguished_name.push(DnType::CommonName, cert.domain.clone());
        
        let certificate = Certificate::from_params(params)
            .map_err(|e| ProxyError::Internal(format!("Failed to generate certificate: {}", e)))?;

        let cert_pem = certificate.serialize_pem()
            .map_err(|e| ProxyError::Internal(format!("Failed to serialize certificate: {}", e)))?;
        
        let key_pem = certificate.serialize_private_key_pem();
        
        // 设置过期时间为1年后
        let expires_at = chrono::Utc::now().naive_utc() + chrono::Duration::days(365);

        // 更新数据库
        let mut cert_model: ActiveModel = cert.clone().into();
        cert_model.cert_pem = Set(cert_pem);
        cert_model.key_pem = Set(key_pem);
        cert_model.expires_at = Set(expires_at);
        cert_model.last_renewed = Set(Some(chrono::Utc::now().naive_utc()));
        cert_model.updated_at = Set(chrono::Utc::now().naive_utc());

        cert_model.update(&*self.app_state.db).await
            .map_err(|e| ProxyError::Internal(format!("Failed to update certificate: {}", e)))?;

        tracing::info!(
            domain = %cert.domain,
            expires_at = %expires_at,
            "Self-signed certificate renewed"
        );

        Ok(())
    }

    // 解析证书过期时间
    fn parse_cert_expiry(&self, cert_pem: &str) -> Result<chrono::NaiveDateTime, ProxyError> {
        use x509_parser::{certificate::X509Certificate, pem::parse_x509_pem};

        let pem = parse_x509_pem(cert_pem.as_bytes())
            .map_err(|e| ProxyError::Internal(format!("Failed to parse PEM: {:?}", e)))?;

        let cert = X509Certificate::from_der(pem.1.contents.as_slice())
            .map_err(|e| ProxyError::Internal(format!("Failed to parse certificate: {:?}", e)))?;

        let not_after = cert.1.validity().not_after;
        let timestamp = not_after.timestamp();
        
        Ok(chrono::NaiveDateTime::from_timestamp(timestamp, 0)
            .ok_or_else(|| ProxyError::Internal("Invalid certificate expiry time".into()))?)
    }
}

// TLS回调函数
pub fn create_tls_callback(
    app_state: Arc<AppState>,
) -> Box<dyn Fn(&mut pingora::tls::TlsAccept) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync> {
    Box::new(move |tls_accept| {
        let sni = tls_accept.server_name().unwrap_or("default");
        let server_config = get_server_config_for_domain(&app_state, sni)?;
        tls_accept.set_server_config(server_config);
        Ok(())
    })
}

// 获取域名对应的TLS配置
fn get_server_config_for_domain(
    app_state: &AppState,
    domain: &str,
) -> Result<Arc<ServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    use entity::tls_certificates::{Entity as TlsCertificates, Column};
    use sea_orm::{EntityTrait, QueryFilter};

    // 这里需要同步查询，因为TLS回调不能是异步的
    // 在实际实现中，应该预先加载证书到内存中
    let rt = tokio::runtime::Handle::current();
    let cert = rt.block_on(async {
        TlsCertificates::find()
            .filter(Column::Domain.eq(domain))
            .one(&*app_state.db)
            .await
    })?;

    let cert = cert.ok_or("Certificate not found for domain")?;

    // 解析证书和私钥
    let cert_chain: Vec<Certificate> = rustls_pemfile::certs(&mut cert.cert_pem.as_bytes())?
        .into_iter()
        .map(Certificate)
        .collect();

    let mut key_reader = cert.key_pem.as_bytes();
    let private_key = if let Some(key) = rustls_pemfile::pkcs8_private_keys(&mut key_reader)?.into_iter().next() {
        PrivateKey(key)
    } else {
        let mut key_reader = cert.key_pem.as_bytes();
        if let Some(key) = rustls_pemfile::rsa_private_keys(&mut key_reader)?.into_iter().next() {
            PrivateKey(key)
        } else {
            return Err("No private key found".into());
        }
    };

    // 构建TLS配置
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;

    Ok(Arc::new(config))
}
```

---

## 5. API接口详细设计

### 5.1 RESTful API规范

**基础URL**: `https://your-domain.com/api/v1`

**认证方式**: Bearer Token (JWT)

**响应格式**: JSON

**错误处理**: 统一错误响应格式

```json
{
  "error": {
    "code": "AUTH_REQUIRED",
    "message": "Authentication token is required",
    "details": {},
    "timestamp": "2024-01-01T00:00:00Z",
    "request_id": "uuid"
  }
}
```

### 5.2 认证相关API

```yaml
# 用户注册
POST /api/auth/register
Content-Type: application/json

Request:
{
  "username": "string",
  "email": "string", 
  "password": "string"
}

Response:
{
  "data": {
    "user_id": 1,
    "username": "string",
    "email": "string",
    "created_at": "2024-01-01T00:00:00Z"
  }
}

---

# 用户登录
POST /api/auth/login
Content-Type: application/json

Request:
{
  "email": "string",
  "password": "string"
}

Response:
{
  "data": {
    "access_token": "jwt_token",
    "refresh_token": "refresh_token",
    "expires_at": "2024-01-01T01:00:00Z",
    "user": {
      "id": 1,
      "username": "string",
      "email": "string"
    }
  }
}

---

# 刷新Token
POST /api/auth/refresh
Authorization: Bearer refresh_token

Response:
{
  "data": {
    "access_token": "new_jwt_token",
    "expires_at": "2024-01-01T01:00:00Z"
  }
}

---

# 用户登出
POST /api/auth/logout
Authorization: Bearer access_token

Response:
{
  "message": "Logged out successfully"
}
```

### 5.3 用户API密钥管理

```yaml
# 获取用户的服务API列表
GET /api/apis
Authorization: Bearer access_token

Response:
{
  "data": [
    {
      "id": 1,
      "provider_type": {
        "id": 1,
        "name": "openai",
        "display_name": "OpenAI ChatGPT"
      },
      "api_key": "proxy_xxxxxxxxxxxx",
      "name": "My OpenAI API",
      "scheduling_strategy": "round_robin",
      "retry_count": 3,
      "timeout_seconds": 30,
      "rate_limit": 1000,
      "is_active": true,
      "created_at": "2024-01-01T00:00:00Z",
      "statistics": {
        "total_requests": 1000,
        "successful_requests": 950,
        "failed_requests": 50,
        "avg_response_time": 1200
      }
    }
  ]
}

---

# 创建用户服务API
POST /api/apis
Authorization: Bearer access_token
Content-Type: application/json

Request:
{
  "provider_type_id": 1,
  "name": "My OpenAI API",
  "description": "Personal OpenAI API for testing",
  "scheduling_strategy": "round_robin",
  "retry_count": 3,
  "timeout_seconds": 30,
  "rate_limit": 1000
}

Response:
{
  "data": {
    "id": 1,
    "api_key": "proxy_xxxxxxxxxxxx",
    "api_secret": "secret_xxxxxxxxxxxx",
    "provider_type_id": 1,
    "name": "My OpenAI API",
    "scheduling_strategy": "round_robin",
    "is_active": true,
    "created_at": "2024-01-01T00:00:00Z"
  }
}

---

# 更新用户服务API配置
PUT /api/apis/{api_id}
Authorization: Bearer access_token
Content-Type: application/json

Request:
{
  "name": "Updated API Name",
  "scheduling_strategy": "weighted",
  "retry_count": 5,
  "rate_limit": 2000
}

Response:
{
  "data": {
    "id": 1,
    "name": "Updated API Name",
    "scheduling_strategy": "weighted",
    "retry_count": 5,
    "rate_limit": 2000,
    "updated_at": "2024-01-01T00:00:00Z"
  }
}
```

### 5.4 提供商密钥池管理

```yaml
# 获取提供商密钥池列表
GET /api/apis/{api_id}/keys
Authorization: Bearer access_token

Response:
{
  "data": [
    {
      "id": 1,
      "name": "OpenAI Key 1",
      "weight": 10,
      "is_active": true,
      "health_status": {
        "is_healthy": true,
        "response_time_ms": 850,
        "success_rate": 0.98,
        "last_check": "2024-01-01T00:00:00Z"
      },
      "created_at": "2024-01-01T00:00:00Z"
    }
  ]
}

---

# 添加提供商API密钥
POST /api/apis/{api_id}/keys
Authorization: Bearer access_token
Content-Type: application/json

Request:
{
  "api_key": "sk-xxxxxxxxxxxx",
  "name": "OpenAI Key 2",
  "weight": 5,
  "max_requests_per_minute": 100,
  "max_tokens_per_day": 100000
}

Response:
{
  "data": {
    "id": 2,
    "name": "OpenAI Key 2", 
    "weight": 5,
    "is_active": true,
    "created_at": "2024-01-01T00:00:00Z"
  }
}

---

# 更新提供商API密钥
PUT /api/keys/{key_id}
Authorization: Bearer access_token
Content-Type: application/json

Request:
{
  "name": "Updated Key Name",
  "weight": 8,
  "is_active": false
}

Response:
{
  "data": {
    "id": 2,
    "name": "Updated Key Name",
    "weight": 8,
    "is_active": false,
    "updated_at": "2024-01-01T00:00:00Z"
  }
}

---

# 删除提供商API密钥
DELETE /api/keys/{key_id}
Authorization: Bearer access_token

Response:
{
  "message": "API key deleted successfully"
}
```

### 5.5 统计和监控API

```yaml
# 获取请求统计
GET /api/statistics/requests
Authorization: Bearer access_token
Query Parameters:
  - api_id: integer (optional)
  - start_date: date (optional)
  - end_date: date (optional) 
  - group_by: enum[hour,day,week,month] (default: day)

Response:
{
  "data": {
    "summary": {
      "total_requests": 10000,
      "successful_requests": 9500,
      "failed_requests": 500,
      "avg_response_time": 1200,
      "total_tokens": 150000
    },
    "time_series": [
      {
        "date": "2024-01-01",
        "requests": 1000,
        "success_rate": 0.95,
        "avg_response_time": 1100,
        "tokens": 15000
      }
    ],
    "provider_breakdown": [
      {
        "provider_type": "openai",
        "requests": 6000,
        "success_rate": 0.96
      }
    ]
  }
}

---

# 获取健康状态监控
GET /api/health/status
Authorization: Bearer access_token
Query Parameters:
  - api_id: integer (optional)

Response:
{
  "data": [
    {
      "api_id": 1,
      "provider_type": "openai",
      "total_keys": 3,
      "healthy_keys": 2,
      "unhealthy_keys": 1,
      "keys": [
        {
          "id": 1,
          "name": "Key 1",
          "is_healthy": true,
          "response_time_ms": 800,
          "success_rate": 0.98,
          "last_check": "2024-01-01T00:00:00Z"
        }
      ]
    }
  ]
}

---

# 获取错误日志
GET /api/logs/errors
Authorization: Bearer access_token
Query Parameters:
  - api_id: integer (optional)
  - start_date: date (optional)
  - end_date: date (optional)
  - limit: integer (default: 100)
  - offset: integer (default: 0)

Response:
{
  "data": {
    "total": 50,
    "errors": [
      {
        "id": 1,
        "request_id": "uuid",
        "api_id": 1,
        "error_type": "TIMEOUT",
        "error_message": "Request timeout after 30 seconds",
        "status_code": 408,
        "path": "/v1/chat/completions",
        "created_at": "2024-01-01T00:00:00Z"
      }
    ]
  }
}
```

### 5.6 AI服务商代理接口

系统需要为每种AI服务商提供兼容的API接口：

```yaml
# OpenAI兼容接口
POST /v1/chat/completions
Authorization: Bearer proxy_xxxxxxxxxxxx
Content-Type: application/json

Request:
{
  "model": "gpt-3.5-turbo",
  "messages": [
    {
      "role": "user", 
      "content": "Hello, how are you?"
    }
  ],
  "temperature": 0.7,
  "max_tokens": 150
}

Response:
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "created": 1677652288,
  "model": "gpt-3.5-turbo",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! I'm doing well, thank you for asking."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 13,
    "completion_tokens": 12,
    "total_tokens": 25
  }
}

---

# Gemini代理接口
POST /v1beta/models/{model}:generateContent
Authorization: Bearer proxy_xxxxxxxxxxxx  
Content-Type: application/json

Request:
{
  "contents": [{
    "parts": [{
      "text": "Hello, how are you?"
    }]
  }],
  "generationConfig": {
    "temperature": 0.7,
    "maxOutputTokens": 150
  }
}

---

# Claude代理接口
POST /v1/messages
Authorization: Bearer proxy_xxxxxxxxxxxx
Content-Type: application/json

Request:
{
  "model": "claude-3-sonnet-20240229",
  "max_tokens": 150,
  "messages": [
    {
      "role": "user",
      "content": "Hello, how are you?"
    }
  ]
}
```

---

## 6. 安全设计

### 6.1 认证安全

**JWT Token管理**
```rust
// JWT配置
pub struct JwtConfig {
    pub secret: String,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration, 
    pub issuer: String,
    pub audience: String,
}

// Token生成
impl JwtService {
    pub fn generate_tokens(&self, user_id: i32) -> Result<TokenPair, AuthError> {
        let now = Utc::now();
        let access_claims = Claims {
            sub: user_id.to_string(),
            exp: (now + self.config.access_token_ttl).timestamp() as usize,
            iat: now.timestamp() as usize,
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
        };

        let refresh_claims = RefreshClaims {
            sub: user_id.to_string(),
            exp: (now + self.config.refresh_token_ttl).timestamp() as usize,
            iat: now.timestamp() as usize,
            token_type: "refresh".to_string(),
        };

        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.config.secret.as_ref()),
        )?;

        let refresh_token = encode(
            &Header::default(), 
            &refresh_claims,
            &EncodingKey::from_secret(self.config.secret.as_ref()),
        )?;

        Ok(TokenPair { access_token, refresh_token })
    }
}
```

**密码安全**
```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng};

pub struct PasswordService;

impl PasswordService {
    pub fn hash_password(password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)?
            .to_string();
            
        Ok(password_hash)
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed_hash = PasswordHash::new(hash)?;
        let argon2 = Argon2::default();
        
        Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
    }
}
```

### 6.2 API密钥安全

**密钥生成**
```rust
use rand::{Rng, thread_rng};
use sha2::{Sha256, Digest};

pub fn generate_api_key() -> String {
    let mut rng = thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    format!("proxy_{}", hex::encode(random_bytes))
}

pub fn generate_api_secret() -> String {
    let mut rng = thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    hex::encode(random_bytes)
}

pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}
```

**密钥验证中间件**
```rust
use axum::{extract::Request, middleware::Next, response::Response};

pub async fn api_key_auth_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let api_key = extract_api_key_from_request(&request)?;
    let user_api = validate_api_key(&api_key).await?;
    
    // 将验证后的用户信息添加到请求扩展中
    request.extensions_mut().insert(user_api);
    
    Ok(next.run(request).await)
}
```



### 6.4 数据安全

**敏感数据加密**
```rust
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, NewAead, generic_array::GenericArray}};

pub struct EncryptionService {
    cipher: Aes256Gcm,
}

impl EncryptionService {
    pub fn new(key: &[u8; 32]) -> Self {
        let key = Key::from_slice(key);
        let cipher = Aes256Gcm::new(key);
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, EncryptionError> {
        let nonce = Nonce::from_slice(b"unique nonce");
        let ciphertext = self.cipher.encrypt(nonce, plaintext.as_bytes())?;
        Ok(base64::encode(ciphertext))
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String, EncryptionError> {
        let nonce = Nonce::from_slice(b"unique nonce");
        let ciphertext = base64::decode(ciphertext)?;
        let plaintext = self.cipher.decrypt(nonce, ciphertext.as_ref())?;
        Ok(String::from_utf8(plaintext)?)
    }
}
```

### 6.5 速率限制

**基于Redis的滑动窗口限流**
```rust
use redis::{Client, Commands};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct RateLimiter {
    redis: Client,
}

impl RateLimiter {
    pub async fn is_allowed(
        &self,
        key: &str,
        limit: u32,
        window: u64,
    ) -> Result<bool, RateLimitError> {
        let mut conn = self.redis.get_connection()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        
        // 使用Lua脚本实现原子操作
        let script = r#"
            local key = KEYS[1]
            local window = tonumber(ARGV[1])
            local limit = tonumber(ARGV[2])
            local now = tonumber(ARGV[3])
            
            -- 清理过期的记录
            redis.call('ZREMRANGEBYSCORE', key, 0, now - window)
            
            -- 获取当前窗口内的请求数
            local current = redis.call('ZCARD', key)
            
            if current < limit then
                -- 添加新的请求记录
                redis.call('ZADD', key, now, now)
                redis.call('EXPIRE', key, window)
                return 1
            else
                return 0
            end
        "#;

        let result: i32 = redis::Script::new(script)
            .key(key)
            .arg(window)
            .arg(limit)
            .arg(now)
            .invoke(&mut conn)?;

        Ok(result == 1)
    }
}
```

---

## 7. 性能与监控设计

### 7.1 性能优化策略

**连接池配置**
```rust
// 数据库连接池
let db_config = sea_orm::ConnectOptions::new(&config.database.url)
    .max_connections(20)
    .min_connections(5)
    .connect_timeout(Duration::from_secs(10))
    .idle_timeout(Duration::from_secs(600))
    .max_lifetime(Duration::from_secs(3600))
    .sqlx_logging(true);

// Redis连接池
let redis_pool = r2d2_redis::RedisConnectionManager::new(&config.redis.url)?;
let redis_pool = r2d2::Pool::builder()
    .max_size(20)
    .connection_timeout(Duration::from_secs(5))
    .build(redis_pool)?;
```

**异步处理优化**
```rust
// 异步统计数据收集
pub struct AsyncStatsCollector {
    sender: mpsc::UnboundedSender<RequestStats>,
}

impl AsyncStatsCollector {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        
        // 启动后台任务处理统计数据
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                tokio::select! {
                    Some(stats) = receiver.recv() => {
                        batch.push(stats);
                        
                        // 批量处理，提高性能
                        if batch.len() >= 100 {
                            Self::batch_insert(&db, &mut batch).await;
                        }
                    }
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            Self::batch_insert(&db, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self { sender }
    }

    pub fn record(&self, stats: RequestStats) {
        self.sender.send(stats).ok();
    }

    async fn batch_insert(db: &DatabaseConnection, batch: &mut Vec<RequestStats>) {
        if batch.is_empty() {
            return;
        }

        // 批量插入统计数据
        let models: Vec<request_statistics::ActiveModel> = batch
            .drain(..)
            .map(|stats| stats.into())
            .collect();

        if let Err(e) = request_statistics::Entity::insert_many(models)
            .exec(db)
            .await
        {
            tracing::error!("Failed to batch insert statistics: {}", e);
        }
    }
}
```

### 7.2 监控指标

**Prometheus指标导出**
```rust
use prometheus::{Counter, Histogram, Gauge, register_counter, register_histogram, register_gauge};

pub struct Metrics {
    pub request_total: Counter,
    pub request_duration: Histogram,
    pub active_connections: Gauge,
    pub healthy_backends: Gauge,
}

impl Metrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let request_total = register_counter!(
            "requests_total",
            "Total number of requests processed"
        )?;

        let request_duration = register_histogram!(
            "request_duration_seconds",
            "Request duration in seconds"
        )?;

        let active_connections = register_gauge!(
            "active_connections",
            "Number of active connections"
        )?;

        let healthy_backends = register_gauge!(
            "healthy_backends_total", 
            "Number of healthy backend services"
        )?;

        Ok(Self {
            request_total,
            request_duration,
            active_connections,
            healthy_backends,
        })
    }
}

// 指标收集中间件
pub async fn metrics_middleware(
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    
    // 增加请求计数
    METRICS.request_total.inc();
    
    let response = next.run(request).await;
    
    // 记录请求耗时
    let duration = start.elapsed().as_secs_f64();
    METRICS.request_duration.observe(duration);
    
    response
}
```

**健康检查端点**
```rust
// 健康检查API
pub async fn health_check(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut status = serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {}
    });

    // 检查数据库连接
    let db_status = match app_state.db.ping().await {
        Ok(_) => "healthy",
        Err(_) => {
            status["status"] = "unhealthy".into();
            "unhealthy"
        }
    };
    status["checks"]["database"] = db_status.into();

    // 检查Redis连接
    let redis_status = match app_state.redis.get_connection() {
        Ok(mut conn) => {
            match redis::cmd("PING").query::<String>(&mut conn) {
                Ok(_) => "healthy",
                Err(_) => {
                    status["status"] = "unhealthy".into();
                    "unhealthy"
                }
            }
        }
        Err(_) => {
            status["status"] = "unhealthy".into();
            "unhealthy"
        }
    };
    status["checks"]["redis"] = redis_status.into();

    // 检查磁盘空间
    let disk_usage = get_disk_usage().unwrap_or(0.0);
    status["checks"]["disk_usage"] = disk_usage.into();
    if disk_usage > 90.0 {
        status["status"] = "degraded".into();
    }

    let response_status = match status["status"].as_str().unwrap() {
        "healthy" => StatusCode::OK,
        "degraded" => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };

    Ok((response_status, Json(status)).into())
}

// 获取系统指标
pub async fn metrics_endpoint() -> String {
    use prometheus::TextEncoder;
    
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode_to_string(&metric_families).unwrap_or_default()
}
```

### 7.3 日志系统

**结构化日志配置**
```rust
use tracing::{Level, info, warn, error};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_logging(config: &LogConfig) -> Result<(), Box<dyn std::error::Error>> {
    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(false)
                .with_timer(fmt::time::UtcTime::rfc_3339())
                .with_writer(std::io::stdout)
        )
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking)
        )
        .with(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(&config.level))?
        );

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

// 请求日志记录
#[derive(Debug, Serialize)]
pub struct AccessLog {
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub user_agent: Option<String>,
    pub client_ip: Option<String>,
    pub user_id: Option<i32>,
    pub api_id: Option<i32>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl AccessLog {
    pub fn log(&self) {
        info!(
            target: "access_log",
            request_id = %self.request_id,
            method = %self.method,
            path = %self.path,
            status_code = self.status_code,
            response_time_ms = self.response_time_ms,
            user_id = ?self.user_id,
            "{} {} {} {}ms",
            self.method,
            self.path,
            self.status_code,
            self.response_time_ms
        );
    }
}
```

### 7.4 告警系统

**告警规则定义**
```rust
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration: Duration,
    pub severity: AlertSeverity,
    pub channels: Vec<AlertChannel>,
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    ErrorRateHigh,
    ResponseTimeSlow,
    HealthyBackendsLow,
    DiskSpaceHigh,
    MemoryUsageHigh,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub enum AlertChannel {
    Email(String),
    Webhook(String),
    Slack(String),
}

pub struct AlertManager {
    rules: Vec<AlertRule>,
    active_alerts: HashMap<String, Alert>,
}

impl AlertManager {
    pub async fn evaluate_rules(&mut self) {
        for rule in &self.rules {
            if self.should_trigger_alert(rule).await {
                self.trigger_alert(rule).await;
            }
        }
    }

    async fn should_trigger_alert(&self, rule: &AlertRule) -> bool {
        match rule.condition {
            AlertCondition::ErrorRateHigh => {
                let error_rate = self.get_error_rate(rule.duration).await;
                error_rate > rule.threshold
            }
            AlertCondition::ResponseTimeSlow => {
                let avg_response_time = self.get_avg_response_time(rule.duration).await;
                avg_response_time > rule.threshold
            }
            _ => false,
        }
    }

    async fn trigger_alert(&mut self, rule: &AlertRule) {
        let alert = Alert {
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            message: format!("Alert triggered: {}", rule.name),
            timestamp: Utc::now(),
        };

        // 发送告警通知
        for channel in &rule.channels {
            self.send_alert(&alert, channel).await;
        }

        self.active_alerts.insert(rule.name.clone(), alert);
    }
}
```

---

## 8. 部署设计

### 8.1 Docker容器化

**Dockerfile**
```dockerfile
# 多阶段构建
FROM rust:1.75-slim as builder

# 安装系统依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制依赖文件
COPY Cargo.toml Cargo.lock ./
COPY migration/Cargo.toml migration/
COPY entity/Cargo.toml entity/

# 预构建依赖
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# 复制源代码并构建
COPY . .
RUN touch src/main.rs && cargo build --release

# 运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

# 创建非root用户
RUN groupadd -r appuser && useradd -r -g appuser appuser

WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/ai-proxy-system .
COPY --from=builder /app/config ./config
COPY --from=builder /app/frontend/dist ./frontend/dist

# 创建数据和日志目录
RUN mkdir -p /app/data /app/logs && chown -R appuser:appuser /app

USER appuser

EXPOSE 80 443

CMD ["./ai-proxy-system"]
```

**docker-compose.yml**
```yaml
version: '3.8'

services:
  ai-proxy:
    build: .
    ports:
      - "80:80"
      - "443:443"
    environment:
      - RUST_LOG=info
      - CONFIG_FILE=/app/config/config.prod.toml
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
      - ./config:/app/config
      - ./ssl:/app/ssl
    depends_on:
      - redis
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 30s
      timeout: 10s
      retries: 3

  # 可选：Prometheus监控
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'

  # 可选：Grafana仪表板
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./monitoring/grafana/datasources:/etc/grafana/provisioning/datasources

volumes:
  redis_data:
  prometheus_data:
  grafana_data:
```

### 8.2 生产环境配置

**config.prod.toml**
```toml
[server]
http_bind_address = "0.0.0.0:80"
https_bind_address = "0.0.0.0:443"
worker_threads = 8
max_connections = 10000

[database]
url = "sqlite:/app/data/production.db?mode=rwc"
max_connections = 20
min_connections = 5
connect_timeout = 10
idle_timeout = 600
max_lifetime = 3600

[redis]
url = "redis://redis:6379"
pool_size = 20
connection_timeout = 5

[tls]
enabled = true
cert_dir = "/app/ssl"
auto_renew = true
acme_email = "admin@your-domain.com"
acme_directory = "https://acme-v02.api.letsencrypt.org/directory"

[logging]
level = "info"
log_dir = "/app/logs"
max_file_size = "100MB"
max_files = 30

[monitoring]
enable_metrics = true
metrics_bind = "0.0.0.0:9090"
health_check_interval = 30
statistics_retention_days = 90

[security]
jwt_secret = "${JWT_SECRET}"
password_pepper = "${PASSWORD_PEPPER}"
encryption_key = "${ENCRYPTION_KEY}"
cors_origins = ["https://your-domain.com"]
max_request_size = "10MB"

[performance]
request_timeout = 30
proxy_timeout = 30
keep_alive_timeout = 60
read_timeout = 30
write_timeout = 30

[rate_limiting]
default_rate_limit = 1000
burst_size = 100
window_size = 60
```

### 8.3 部署脚本

**deploy.sh**
```bash
#!/bin/bash

set -e

# 配置变量
APP_NAME="ai-proxy-system"
VERSION=${1:-latest}
DOCKER_REGISTRY=${DOCKER_REGISTRY:-"your-registry.com"}
ENVIRONMENT=${2:-production}

echo "Deploying $APP_NAME version $VERSION to $ENVIRONMENT environment..."

# 构建镜像
echo "Building Docker image..."
docker build -t $DOCKER_REGISTRY/$APP_NAME:$VERSION .
docker tag $DOCKER_REGISTRY/$APP_NAME:$VERSION $DOCKER_REGISTRY/$APP_NAME:latest

# 推送到镜像仓库
echo "Pushing image to registry..."
docker push $DOCKER_REGISTRY/$APP_NAME:$VERSION
docker push $DOCKER_REGISTRY/$APP_NAME:latest

# 备份当前配置
echo "Backing up current configuration..."
kubectl create configmap $APP_NAME-config-backup-$(date +%Y%m%d%H%M%S) \
    --from-file=config/ \
    --namespace=$ENVIRONMENT || true

# 应用Kubernetes配置
echo "Applying Kubernetes manifests..."
envsubst < k8s/namespace.yaml | kubectl apply -f -
envsubst < k8s/configmap.yaml | kubectl apply -f -
envsubst < k8s/secret.yaml | kubectl apply -f -
envsubst < k8s/deployment.yaml | kubectl apply -f -
envsubst < k8s/service.yaml | kubectl apply -f -
envsubst < k8s/ingress.yaml | kubectl apply -f -

# 等待部署完成
echo "Waiting for deployment to be ready..."
kubectl rollout status deployment/$APP_NAME -n $ENVIRONMENT --timeout=600s

# 运行健康检查
echo "Running health checks..."
sleep 30
HEALTH_URL="https://your-domain.com/api/health"
if curl -f $HEALTH_URL; then
    echo "✅ Health check passed"
else
    echo "❌ Health check failed"
    exit 1
fi

# 清理旧版本镜像
echo "Cleaning up old images..."
docker image prune -f

echo "🎉 Deployment completed successfully!"
```

**Kubernetes部署清单**
```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ai-proxy-system
  namespace: production
spec:
  replicas: 3
  selector:
    matchLabels:
      app: ai-proxy-system
  template:
    metadata:
      labels:
        app: ai-proxy-system
    spec:
      containers:
      - name: ai-proxy-system
        image: your-registry.com/ai-proxy-system:latest
        ports:
        - containerPort: 80
        - containerPort: 443
        env:
        - name: CONFIG_FILE
          value: "/app/config/config.prod.toml"
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: ai-proxy-secrets
              key: jwt-secret
        volumeMounts:
        - name: config
          mountPath: /app/config
        - name: data
          mountPath: /app/data
        - name: ssl
          mountPath: /app/ssl
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /api/health
            port: 80
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /api/health
            port: 80
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: ai-proxy-config
      - name: data
        persistentVolumeClaim:
          claimName: ai-proxy-data
      - name: ssl
        secret:
          secretName: ai-proxy-tls
```

---

## 9. 测试策略

### 9.1 单元测试

**测试框架配置**
```rust
// Cargo.toml
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"
wiremock = "0.5"
testcontainers = "0.14"
```

**核心功能单元测试**
```rust
// tests/unit/scheduler_test.rs
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_round_robin_scheduler() {
        let mock_db = MockDatabase::new();
        let mock_redis = MockRedis::new();
        
        // 设置mock期望
        mock_db
            .expect_get_provider_keys()
            .with(eq(1), eq(1))
            .returning(|_, _| Ok(create_test_keys()));

        mock_redis
            .expect_get()
            .with(eq("round_robin:1:1"))
            .returning(|_| Ok(Some("0".to_string())));

        let scheduler = RoundRobinScheduler::new(Arc::new(mock_db), Arc::new(mock_redis));
        let user_api = create_test_user_api();
        
        let result = scheduler.select_backend(&user_api).await;
        
        assert!(result.is_ok());
        let selected = result.unwrap();
        assert_eq!(selected.id, 1);
    }

    #[tokio::test]
    async fn test_weighted_scheduler() {
        // 测试权重调度逻辑
        let scheduler = WeightedScheduler::new(mock_db(), mock_redis());
        // ... 测试实现
    }

    #[tokio::test] 
    async fn test_health_best_scheduler() {
        // 测试健康度最佳调度逻辑
        let scheduler = HealthBestScheduler::new(mock_db(), mock_redis(), mock_health_checker());
        // ... 测试实现
    }

    fn create_test_keys() -> Vec<entity::user_provider_keys::Model> {
        vec![
            entity::user_provider_keys::Model {
                id: 1,
                user_id: 1,
                provider_type_id: 1,
                api_key: "test-key-1".to_string(),
                name: "Test Key 1".to_string(),
                weight: 10,
                is_active: true,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
                max_requests_per_minute: 100,
                max_tokens_per_day: 1000000,
                used_tokens_today: 0,
                last_used: None,
            },
            // 更多测试数据...
        ]
    }
}
```

**认证测试**
```rust
// tests/unit/auth_test.rs
#[tokio::test]
async fn test_jwt_token_generation_and_validation() {
    let jwt_service = JwtService::new(JwtConfig {
        secret: "test-secret".to_string(),
        access_token_ttl: Duration::from_secs(3600),
        refresh_token_ttl: Duration::from_secs(86400),
        issuer: "test".to_string(),
        audience: "test".to_string(),
    });

    // 生成token
    let tokens = jwt_service.generate_tokens(123).unwrap();
    assert!(!tokens.access_token.is_empty());
    assert!(!tokens.refresh_token.is_empty());

    // 验证token
    let claims = jwt_service.validate_token(&tokens.access_token).unwrap();
    assert_eq!(claims.sub, "123");
}

#[tokio::test]
async fn test_password_hashing_and_verification() {
    let password = "test-password-123";
    let hash = PasswordService::hash_password(password).unwrap();
    
    assert!(PasswordService::verify_password(password, &hash).unwrap());
    assert!(!PasswordService::verify_password("wrong-password", &hash).unwrap());
}

#[tokio::test]
async fn test_api_key_generation() {
    let api_key = generate_api_key();
    let secret = generate_api_secret();
    
    assert!(api_key.starts_with("proxy_"));
    assert_eq!(api_key.len(), 70); // "proxy_" + 64 hex chars
    assert_eq!(secret.len(), 64); // 64 hex chars
}
```

### 9.2 集成测试

**数据库集成测试**
```rust
// tests/integration/database_test.rs
use testcontainers::{clients::Cli, images::postgres::Postgres, Container};

#[tokio::test]
async fn test_user_crud_operations() {
    let docker = Cli::default();
    let container = docker.run(Postgres::default());
    
    let database_url = format!(
        "postgres://postgres:postgres@localhost:{}/postgres",
        container.get_host_port_ipv4(5432)
    );
    
    let db = Database::connect(&database_url).await.unwrap();
    migration::run_migrations(&db).await.unwrap();
    
    // 测试用户创建
    let user = create_test_user(&db).await;
    assert_eq!(user.username, "testuser");
    
    // 测试用户查询
    let found_user = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
    assert_eq!(found_user.email, user.email);
    
    // 测试用户更新
    let mut user_model: users::ActiveModel = found_user.into();
    user_model.username = Set("updated_user".to_string());
    let updated_user = user_model.update(&db).await.unwrap();
    assert_eq!(updated_user.username, "updated_user");
    
    // 测试用户删除
    updated_user.delete(&db).await.unwrap();
    let deleted = Users::find_by_id(updated_user.id).one(&db).await.unwrap();
    assert!(deleted.is_none());
}
```

**API集成测试**
```rust
// tests/integration/api_test.rs
use axum_test::TestServer;

#[tokio::test]
async fn test_user_registration_and_login() {
    let app_state = create_test_app_state().await;
    let app = create_management_router(app_state);
    let server = TestServer::new(app).unwrap();

    // 测试用户注册
    let registration_data = serde_json::json!({
        "username": "testuser",
        "email": "test@example.com",
        "password": "test123456"
    });

    let response = server
        .post("/api/auth/register")
        .json(&registration_data)
        .await;

    response.assert_status_ok();
    let user_data: serde_json::Value = response.json();
    assert_eq!(user_data["data"]["username"], "testuser");

    // 测试用户登录
    let login_data = serde_json::json!({
        "email": "test@example.com",
        "password": "test123456"
    });

    let response = server
        .post("/api/auth/login")
        .json(&login_data)
        .await;

    response.assert_status_ok();
    let login_response: serde_json::Value = response.json();
    assert!(login_response["data"]["access_token"].is_string());
    
    let access_token = login_response["data"]["access_token"].as_str().unwrap();

    // 测试API密钥创建
    let api_data = serde_json::json!({
        "provider_type_id": 1,
        "name": "Test API",
        "scheduling_strategy": "round_robin"
    });

    let response = server
        .post("/api/apis")
        .add_header("authorization", format!("Bearer {}", access_token))
        .json(&api_data)
        .await;

    response.assert_status_ok();
    let api_response: serde_json::Value = response.json();
    assert_eq!(api_response["data"]["name"], "Test API");
}
```

### 9.3 负载测试

**使用Artillery进行负载测试**
```yaml
# artillery-load-test.yml
config:
  target: 'https://localhost'
  phases:
    - duration: 60
      arrivalRate: 10
      name: "Warm up"
    - duration: 300
      arrivalRate: 50
      name: "Sustained load"
    - duration: 120
      arrivalRate: 100
      name: "Spike test"

scenarios:
  - name: "API Proxy Test"
    weight: 70
    flow:
      - post:
          url: "/v1/chat/completions"
          headers:
            Authorization: "Bearer {{ proxy_api_key }}"
            Content-Type: "application/json"
          json:
            model: "gpt-3.5-turbo"
            messages:
              - role: "user"
                content: "Hello, how are you?"
            max_tokens: 50

  - name: "Management API Test"
    weight: 30
    flow:
      - get:
          url: "/api/statistics/requests"
          headers:
            Authorization: "Bearer {{ management_token }}"
```

**压力测试脚本**
```bash
#!/bin/bash

# load-test.sh
echo "Starting load tests..."

# 启动监控
docker-compose up -d prometheus grafana

# 运行负载测试
artillery run artillery-load-test.yml --output results.json

# 生成报告
artillery report results.json --output load-test-report.html

# 检查系统指标
echo "System metrics during test:"
echo "Memory usage: $(free -h)"
echo "CPU usage: $(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | awk -F'%' '{print $1}')"
echo "Disk usage: $(df -h)"

echo "Load test completed. Report available at load-test-report.html"
```

### 9.4 端到端测试

**使用Playwright的E2E测试**
```javascript
// tests/e2e/user-flow.spec.js
const { test, expect } = require('@playwright/test');

test.describe('Complete User Flow', () => {
  test('user can register, create API, and make proxy request', async ({ page }) => {
    // 用户注册
    await page.goto('/register');
    await page.fill('[data-testid="username"]', 'testuser');
    await page.fill('[data-testid="email"]', 'test@example.com');
    await page.fill('[data-testid="password"]', 'password123');
    await page.click('[data-testid="register-button"]');
    
    await expect(page).toHaveURL('/dashboard');

    // 创建API服务
    await page.click('[data-testid="create-api-button"]');
    await page.selectOption('[data-testid="provider-select"]', 'openai');
    await page.fill('[data-testid="api-name"]', 'My Test API');
    await page.click('[data-testid="submit-api"]');

    // 添加提供商密钥
    await page.click('[data-testid="add-key-button"]');
    await page.fill('[data-testid="provider-key"]', 'sk-test123456789');
    await page.fill('[data-testid="key-name"]', 'Test Key');
    await page.click('[data-testid="save-key"]');

    // 测试代理请求
    await page.click('[data-testid="test-api-button"]');
    await page.fill('[data-testid="test-message"]', 'Hello, test!');
    await page.click('[data-testid="send-test"]');

    await expect(page.locator('[data-testid="test-response"]')).toBeVisible();
    await expect(page.locator('[data-testid="test-response"]')).toContainText('response');
  });

  test('admin can view system metrics', async ({ page }) => {
    // 管理员登录
    await page.goto('/admin/login');
    await page.fill('[data-testid="email"]', 'admin@example.com');
    await page.fill('[data-testid="password"]', 'admin123');
    await page.click('[data-testid="login-button"]');

    // 查看系统监控
    await page.goto('/admin/monitoring');
    await expect(page.locator('[data-testid="system-status"]')).toContainText('Healthy');
    await expect(page.locator('[data-testid="active-connections"]')).toBeVisible();
    await expect(page.locator('[data-testid="request-rate"]')).toBeVisible();
  });
});
```

---

## 10. 项目实施计划

### 10.1 项目阶段规划

#### **第一阶段：基础架构 (6周)**

**目标**：建立项目基础框架和核心数据层

**第1-2周：项目搭建**
- [ ] Rust项目初始化和依赖配置
- [ ] 项目目录结构搭建
- [ ] CI/CD流水线配置 
- [ ] 开发环境Docker化
- [ ] 代码规范和lint配置

**第3-4周：数据库设计与实现**
- [ ] SQLite数据库结构设计
- [ ] Sea-ORM实体模型创建
- [ ] 数据库迁移脚本编写
- [ ] Redis缓存层集成
- [ ] 基础CRUD操作实现

**第5-6周：认证系统**
- [ ] JWT认证服务实现
- [ ] 用户注册/登录功能
- [ ] 密码加密和验证
- [ ] 认证中间件开发
- [ ] 单元测试编写

**里程碑检查**：
- [x] 用户可以注册和登录
- [x] 数据库连接正常工作
- [x] 基础测试用例通过
- [x] CI/CD流水线运行成功

---

#### **第二阶段：Pingora集成与代理核心 (8周)**

**目标**：实现统一入口和AI服务代理功能

**第7-9周：Pingora服务集成**
- [ ] Pingora框架集成
- [ ] 统一入口服务实现
- [ ] 路由分发机制
- [ ] 请求上下文管理
- [ ] 基础HTTP代理功能

**第10-12周：AI服务商适配**
- [ ] OpenAI API接口适配
- [ ] Gemini API接口适配
- [ ] Claude API接口适配
- [ ] 请求/响应格式转换
- [ ] 上游服务连接管理

**第13-14周：负载均衡实现**
- [ ] 轮询调度器
- [ ] 权重调度器
- [ ] 健康度最佳调度器
- [ ] 调度策略切换
- [ ] 故障转移机制

**里程碑检查**：
- [x] 可以成功代理转发到三个AI服务商
- [x] 负载均衡策略正常工作
- [x] 支持故障自动切换
- [x] 基本的请求统计功能

---

#### **第三阶段：管理功能与监控 (6周)**

**目标**：完成用户管理界面和监控系统

**第15-17周：Axum管理API**
- [ ] 内嵌Axum服务实现
- [ ] 用户API密钥管理接口
- [ ] 提供商密钥池管理接口
- [ ] 统计数据查询接口
- [ ] API文档生成

**第18-20周：健康检查与监控**
- [ ] 健康检查服务实现
- [ ] 后台定时任务
- [ ] 统计数据收集
- [ ] Prometheus指标导出
- [ ] 告警机制实现

**里程碑检查**：
- [x] 完整的管理API功能
- [x] 健康检查正常工作
- [x] 监控数据可查看
- [x] 告警通知正常

---

#### **第四阶段：安全与TLS (4周)**

**目标**：实现安全传输和证书管理

**第21-22周：TLS证书管理**
- [ ] TLS证书管理器
- [ ] Let's Encrypt集成
- [ ] 自动证书续期
- [ ] SNI多域名支持

**第23-24周：安全强化**
- [ ] 源信息隐藏机制
- [ ] 速率限制实现
- [ ] 安全头部设置
- [ ] 数据加密存储

**里程碑检查**：
- [x] HTTPS正常工作
- [x] 证书自动续期
- [x] 安全防护到位
- [x] 通过安全测试

---

#### **第五阶段：前端开发 (6周)**

**目标**：构建完整的Web管理界面

**第25-27周：Vue项目搭建**
- [ ] Vue 3 + Element Plus搭建
- [ ] 路由和状态管理
- [ ] API客户端封装
- [ ] 组件库搭建

**第28-30周：核心页面开发**
- [ ] 登录注册页面
- [ ] 仪表板页面
- [ ] API密钥管理页面
- [ ] 统计监控页面
- [ ] 系统设置页面

**里程碑检查**：
- [x] 完整的前端管理界面
- [x] 用户体验友好
- [x] 响应式设计
- [x] 前后端联调成功

---

#### **第六阶段：测试与优化 (4周)**

**目标**：全面测试和性能优化

**第31-32周：测试完善**
- [ ] 单元测试补充
- [ ] 集成测试编写
- [ ] E2E测试实现
- [ ] 负载测试执行

**第33-34周：性能优化与部署**
- [ ] 性能瓶颈分析
- [ ] 数据库查询优化
- [ ] 缓存策略优化
- [ ] 部署脚本完善

**里程碑检查**：
- [x] 测试覆盖率 > 80%
- [x] 性能指标达标
- [x] 生产环境部署成功
- [x] 用户验收通过

### 10.2 团队配置建议

**核心团队 (4-5人)**

- **后端工程师 (2人)**
  - Rust开发经验 > 2年
  - 熟悉异步编程和网络编程
  - 了解代理服务器原理
  - 负责Pingora集成和代理逻辑

- **全栈工程师 (1人)**
  - Rust + Vue开发经验
  - 熟悉API设计和前端开发
  - 负责管理API和前端界面

- **DevOps工程师 (1人)**
  - Docker和Kubernetes经验
  - 熟悉CI/CD流程
  - 负责部署和运维

- **测试工程师 (1人，可选)**
  - 自动化测试经验
  - 性能测试和安全测试
  - 负责测试策略和执行

### 10.3 风险评估与应对

#### **技术风险**

| 风险 | 影响度 | 概率 | 应对措施 |
|------|--------|------|----------|
| Pingora集成复杂度超预期 | 高 | 中 | 预研POC，准备hyper替代方案 |
| AI服务商API变更 | 中 | 高 | 接口版本管理，适配器模式 |
| TLS证书续期失败 | 高 | 低 | 多重备份，手动证书支持 |
| 高并发性能问题 | 高 | 中 | 早期压力测试，架构优化 |

#### **项目风险**

| 风险 | 影响度 | 概率 | 应对措施 |
|------|--------|------|----------|
| 关键人员离职 | 高 | 中 | 知识文档化，技能交叉培训 |
| 需求变更频繁 | 中 | 高 | 敏捷开发，需求优先级管理 |
| 第三方依赖风险 | 中 | 中 | 依赖版本锁定，替代方案调研 |
| 安全漏洞发现 | 高 | 中 | 代码审查，安全测试，及时修补 |

### 10.4 质量保证措施

#### **代码质量**
- 代码审查(Code Review)必须通过
- 测试覆盖率要求 > 80%
- 自动化代码格式检查
- 静态代码分析

#### **性能标准**
- API响应时间 < 500ms (P95)
- 代理转发延迟 < 100ms
- 支持并发连接 > 10,000
- 可用性 > 99.9%

#### **安全标准**
- 所有敏感数据加密存储
- 通过OWASP安全测试
- 定期安全漏洞扫描
- 渗透测试通过

### 10.5 发布计划

#### **Alpha版本 (第20周)**
- 核心代理功能
- 基础管理API
- 内部测试使用

#### **Beta版本 (第28周)**
- 完整功能实现
- 前端管理界面
- 小范围用户测试

#### **正式版本 (第34周)**
- 生产环境部署
- 完整文档
- 用户培训和支持

---

## 11. 项目任务管理与实施跟踪

### 11.1 任务分类标识

- 🟢 **已完成** (Completed) - 功能完整实现并测试通过
- 🟡 **进行中** (In Progress) - 正在开发实现中 
- ⚪ **待完成** (Pending) - 等待开始的任务
- 🔄 **重构中** (Refactoring) - 架构重构相关任务

### 11.2 基础设施搭建阶段 ✅

#### 🟢 开发环境配置
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- 验证Rust 1.75+版本环境
- 配置VS Code + rust-analyzer开发工具链
- 设置clippy、rustfmt代码质量工具
- 配置cargo audit安全审计工具

**关键输出**:
- 标准化的Rust开发环境
- 代码质量检查配置
- 安全审计流程

#### 🟢 项目目录结构
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- 按照DESIGN.md创建完整的目录结构
- 建立src/、migration/、entity/、frontend/等核心目录
- 定义模块化的代码组织方式

**关键输出**:
- 清晰的项目目录结构
- 模块化的代码组织
- 便于维护的架构布局

#### 🟢 错误处理框架
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- 使用thiserror + anyhow构建错误处理体系
- 定义ProxyError、DatabaseError等专用错误类型
- 实现统一的错误传播和处理机制

**关键输出**:
- 完整的错误类型定义 (`src/error/`)
- 统一的错误处理接口
- 结构化错误信息

#### 🟢 数据库实体定义
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- 使用Sea-ORM定义8个核心表的实体模型
- 建立用户、API密钥、统计等核心实体
- 定义实体间的关联关系

**关键输出**:
- 完整的实体定义 (`entity/src/`)
- 数据库表结构设计
- 实体关联关系

### 11.3 核心代理功能阶段 🔄

#### 🟢 认证授权中间件
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- 实现API密钥验证机制
- 支持JWT令牌处理
- 基于角色的权限检查(RBAC)

**关键输出**:
- 认证服务 (`src/auth/`)
- JWT管理器
- 权限检查系统

**技术实现**:
- 支持Bearer Token、API Key、Basic Auth
- 17种权限类型，7种预定义角色
- 数据库集成的API密钥管理
- 缓存优化的认证性能

#### 🟢 动态配置管理系统
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- 从数据库动态加载服务商配置，替代硬编码地址
- 实现ProviderConfigManager统一配置管理
- 支持缓存优化和配置热重载

**关键输出**:
- 动态配置加载系统 (`src/config/provider_config.rs`)
- 数据库驱动的服务商地址管理
- 缓存优化的配置访问

**技术实现**:
- provider_types表驱动的配置管理
- Redis缓存优化配置访问性能
- 支持Google API Key和标准Bearer Token认证
- 自动地址标准化和端口处理

#### ⚪ 负载均衡调度器
**优先级**: 高  
**状态**: 待完成

**核心描述**:
- 实现轮询(Round Robin)调度算法
- 支持权重(Weighted)分配策略
- 基于健康度的最佳调度算法

**待实现功能**:
- 多种负载均衡算法
- 动态权重调整
- 健康检查集成
- 故障转移机制

#### ⚪ AI服务商适配器
**优先级**: 高  
**状态**: 待完成

**核心描述**:
- 实现OpenAI API格式标准化
- Google Gemini适配器实现
- Anthropic Claude适配器实现

**待实现功能**:
- Chat Completions API适配
- 流式响应处理
- 模型参数转换
- 错误处理和重试

### 11.4 管理和监控阶段 📊

#### ⚪ Axum管理API
**优先级**: 中  
**状态**: 待完成

**核心描述**:
- 实现内嵌HTTP服务
- 设计RESTful API接口
- 提供管理功能端点

**待实现功能**:
- 用户管理API
- API密钥管理API
- 统计查询API
- 系统配置API

#### ⚪ 监控统计系统
**优先级**: 中  
**状态**: 待完成

**核心描述**:
- 实时数据收集和聚合
- 性能指标监控
- Token使用统计和分析

**待实现功能**:
- 实时指标收集
- 历史数据分析
- 性能监控面板
- 告警规则配置

### 11.5 前端界面开发 🎨

#### ✅ React 18前端应用
**优先级**: 高  
**状态**: 已完成

**核心描述**:
- [x] 搭建React 18 + TypeScript技术栈
- [x] 集成ESBuild构建工具和shadcn/ui
- [x] 使用Zustand状态管理和React Router 7路由

**已实现功能**:
- [x] 现代化前端架构和主题系统
- [x] 响应式用户界面和移动端适配
- [x] 组件化开发(基于shadcn/ui)
- [x] TypeScript类型安全保障

### 11.6 当前状态总结

#### 📊 任务统计
- ✅ **已完成**: 6个核心任务 (基础设施 + 核心认证 + 动态配置)
- 🔄 **重构中**: 1个任务 (架构升级)
- ⏳ **待完成**: 15个主要任务

#### 🎯 当前重点任务
1. **负载均衡调度器** - 实现多种调度算法
2. **AI服务商适配器** - 完成主要AI提供商适配  
3. **健康检查系统** - 建立服务监控机制
4. **请求转发处理** - 完善代理转发逻辑

#### 📈 重要里程碑
- **Phase 0完成**: 动态配置系统替代硬编码地址 ✅
- **Phase 1完成**: 基础认证和权限系统 ✅  
- **Phase 2目标**: 完整的AI代理核心功能
- **Phase 3目标**: 管理界面和监控系统

#### 🔄 下一步行动
1. **立即开始**: 负载均衡器核心算法实现
2. **并行开发**: OpenAI适配器和健康检查系统
3. **重点测试**: 端到端代理功能验证
4. **性能验证**: 高并发场景压力测试

---

## 12. 新增核心功能详细设计

### 12.1 OAuth 2.0 授权系统设计

#### 12.1.1 系统架构

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Client App    │    │  OAuth Client   │    │  OAuth Provider │
│                 │    │                 │    │                 │
│ • 用户界面       │    │ • 客户端ID       │    │ • 授权服务器     │
│ • 重定向处理     │    │ • 客户端密钥     │    │ • 令牌端点       │
│ • Token存储     │    │ • 重定向URI      │    │ • 用户信息       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │                       │                       │
         └─────── 1. 授权请求 ────┼───────────────────────▶
                 │              │                       │
                 │              │    2. 用户登录/同意    │
                 │              │◀───────────────────────┤
                 │              │                       │
                 │ 3. 授权码返回  │                       │
                 │◀─────────────┼───────────────────────┤
                 │              │                       │
                 │ 4. Token交换  │                       │
                 └───────────────┼───────────────────────▶
                                │                       │
                                │   5. Access Token     │
                                │◀───────────────────────┤
                                │                       │
                                │ 6. 自动Token刷新      │
                                │◀───────────────────────┘ (后台)
```

#### 12.1.2 数据库设计

```sql
-- OAuth客户端会话表
CREATE TABLE oauth_client_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    provider_type_id INTEGER NOT NULL,
    client_id VARCHAR(255) NOT NULL,        -- OAuth客户端ID
    client_secret VARCHAR(255) NOT NULL,   -- OAuth客户端密钥
    authorization_code TEXT,               -- 授权码
    access_token TEXT NOT NULL,            -- 访问令牌
    refresh_token TEXT,                   -- 刷新令牌
    token_type VARCHAR(50) DEFAULT 'Bearer', -- 令牌类型
    expires_at DATETIME,                   -- 过期时间
    scope VARCHAR(500),                   -- 权限范围
    is_active BOOLEAN DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    INDEX idx_user_provider (user_id, provider_type_id),
    INDEX idx_access_token (access_token),
    INDEX idx_refresh_token (refresh_token),
    INDEX idx_expires_at (expires_at)
);

-- OAuth token刷新任务表
CREATE TABLE oauth_token_refresh_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    oauth_session_id INTEGER NOT NULL,
    next_refresh_at DATETIME NOT NULL,       -- 下次刷新时间
    refresh_count INTEGER DEFAULT 0,       -- 刷新次数
    last_refresh_status VARCHAR(20),       -- 最后刷新状态
    last_refresh_error TEXT,               -- 最后刷新错误
    is_active BOOLEAN DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (oauth_session_id) REFERENCES oauth_client_sessions(id) ON DELETE CASCADE,
    INDEX idx_next_refresh (next_refresh_at),
    INDEX idx_active_tasks (is_active, next_refresh_at)
);
```

#### 12.1.3 核心组件设计

**OAuth客户端管理器**
```rust
pub struct OAuthClient {
    db: Arc<DatabaseConnection>,
    config: Arc<OAuthConfig>,
    token_store: Arc<TokenCache>,
}

impl OAuthClient {
    // 授权码流程
    pub async fn authorize_with_code(&self, request: AuthCodeRequest) -> Result<AuthCodeResponse>

    // Token交换
    pub async fn exchange_code_for_token(&self, code: String) -> Result<TokenResponse>

    // Token刷新
    pub async fn refresh_access_token(&self, refresh_token: String) -> Result<TokenResponse>

    // 获取有效的访问令牌
    pub async fn get_valid_access_token(&self, session_id: i32) -> Result<String>
}
```

**Token刷新服务**
```rust
pub struct OAuthTokenRefreshService {
    db: Arc<DatabaseConnection>,
    oauth_client: Arc<OAuthClient>,
    config: Arc<RefreshServiceConfig>,
}

impl OAuthTokenRefreshService {
    // 执行token刷新
    pub async fn refresh_token(&self, session_id: i32) -> Result<()>

    // 批量刷新过期token
    pub async fn refresh_expired_tokens(&self) -> Result<Vec<i32>>

    // 调度刷新任务
    pub async fn schedule_refresh_tasks(&self) -> Result<()>
}
```

**后台刷新任务**
```rust
pub struct OAuthTokenRefreshTask {
    refresh_service: Arc<OAuthTokenRefreshService>,
    config: Arc<RefreshTaskConfig>,
}

impl OAuthTokenRefreshTask {
    // 启动后台刷新任务
    pub async fn start(&self) -> Result<()>

    // 停止后台刷新任务
    pub async fn stop(&self) -> Result<()>

    // 执行刷新循环
    async fn refresh_loop(&self) -> !
}
```

### 12.2 智能API密钥管理系统

#### 12.2.1 系统架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    SmartApiKeyProvider                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   密钥选择器     │  │   健康检查器     │  │   故障恢复器     │ │
│  │                 │  │                 │  │                 │ │
│  │ • 轮询策略       │  │ • 实时监控       │  │ • 自动切换       │ │
│  │ • 权重策略       │  │ • 性能统计       │  │ • 降级处理       │ │
│  │ • 健康度策略     │  │ • 故障检测       │  │ • 重试机制       │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │
              ┌─────────────────────────────────────────────────────────────────┐
              │                    API密钥池                                      │
              │                                                                 │
              │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
              │  │   OpenAI    │ │   Gemini    │ │   Claude    │ │   Custom    │ │
              │  │             │ │             │ │             │ │             │ │
              │  │ • API Key   │ │ • API Key   │ │ • API Key   │ │ • API Key   │ │
              │  │ • 权重      │ │ • 权重      │ │ • 权重      │ │ • 权重      │ │
              │  │ • 健康状态  │ │ • 健康状态  │ │ • 健康状态  │ │ • 健康状态  │ │
              │  │ • 使用统计  │ │ • 使用统计  │ │ • 使用统计  │ │ • 使用统计  │ │
              │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘ │
              └─────────────────────────────────────────────────────────────────┘
```

#### 12.2.2 核心算法设计

**智能密钥选择算法**
```rust
pub enum SelectionStrategy {
    RoundRobin,      // 轮询调度
    Weighted,        // 权重调度
    HealthBest,      // 健康度最佳
    LeastUsed,       // 最少使用
    FastestResponse, // 最快响应
}

impl SmartApiKeyProvider {
    // 智能选择API密钥
    pub async fn select_api_key(&self, context: &SelectionContext) -> Result<ApiKey> {
        match self.strategy {
            SelectionStrategy::RoundRobin => self.round_robin_select(context).await,
            SelectionStrategy::Weighted => self.weighted_select(context).await,
            SelectionStrategy::HealthBest => self.health_best_select(context).await,
            SelectionStrategy::LeastUsed => self.least_used_select(context).await,
            SelectionStrategy::FastestResponse => self.fastest_response_select(context).await,
        }
    }

    // 健康度最佳选择算法
    async fn health_best_select(&self, context: &SelectionContext) -> Result<ApiKey> {
        let available_keys = self.get_available_keys(context).await?;

        // 计算每个密钥的健康分数
        let mut scored_keys: Vec<_> = available_keys.into_iter().map(|key| {
            let health_score = self.calculate_health_score(&key).await;
            (key, health_score)
        }).collect();

        // 按健康分数排序
        scored_keys.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 选择健康分数最高的密钥
        scored_keys.into_iter()
            .find(|(_, score)| *score > 0.7) // 健康分数阈值
            .map(|(key, _)| key)
            .ok_or_else(|| ProxyError::NoHealthyKey)
    }

    // 计算健康分数
    async fn calculate_health_score(&self, key: &ApiKey) -> f64 {
        let health_status = self.health_checker.get_health_status(key.id).await;
        let stats = self.statistics.get_key_statistics(key.id).await;

        let mut score = 0.0;

        // 基础健康状态 (40%)
        score += if health_status.is_healthy { 0.4 } else { 0.0 };

        // 响应时间 (30%)
        let response_time_score = (1.0 / (1.0 + health_status.response_time_ms as f64 / 1000.0)) * 0.3;
        score += response_time_score;

        // 成功率 (20%)
        score += health_status.success_rate * 0.2;

        // 负载均衡 (10%)
        let load_score = (1.0 / (1.0 + stats.concurrent_requests as f64 / 10.0)) * 0.1;
        score += load_score;

        score.min(1.0).max(0.0)
    }
}
```

### 12.3 API密钥健康监控系统

#### 12.3.1 监控架构

```
┌─────────────────────────────────────────────────────────────────┐
│                   HealthMonitor                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   检查调度器     │  │   状态收集器     │  │   故障处理器     │ │
│  │                 │  │                 │  │                 │ │
│  │ • 定时检查       │  │ • 性能指标       │  │ • 故障检测       │ │
│  │ • 触发检查       │  │ • 错误统计       │  │ • 自动恢复       │ │
│  │ • 优先级管理     │  │ • 趋势分析       │  │ • 告警通知       │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │
              ┌─────────────────────────────────────────────────────────────────┐
              │                   健康状态存储                                    │
              │                                                                 │
              │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
              │  │  健康状态    │ │  检查历史    │ │  性能指标    │ │  故障日志    │ │
              │  │             │ │             │ │             │ │             │ │
              │  │ • 当前状态    │ │ • 检查记录    │ │ • 响应时间    │ │ • 故障时间    │ │
              │  │ • 健康分数    │ │ • 状态变化    │ │ • 成功率      │ │ • 恢复时间    │ │
              │  │ • 最后检查    │ │ • 持续时间    │ │ • 错误率      │ │ • 处理结果    │ │
              │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘ │
              └─────────────────────────────────────────────────────────────────┘
```

#### 12.3.2 健康检查算法

```rust
pub struct ApiKeyHealthChecker {
    db: Arc<DatabaseConnection>,
    cache: Arc<CacheManager>,
    config: Arc<HealthConfig>,
}

impl ApiKeyHealthChecker {
    // 执行健康检查
    pub async fn check_health(&self, key_id: i32) -> Result<HealthStatus> {
        let key = self.get_api_key(key_id).await?;
        let start_time = Instant::now();

        // 执行实际的API测试
        let result = self.perform_api_test(&key).await;
        let response_time = start_time.elapsed();

        // 更新健康状态
        let health_status = HealthStatus {
            key_id,
            is_healthy: result.is_ok(),
            response_time_ms: response_time.as_millis() as u32,
            success_rate: self.calculate_success_rate(key_id).await?,
            last_success: if result.is_ok() { Some(Utc::now()) } else { None },
            last_failure: if result.is_err() { Some(Utc::now()) } else { None },
            consecutive_failures: self.get_consecutive_failures(key_id).await?,
            error_message: result.err().map(|e| e.to_string()),
            updated_at: Utc::now(),
        };

        // 保存健康状态
        self.save_health_status(&health_status).await?;

        Ok(health_status)
    }

    // 批量健康检查
    pub async fn batch_check_health(&self, key_ids: &[i32]) -> Vec<(i32, Result<HealthStatus>)> {
        let tasks: Vec<_> = key_ids.iter().map(|&key_id| {
            let checker = self.clone();
            async move {
                let result = checker.check_health(key_id).await;
                (key_id, result)
            }
        }).collect();

        join_all(tasks).await
    }

    // 自动故障恢复
    pub async fn auto_recovery(&self, key_id: i32) -> Result<bool> {
        let health_status = self.get_health_status(key_id).await?;

        // 如果连续失败次数过多，执行恢复流程
        if health_status.consecutive_failures > self.config.max_consecutive_failures {
            // 1. 检查网络连接
            if self.check_network_connectivity(&health_status).await? {
                // 2. 尝试重新认证
                if self.reauthenticate_key(&health_status).await? {
                    // 3. 验证API功能
                    if self.validate_api_functionality(&health_status).await? {
                        // 恢复成功
                        self.mark_key_healthy(key_id).await?;
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }
}
```

### 12.4 统一请求追踪系统

#### 12.4.1 追踪架构

```
┌─────────────────────────────────────────────────────────────────┐
│                   UnifiedTraceSystem                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   TraceID生成   │  │   请求收集器     │  │   数据提取器     │ │
│  │                 │  │                 │  │                 │ │
│  │ • 唯一标识       │  │ • 请求信息       │  │ • Token提取     │ │
│  │ • 链路追踪       │  │ • 响应信息       │  │ • 模型提取     │ │
│  │ • 上下文传递     │  │ • 错误信息       │  │ • 字段映射     │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │
              ┌─────────────────────────────────────────────────────────────────┐
              │                   追踪数据存储                                    │
              │                                                                 │
              │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
              │  │  请求追踪    │ │  Token统计   │ │  模型统计    │ │  错误日志    │ │
              │  │             │ │             │ │             │ │             │ │
              │  │ • 基本信息    │ │ • 使用量      │ │ • 使用分布    │ │ • 错误类型    │ │
              │  │ • 性能指标    │ │ • 成本统计    │ │ • 成本分析    │ │ • 错误频率    │ │
              │  │ • 状态码      │ │ • 趋势分析    │ │ • 性能指标    │ │ • 恢复策略    │ │
              │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘ │
              └─────────────────────────────────────────────────────────────────┘
```

#### 12.4.2 核心追踪实现

```rust
pub struct ImmediateProxyTracer {
    db: Arc<DatabaseConnection>,
    config: Arc<ImmediateTracerConfig>,
    token_extractor: Arc<TokenFieldExtractor>,
    model_extractor: Arc<ModelExtractor>,
}

impl ImmediateProxyTracer {
    // 追踪请求
    pub async fn trace_request(&self, trace_data: RequestTraceData) -> Result<()> {
        // 生成唯一TraceID
        let trace_id = self.generate_trace_id();

        // 提取Token信息
        let token_info = self.token_extractor.extract_tokens(&trace_data).await?;

        // 提取模型信息
        let model_info = self.model_extractor.extract_model(&trace_data).await?;

        // 构建追踪记录
        let trace_record = ProxyTracing {
            id: generate_id(),
            trace_id,
            request_id: trace_data.request_id,
            user_service_api_id: trace_data.user_service_api_id,
            user_provider_key_id: trace_data.user_provider_key_id,
            method: trace_data.method,
            path: trace_data.path,
            status_code: trace_data.status_code,
            response_time_ms: trace_data.response_time_ms,
            request_size: trace_data.request_size,
            response_size: trace_data.response_size,

            // Token信息
            prompt_tokens: token_info.prompt_tokens,
            completion_tokens: token_info.completion_tokens,
            total_tokens: token_info.total_tokens,

            // 模型信息
            model_used: model_info.model_name,
            provider_type: model_info.provider_type,

            // 其他信息
            client_ip: trace_data.client_ip,
            user_agent: trace_data.user_agent,
            error_type: trace_data.error_type,
            error_message: trace_data.error_message,
            created_at: Utc::now(),
        };

        // 立即写入数据库
        self.save_trace_record(&trace_record).await?;

        // 异步更新统计数据
        self.update_statistics(&trace_record).await?;

        Ok(())
    }

    // 数据驱动字段提取
    async fn extract_tokens_dynamic(&self, response: &Response) -> Result<TokenInfo> {
        // 从数据库配置中获取提取规则
        let extraction_rules = self.get_token_extraction_rules().await?;

        let mut token_info = TokenInfo::default();

        for rule in extraction_rules {
            if rule.provider_type == self.provider_type {
                match rule.field_type {
                    FieldType::PromptTokens => {
                        token_info.prompt_tokens = self.extract_field(response, &rule.field_path).await?;
                    }
                    FieldType::CompletionTokens => {
                        token_info.completion_tokens = self.extract_field(response, &rule.field_path).await?;
                    }
                    FieldType::TotalTokens => {
                        token_info.total_tokens = self.extract_field(response, &rule.field_path).await?;
                    }
                }
            }
        }

        Ok(token_info)
    }
}
```

---

## 总结

本架构设计文档详细描述了AI代理系统的完整技术方案，涵盖了从系统架构到具体实现的各个层面。该系统采用Rust + Pingora作为统一入口，内嵌Axum提供管理功能，支持多AI服务商代理、负载均衡、监控统计等企业级功能。

**核心优势**：
1. **统一入口**：Pingora处理所有请求，简化部署
2. **高性能**：Rust异步架构，支持高并发
3. **源信息隐藏**：完全保护客户端隐私
4. **智能负载均衡**：多种调度策略，自动故障转移
5. **完整监控**：实时统计，健康检查，告警通知
6. **OAuth 2.0集成**：完整授权流程，自动token刷新
7. **智能密钥管理**：动态密钥选择，健康监控
8. **统一追踪系统**：数据驱动提取，完整请求记录
6. **安全可靠**：TLS加密，证书自动续期，权限控制

**技术栈选择合理**，实施计划详细可行，风险评估全面，为项目成功交付提供了坚实的技术保障。建议严格按照阶段规划推进，重点关注Pingora集成和性能优化，确保系统稳定可靠运行。
