# AI 代理系统完整架构设计与详细设计文档

## 文档版本信息

| 版本 | 日期    | 作者       | 变更说明                                       |
| ---- | ------- | ---------- | ---------------------------------------------- |
| 1.0  | 2024-12 | 系统架构师 | 初始版本                                       |
| 2.0  | 2025-08 | 系统架构师 | 生产版本发布，数据驱动架构完成，前端技术栈更新 |

---

## 目录

1. [项目概述](#1-项目概述)
2. [系统架构设计](#2-系统架构设计)
3. [数据库详细设计](#3-数据库详细设计)
4. [核心模块详细设计](#4-核心模块详细设计)
5. [API 接口详细设计](#5-api接口详细设计)
6. [安全设计](#6-安全设计)
7. [性能与监控设计](#7-性能与监控设计)
8. [部署设计](#8-部署设计)
9. [测试策略](#9-测试策略)
10. [项目实施计划](#10-项目实施计划)

---

## 1. 项目概述

### 1.1 项目目标

构建一个企业级 AI 服务代理平台，为用户提供统一的 AI 服务访问接口，支持多个主流 AI 服务提供商，具备负载均衡、监控统计、安全防护等完整功能。

### 1.2 核心功能

**用户管理系统**

- 用户注册、登录、权限管理
- 基于 JWT 的无状态认证

**API 密钥管理**

- 用户对外 API 密钥（每种服务商类型只能创建一个）
- 内部代理商 API 密钥池（每种类型可创建多个，组成号池）
- 密钥的增删改查和状态管理

**智能负载均衡**

- 轮询调度：按顺序轮流分配请求到各个上游服务器
- 权重调度：根据权重比例分配请求到上游服务器
- 健康优选：优先选择健康状态最佳的上游服务器

**多 AI 服务商支持**

- OpenAI ChatGPT API 完全兼容
- Google Gemini API 支持
- Anthropic Claude API 支持
- 统一接口格式转换

**监控与统计**

- 实时请求统计（成功/失败/响应时间）
- API 健康状态监控
- Token 使用量统计
- 错误日志记录

**安全防护**

- TLS 加密传输
- 证书自动续期
- 源信息隐藏
- 请求重试机制

### 1.3 技术栈

**后端技术栈**

- **核心框架**: Rust 2024 Edition + Pingora 0.6.0 (代理服务) + Axum 0.8.4 (管理服务)
- **数据库**: SQLite + Sea-ORM 1.x
- **缓存**: Redis + Moka (内存缓存)
- **并发工具**: DashMap, Tokio-Util (DelayQueue)
- **HTTP 客户端**: Reqwest
- **错误处理**: thiserror + anyhow
- **TLS**: rustls + acme-lib

**前端技术栈**

- **框架**: React 18 + TypeScript
- **UI 库**: shadcn/ui + Radix UI
- **构建工具**: ESBuild (通过自定义脚本 `scripts/build.mjs`)
- **状态管理**: Zustand
- **路由**: React Router
- **样式**: Tailwind CSS

### 1.4 系统特色

**双端口分离架构**: Pingora 专注 AI 代理(8080)，Axum 专注管理 API(9090)
**职责清晰分离**: 代理服务专注性能，管理服务专注业务逻辑
**完全数据驱动设计**: 所有配置存储在数据库中，支持动态更新无需重启
**智能字段提取**: 基于数据库配置的 TokenFieldExtractor 和 ModelExtractor
**源信息隐藏**: 完全隐藏客户端信息，AI 服务商只能看到代理服务器信息
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
        AuthService["认证服务<br/>(JWT, API Key, OAuth2)"]
        ApiKeySchedulerService["API密钥调度服务<br/>(智能选择, 负载均衡)"]
        ApiKeyHealthService["API密钥健康检查服务<br/>(主动探测, 故障恢复)"]
        RateLimiter["分布式速率限制器<br/>(QPS, 每日配额)"]
        CollectService["CollectService<br/>请求统计采集"]
        TraceManager["TraceManager<br/>追踪与副作用"]
        OAuthClient["OAuth客户端<br/>(授权, Token交换, 自动刷新)"]
        OAuthTokenRefreshTask["OAuth Token刷新后台任务"]
    end

    subgraph "数据持久层 (Data Layer)"
        Database[(SQLite / PostgreSQL)]
        Cache[(Redis / Moka)]
    end

    subgraph "上游服务商 (Upstream Providers)"
        Provider[OpenAI, Gemini, Claude, etc.]
    end

    Client -- "AI 请求" --> ProxyServer
    Client -- "管理操作" --> MgmtServer

    ProxyServer -- "调用" --> AuthService
    ProxyServer -- "调用" --> ApiKeySchedulerService
    ProxyServer -- "调用" --> RateLimiter
    ProxyServer -- "调用" --> CollectService
    ProxyServer -- "调用" --> TraceManager

    MgmtServer -- "调用" --> AuthService
    MgmtServer -- "调用" --> ApiKeySchedulerService
    MgmtServer -- "调用" --> OAuthClient
    MgmtServer -- "读/写" --> Database
    MgmtServer -- "读/写" --> Cache

    AuthService -- "使用" --> OAuthClient
    AuthService -- "使用" --> RateLimiter

    ApiKeySchedulerService -- "依赖" --> ApiKeyHealthService
    ApiKeySchedulerService -- "使用" --> OAuthClient
    ApiKeySchedulerService -- "读/写" --> Database
    ApiKeySchedulerService -- "读/写" --> Cache

    ApiKeyHealthService -- "更新" --> Database
    ApiKeyHealthService -- "更新" --> Cache
    ApiKeyHealthService -- "触发" --> OAuthTokenRefreshTask

    RateLimiter -- "读/写" --> Cache
    RateLimiter -- "读" --> Database

    OAuthClient -- "读/写" --> Database
    OAuthClient -- "读/写" --> Cache

    OAuthTokenRefreshTask -- "使用" --> OAuthClient
    OAuthTokenRefreshTask -- "读/写" --> Database

    TraceManager -- "写入" --> Database

    ProxyServer -- "转发请求至" --> Provider
```

### 2.2 核心设计原则

**双端口分离原则**

- Pingora(8080)专注 AI 代理：负载均衡、限速、熔断、请求转发
- Axum(9090)专注管理功能：用户管理、API 管理、统计查询、配置管理
- TLS 证书管理统一处理，两个服务共享

**职责专业化原则**

- 代理服务追求极致性能和稳定性
- 管理服务提供丰富的业务功能
- 认证服务层统一共享
- 数据存储层统一管理

**故障隔离原则**

- 管理服务故障不影响 AI 代理功能
- 代理服务故障不影响管理操作
- 独立扩展和部署能力
- 分离的监控和告警体系

**安全优先原则**

- 端到端加密传输
- 完全隐藏源请求信息
- 分层安全策略：代理端口对外，管理端口内网
- 最小权限原则
- **PKCE (Proof Key for Code Exchange)**: 为 OAuth 公共客户端提供额外的安全保护，防止授权码拦截攻击。

**代理透明原则**

- Pingora 代理服务保持最大透明性：只做路径匹配、认证、转发
- 不解析具体 URI 参数和业务逻辑，完整保持客户端请求
- 所有 AI API 的具体处理逻辑交给上游服务商
- 避免过度复杂的路由判断，确保高性能和兼容性

**智能凭证管理原则**

- **OAuth Token 生命周期管理**: 自动刷新即将过期的 OAuth Access Token，确保服务连续性。
- **智能 API 密钥选择**: 综合考虑密钥的健康状态、认证状态、过期时间、速率限制等因素，动态选择最优密钥。
- **数据驱动配置**: OAuth 提供商配置、认证头解析格式等均从数据库动态加载，支持热更新。

**主动健康监控与故障恢复原则**

- **API 密钥健康检查**: 定期主动探测 API 密钥的可用性和性能，及时发现并隔离故障密钥。
- **错误分类与健康评分**: 细致分类 API 密钥错误类型，并根据检查结果计算健康分数，为调度提供依据。
- **自动限流恢复**: 自动识别并调度恢复因速率限制而暂时不可用的 API 密钥。

**分布式速率限制原则**

- **实时 QPS 限制**: 基于 Redis 实现分布式、高精度的每分钟请求速率限制。
- **每日配额管理**: 结合数据库追踪和缓存加速，实现每日 Token 和成本配额限制。
- **缓存与数据库协同**: 缓存用于实时计数，数据库用于持久化和精确统计。

### 2.3 数据流设计

**AI 代理请求流程（端口 8080）** - 协调器与管线模式

```
Client → Pingora(8080) → RequestFilter Pipeline → Upstream → Response → Logging & Tracing
```

**管线步骤 (Pipeline Steps)**:

1.  **客户端 API 密钥认证 (Client API Key Authentication)**: `AuthService` 验证客户端提供的 `user_service_apis` API 密钥。
2.  **追踪启动 (Start Trace)**: `TraceManager` 初始化追踪记录。
3.  **分布式速率限制 (Distributed Rate Limit)**: `RateLimiter` 检查并应用 QPS、每日请求、Token 和成本限制。
4.  **配置加载 (Load Config)**: 读取数据库中的服务商配置，填充 `ctx.provider_type`。
5.  **智能后端 API 密钥选择 (Intelligent Backend API Key Selection)**: `ApiKeySchedulerService` 综合考虑健康状态、认证状态、过期时间、速率限制等因素，通过 `ApiKeySelectService` 获取有效的后端 API 密钥（或 OAuth Token）。
6.  **追踪更新 (Update Trace)**: `TraceManager` 更新追踪记录，关联后端密钥。

**OAuth 授权与 Token 刷新流程**

```
Client → OAuthClient.start_authorization() → OAuth Provider (用户授权) → OAuthClient.poll_session() → OAuthClient.exchange_token() → OAuthClient.get_valid_access_token() (自动刷新)
```

1.  **授权请求**: 客户端通过 `OAuthClient.start_authorization()` 获取授权 URL，用户在 OAuth Provider 完成授权。
2.  **轮询状态**: 客户端通过 `OAuthClient.poll_session()` 定期查询授权状态。
3.  **Token 交换**: OAuth Provider 返回授权码后，`OAuthClient.exchange_token()` 将授权码交换为 Access Token 和 Refresh Token。
4.  **Token 存储**: Access Token 和 Refresh Token 存储在 `oauth_client_sessions` 表中。
5.  **自动刷新**: `ApiKeyRefreshService` (被动) 和 `OAuthTokenRefreshTask` (主动) 监控 Access Token 的过期时间，并在即将过期时自动使用 Refresh Token 进行刷新。
6.  **凭证获取**: `ApiKeySelectService` 在代理请求时，智能地从缓存或通过 `ApiKeyRefreshService` 获取有效的 Access Token。

**管理 API 请求流程（端口 9090）**

```
Client → Axum(9090) → Auth → Business Logic → Database/Redis → Response
```

**双服务协同数据同步**

```
配置变更: Axum管理服务 → Database → Redis缓存 → Pingora代理服务自动更新
健康状态: Pingora代理服务 → Database/Redis → Axum管理服务监控展示
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

    // 步骤 1: 客户端API密钥认证
    // 验证客户端提供的 user_service_apis API密钥
    let user_service_api = self.auth_service.authenticate_user_service_api(ctx.client_api_key.as_ref().unwrap()).await?;
    ctx.set_user_service_api(user_service_api);

    // 步骤 2: 启动追踪
    self.tracing_service.start_trace(session, ctx).await?;

    // 步骤 3: 分布式速率限制
    // 检查并应用QPS、每日请求、Token和成本限制
    self.rate_limiter.check_rate_limit(ctx).await?;

    // 步骤 4: 读取 Provider 配置（认证阶段已填充 ctx.provider_type）
    let provider = ctx.provider_type.as_ref().ok_or_else(|| crate::error!(Internal, "missing provider metadata"))?;

    // 步骤 5: 智能后端API密钥选择
    // 综合考虑健康状态、认证状态、过期时间、速率限制等因素，通过 ApiKeySelectService 获取有效的后端API密钥（或OAuth Token）
    let selection_result = self.api_key_scheduler_service.select_api_key_from_service_api(ctx.user_service_api.as_ref().unwrap(), ctx.selection_context()).await?;
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
    // 从上下文中获取 provider_type 配置
    let provider = ctx.provider_type.as_ref().ok_or_else(|| ...)?;

    // 使用 Provider 的 base_url 构建上游地址
    let upstream_addr = format!("{}:443", provider.base_url);

    // 创建一个启用 TLS 的上游对等点
    let peer = HttpPeer::new(upstream_addr, true, provider.base_url.clone());

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
- 使用 WAL 模式提高并发性能
- 定期备份和恢复机制

**缓存层：Redis**

- 存储临时数据和热点数据
- API 健康状态缓存
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

#### 3.2.2 AI 服务商配置表

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

#### 3.2.3 用户 API 密钥管理表

````sql
-- 用户的内部代理商API密钥池（号池）
CREATE TABLE user_provider_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    provider_type_id INTEGER NOT NULL,
    api_key VARCHAR(255) NOT NULL,       -- 实际的AI服务商API密钥 或 OAuth Session ID
    name VARCHAR(100) NOT NULL,          -- 用户给这个密钥起的名字
    weight INTEGER DEFAULT 1,            -- 权重（用于权重调度）
    max_requests_per_minute INTEGER DEFAULT 100, -- 每分钟最大请求数
    max_tokens_prompt_per_minute INTEGER DEFAULT 100000, -- 每分钟最大Token提示数
    max_requests_per_day INTEGER DEFAULT 1000000,  -- 每天最大请求数
    used_tokens_today INTEGER DEFAULT 0,         -- 今日已使用token数
    last_used DATETIME,                  -- 最后使用时间
    is_active BOOLEAN DEFAULT TRUE,
    auth_type VARCHAR(20) DEFAULT 'api_key',     -- 认证类型: 'api_key' 或 'oauth'
    auth_status VARCHAR(20) DEFAULT 'authorized',-- OAuth会话状态: pending, authorized, expired, error, revoked
    expires_at DATETIME,                         -- OAuth token过期时间
    health_status VARCHAR(20) DEFAULT 'healthy', -- 健康状态: healthy, unhealthy, rate_limited
    health_status_detail TEXT,                   -- 健康状态详情 (JSON)
    rate_limit_resets_at DATETIME,               -- 限流重置时间
    last_error_time DATETIME,                    -- 最后错误时间
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    UNIQUE(user_id, provider_type_id, name), -- 同一用户同一服务商下名称不能重复
    INDEX idx_user_provider (user_id, provider_type_id),
    INDEX idx_active_keys (is_active, provider_type_id),
    INDEX idx_health_status (health_status, rate_limit_resets_at)
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

-- OAuth客户端会话表 (新增)
CREATE TABLE oauth_client_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id VARCHAR(255) UNIQUE NOT NULL,    -- 唯一会话ID，作为user_provider_keys.api_key
    user_id INTEGER NOT NULL,
    provider_name VARCHAR(50) NOT NULL,
    provider_type_id INTEGER,                   -- 对应的provider_types.id
    code_verifier VARCHAR(255) NOT NULL,        -- PKCE code verifier
    code_challenge VARCHAR(255) NOT NULL,       -- PKCE code challenge
    state VARCHAR(255) NOT NULL,                -- 用于防CSRF，与授权码流程的state参数一致
    name VARCHAR(100) NOT NULL,                 -- 会话名称
    description TEXT,
    access_token TEXT,                          -- 访问令牌
    refresh_token TEXT,                         -- 刷新令牌
    id_token TEXT,                              -- ID令牌（OpenID Connect）
    token_type VARCHAR(50),                     -- 令牌类型 (e.g., Bearer)
    expires_in INTEGER,                         -- 访问令牌过期秒数
    expires_at DATETIME,                        -- 访问令牌过期时间
    scope TEXT,                                 -- 授权范围
    status VARCHAR(20) NOT NULL,                -- 会话状态 (pending, authorized, failed, expired, error, revoked)
    error_message TEXT,                         -- 错误消息
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,                      -- 授权完成时间
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    INDEX idx_session_id (session_id),
    INDEX idx_user_provider_session (user_id, provider_name),
    INDEX idx_oauth_expires_at (expires_at),
    INDEX idx_oauth_status (status)
);```

#### 3.2.4 监控统计表

-- API健康状态表 (已废弃，健康状态已集成到 user_provider_keys 表)
-- CREATE TABLE api_health_status (...)

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
    updated_at DATETIME DEFAULT CURRENT_T

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
````

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
│   ├── auth/                     # 认证授权模块
│   │   ├── api_key.rs            # API密钥管理
│   │   ├── jwt.rs                # JWT管理
│   │   ├── oauth_client/         # OAuth客户端实现 (授权码, PKCE, 自动刷新, 轮询)
│   │   ├── api_key_refresh_service.rs # OAuth Token智能刷新服务
│   │   ├── api_key_refresh_task.rs    # OAuth Token刷新后台任务
│   │   ├── api_key_select_service.rs  # 智能API密钥提供者 (统一API Key/OAuth Token获取)
│   │   ├── rate_limit_dist.rs    # 分布式速率限制器
│   │   ├── service.rs            # 认证服务入口
│   │   └── ...                   # 其他认证相关工具和类型
│   ├── proxy/                    # Pingora代理服务 (service.rs, builder.rs)
│   ├── management/               # Axum管理API
│   ├── key_pool/                 # API密钥池调度模块
│   │   ├── algorithms.rs         # 调度算法 (轮询, 权重)
│   │   ├── api_key_health.rs     # API密钥健康检查服务
│   │   ├── pool_manager.rs       # API密钥调度服务 (核心调度逻辑)
│   │   ├── rate_limit_reset_task.rs # 限流重置后台任务
│   │   └── ...                   # 其他密钥池相关类型
│   ├── health/                   # 健康检查 (api_key_health.rs) - 已合并到key_pool
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

### 4.2 Pingora 统一入口服务

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
    let db = Arc::new(init_database(&app_config.database).await?);
    let cache_manager = Arc::new(CacheManager::new(app_config.redis.clone()));

    // OAuth相关服务
    let oauth_provider_manager = Arc::new(OAuthProviderManager::new(db.clone()));
    let oauth_session_manager = Arc::new(SessionManager::new(db.clone()));
    let oauth_token_exchange_client = Arc::new(TokenExchangeClient::new());
    let oauth_client = Arc::new(OAuthClient::new(db.clone())); // 封装了上述OAuth组件

    // API Key刷新服务
    let api_key_refresh_service = Arc::new(ApiKeyRefreshService::new(db.clone(), oauth_client.clone()));

    // API Key选择服务 (智能提供者)
    let api_key_select_service = Arc::new(ApiKeySelectService::new(db.clone(), oauth_client.clone(), api_key_refresh_service.clone()));

    // 健康检查服务
    let api_key_health_service = Arc::new(ApiKeyHealthService::new(db.clone()));

    // 限流重置任务
    let rate_limit_reset_task = Arc::new(RateLimitResetTask::new(db.clone(), api_key_health_service.get_health_status_cache()));
    api_key_health_service.set_rate_limit_reset_sender(rate_limit_reset_task.get_command_sender().await.unwrap()).await;

    // API Key调度服务
    let api_key_scheduler_service = Arc::new(ApiKeySchedulerService::new(db.clone(), api_key_health_service.clone()));
    api_key_scheduler_service.set_smart_provider(api_key_select_service.clone()).await;

    // OAuth Token刷新后台任务
    let oauth_token_refresh_task = Arc::new(OAuthTokenRefreshTask::new(api_key_refresh_service.clone()));

    // 认证服务
    let jwt_manager = Arc::new(JwtManager::new(app_config.auth.clone())?);
    let api_key_manager = Arc::new(ApiKeyManager::new(db.clone(), app_config.auth.clone(), cache_manager.clone(), app_config.cache.clone()));
    let auth_service = Arc::new(AuthService::with_cache(jwt_manager.clone(), api_key_manager.clone(), db.clone(), app_config.auth.clone(), cache_manager.clone()));

    // 分布式速率限制器
    let rate_limiter = Arc::new(RateLimiter::new(cache_manager.clone(), db.clone()));

    // ... 其他服务
    Ok(SharedServices {
        db,
        cache_manager,
        auth_service,
        api_key_scheduler_service,
        api_key_health_service,
        api_key_select_service,
        api_key_refresh_service,
        oauth_token_refresh_task,
        rate_limit_reset_task,
        rate_limiter,
        oauth_client,
        // ...
    })
}
```

#### 4.2.2 统一代理服务

```rust
// src/proxy/service.rs
pub struct ProxyService {
    // 依赖注入所有需要的共享服务
    auth_service: Arc<AuthenticationService>,
    collect_service: Arc<CollectService>,
    trace_manager: Arc<TraceManager>,
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
        // 采集指标并完成追踪
        let status_code = ...; // 根据上下文解析最终状态码
        let metrics = self.collect_service.finalize_metrics(ctx, status_code).await;
        self.trace_manager.record_success(&metrics, ctx).await?;
    }
}
```

### 4.3 AI 代理处理器

(逻辑被分解到 `AuthenticationService`, `ApiKeySelectionService` 等多个服务中，由 `ProxyService` 协调)

### 4.4 API 密钥调度服务 (ApiKeySchedulerService)

**设计原则**: `ApiKeySchedulerService` 是核心的 API 密钥调度器，负责从用户的 API 密钥池中智能选择最合适的密钥。它整合了认证状态、健康检查、速率限制和调度算法，以确保高可用性和性能。

**核心组件**:

- `ApiKeySchedulerService`: 调度服务的主入口，协调密钥选择流程。
- `ApiKeyHealthService`: 提供 API 密钥的实时健康状态。
- `ApiKeySelectService`: 智能凭证提供者，负责获取 API 密钥或 OAuth Access Token，并处理 OAuth Token 的自动刷新。
- `ApiKeySelector` (RoundRobin, Weighted): 具体的调度算法实现。

**密钥选择流程 (`select_api_key_from_service_api`)**:

1.  **获取候选密钥 ID**: 从 `user_service_apis.user_provider_keys_ids` 中解析出用户配置的后端 API 密钥 ID 列表。
2.  **加载活跃密钥**: 从数据库加载这些 ID 对应的 `user_provider_keys` 记录，并过滤掉非活跃密钥。
3.  **初步有效性过滤 (`filter_valid_keys`)**:
    - 检查密钥的 `is_active` 状态。
    - 对于 OAuth 类型的密钥，检查其 `auth_status` (必须为 `authorized`)。
    - 检查密钥的 `expires_at` (确保未过期)。
    - 检查密钥的 `health_status` (排除 `unhealthy` 状态，并处理 `rate_limited` 状态的恢复)。
4.  **健康状态过滤 (`filter_healthy_keys`)**:
    - 调用 `ApiKeyHealthService` 获取所有健康的密钥 ID。
    - 结合密钥自身的 `health_status` 字段（特别是 `rate_limited` 状态下的 `rate_limit_resets_at`），进一步过滤出当前可用的健康密钥。
    - 如果所有密钥都不健康，系统将进入降级模式，使用所有密钥（包括不健康的）进行尝试，以避免完全中断服务。
5.  **凭证获取与刷新**: 对于 OAuth 类型的密钥，`ApiKeySelectService` 会在需要时自动获取或刷新 Access Token，确保调度器总是拿到有效的凭证。
6.  **调度算法选择**: 根据 `user_service_apis.scheduling_strategy` (如 `RoundRobin`, `Weighted`) 选择相应的 `ApiKeySelector`。
7.  **执行调度**: 选定的 `ApiKeySelector` 从过滤后的健康密钥池中，根据其算法（如轮询、权重）选择最终的 API 密钥。

**调度算法 (`src/key_pool/algorithms.rs`)**:

- **轮询调度 (RoundRobin)**: 简单地按顺序循环选择密钥。
- **权重调度 (Weighted)**: 根据密钥配置的权重比例进行选择，权重高的密钥被选中的概率更大。
- **状态管理**: 调度器是有状态的，使用 `DashMap` 维护每个 `(user_service_api_id, route_group)` 的计数器，确保在同一路由组内的请求能够公平或按权重分配。

```rust
// src/key_pool/pool_manager.rs
pub struct ApiKeySchedulerService {
    db: Arc<DatabaseConnection>,
    selectors: tokio::sync::RwLock<HashMap<SchedulingStrategy, Arc<dyn ApiKeySelector>>>,
    api_key_health_service: Arc<ApiKeyHealthService>,
    api_key_select_service: tokio::sync::RwLock<Option<Arc<ApiKeySelectService>>>,
    ready: AtomicBool,
}

impl ApiKeySchedulerService {
    pub async fn select_api_key_from_service_api(
        &self,
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        // 1. 获取候选密钥ID
        let provider_key_ids = Self::get_provider_key_ids(service_api, context)?;
        // 2. 从数据库加载活跃密钥
        let all_candidate_keys = self.load_active_provider_keys(&provider_key_ids, context).await?;

        // 3. 初步有效性过滤 (is_active, auth_status, expires_at, health_status)
        let user_keys = Self::filter_valid_keys_with_logging(&all_candidate_keys, context)?;

        // 4. 健康状态过滤 (结合 ApiKeyHealthService 和本地状态)
        let healthy_keys = self.filter_healthy_keys_with_logging(&user_keys, service_api, context).await;

        // 5. 确定最终用于调度的密钥池 (如果无健康密钥，则降级使用所有有效密钥)
        let keys_to_use = if healthy_keys.is_empty() {
            user_keys.as_slice()
        } else {
            healthy_keys.as_slice()
        };

        // 6. 调度算法选择
        let scheduling_strategy = Self::resolve_strategy(service_api);
        let selector = self.get_selector(scheduling_strategy).await;

        // 7. 执行调度
        selector.select_key(keys_to_use, context).await
    }
}

// src/key_pool/algorithms.rs
#[async_trait::async_trait]
pub trait ApiKeySelector: Send + Sync {
    async fn select_key(
        &self,
        keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult>;
    fn name(&self) -> &'static str;
    async fn reset(&self);
}

pub struct RoundRobinApiKeySelector { /* ... */ }
impl ApiKeySelector for RoundRobinApiKeySelector { /* ... */ }

pub struct WeightedApiKeySelector { /* ... */ }
impl ApiKeySelector for WeightedApiKeySelector { /* ... */ }
```

### 4.5 API 密钥健康检查服务 (ApiKeyHealthService)

**设计原则**: `ApiKeyHealthService` 负责对 API 密钥进行主动健康检查，评估其可用性和性能，并根据检查结果更新密钥的健康状态。它集成了错误分类、健康评分和与数据库的同步机制，确保调度器能够获取到最准确的密钥健康信息。

**核心组件**:

- `ApiKeyHealthService`: 健康检查服务的主入口。
- `ApiKeyHealth`: 内存中存储的单个 API 密钥的健康状态详情。
- `ApiKeyCheckResult`: 单次健康检查的结果，包含时间戳、成功状态、响应时间、状态码、错误信息和错误分类。
- `ApiKeyErrorCategory`: 错误分类枚举，用于识别不同类型的 API 密钥故障（如 `InvalidKey`, `QuotaExceeded`, `NetworkError` 等）。
- `RateLimitResetTask`: 后台任务，用于自动重置因速率限制而暂时不可用的密钥。

**健康检查流程**:

1.  **启动时加载**: 服务启动时，从数据库加载所有 API 密钥的健康状态到内存 `health_status` 缓存。
2.  **主动探测**: `ApiKeyHealthService` 会定期对所有活跃的 API 密钥执行主动探测。
    - **检查方式**: 根据 `provider_types` 配置的 `health_check_path` 和 `api_format`，构建针对不同 AI 服务商的健康检查请求（例如，OpenAI 的 `/models` GET 请求，Claude 的 `/messages` POST 请求）。
    - **认证头**: 根据服务商类型，使用正确的认证头（例如，Gemini 使用 `X-goog-api-key`，其他使用 `Authorization: Bearer`）。
    - **超时**: 请求设置超时，避免长时间阻塞。
3.  **结果处理**:
    - 记录响应时间、HTTP 状态码和成功/失败状态。
    - **错误分类**: 将失败的请求错误分类为 `InvalidKey`, `QuotaExceeded`, `NetworkError`, `ServerError` 等。
    - **更新内存状态**: 更新内存中 `ApiKeyHealth` 对象的连续成功/失败计数、平均响应时间、最后错误信息和最近检查结果历史。
    - **健康分数计算**: 根据成功率、响应时间惩罚和连续失败惩罚，计算 `health_score` (0-100)。
    - **同步到数据库**: 将更新后的健康状态（`health_status`, `health_status_detail`, `rate_limit_resets_at`, `last_error_time`）同步到 `user_provider_keys` 表。
4.  **速率限制处理**:
    - 如果健康检查返回 429 状态码，服务会尝试从错误信息中解析 `resets_in_seconds`。
    - 将密钥状态标记为 `rate_limited`，并设置 `rate_limit_resets_at`。
    - 向 `RateLimitResetTask` 发送命令，调度在 `resets_at` 时间后自动将密钥状态重置为 `healthy`。

**关键代码结构**:

```rust
// src/key_pool/api_key_health.rs
pub struct ApiKeyHealthService {
    db: Arc<DatabaseConnection>,
    client: Client,
    health_status: Arc<RwLock<HashMap<i32, ApiKeyHealth>>>,
    rate_limit_reset_sender: Arc<RwLock<Option<mpsc::Sender<ScheduleResetCommand>>>>,
    // ... 配置参数
}

impl ApiKeyHealthService {
    // 检查单个API密钥的健康状态
    pub async fn check_api_key(&self, key_model: &user_provider_keys::Model) -> Result<ApiKeyCheckResult> { /* ... */ }

    // 基于数据库配置检查API密钥 (执行实际HTTP请求)
    async fn check_provider_key(&self, api_key: &str, provider_info: &provider_types::Model) -> Result<(u16, bool)> { /* ... */ }

    // 分类错误类型
    fn categorize_error(error: &crate::error::ProxyError) -> ApiKeyErrorCategory { /* ... */ }

    // 更新API密钥健康状态 (内存和数据库)
    async fn update_health_status(
        &self,
        key_id: i32,
        provider_type_id: ProviderTypeId,
        check_result: ApiKeyCheckResult,
    ) -> Result<()> { /* ... */ }

    // 同步健康状态到数据库
    async fn sync_health_status_to_database(
        &self,
        key_id: i32,
        status: &ApiKeyHealth,
        check_result: &ApiKeyCheckResult,
    ) -> Result<()> { /* ... */ }

    // 计算健康分数
    fn calculate_health_score(status: &ApiKeyHealth) -> f32 { /* ... */ }

    // 从数据库加载健康状态到内存
    pub async fn load_health_status_from_database(&self) -> Result<()> { /* ... */ }

    // 将密钥标记为速率受限
    pub async fn mark_key_as_rate_limited(
        &self,
        key_id: i32,
        resets_at: Option<chrono::NaiveDateTime>,
        details: &str,
    ) -> Result<()> { /* ... */ }
}
```

### 4.6 管理 API 模块 (Axum 内嵌服务)

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

### 4.7 TLS 证书管理模块

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

## 5. API 接口详细设计

### 5.1 RESTful API 规范

**基础 URL**: `https://your-domain.com/api/v1`

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

### 5.2 认证相关 API

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

### 5.3 用户 API 密钥管理

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

### 5.5 统计和监控 API

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

### 5.6 AI 服务商代理接口

系统需要为每种 AI 服务商提供兼容的 API 接口：

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

**JWT Token 管理**

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
impl JwtManager { // Changed from JwtService to JwtManager
    pub fn generate_token_pair( // Changed from generate_tokens to generate_token_pair
        &self,
        user_id: i32,
        username: String,
        is_admin: bool,
        role: UserRole,
    ) -> Result<TokenPair> { /* ... */ }
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

### 6.2 API 密钥安全

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

### 6.3 PKCE (Proof Key for Code Exchange)

PKCE 是一种 OAuth 2.0 扩展，旨在为公共客户端（如移动应用或单页应用）提供额外的安全性，防止授权码拦截攻击。

**核心原理**:

1.  **Code Verifier 生成**: 客户端生成一个高熵的随机字符串 `code_verifier`。
2.  **Code Challenge 生成**: 客户端使用 `SHA256` 哈希 `code_verifier`，并进行 `base64url` 编码，生成 `code_challenge`。
3.  **授权请求**: 客户端在向授权服务器发起授权请求时，携带 `code_challenge` 和 `code_challenge_method` (S256)。
4.  **令牌交换**: 客户端在用授权码交换 Access Token 时，必须再次发送原始的 `code_verifier`。授权服务器会重新计算 `code_challenge` 并与之前收到的进行比对，验证通过后才颁发 Token。

**实现**:

- `src/auth/oauth_client/pkce.rs` 模块提供了 `PkceVerifier` 和 `PkceChallenge` 结构体，用于生成和验证 PKCE 参数。
- `oauth_client_sessions` 表存储 `code_verifier` 和 `code_challenge`。

```rust
// src/auth/oauth_client/pkce.rs
pub struct PkceVerifier { /* ... */ }
pub struct PkceChallenge { /* ... */ }
pub struct PkceParams {
    pub verifier: PkceVerifier,
    pub challenge: PkceChallenge,
}

impl PkceParams {
    pub fn new() -> Self { /* ... */ }
    pub fn verify(&self) -> Result<bool, PkceError> { /* ... */ }
}
```

### 6.4 OAuth Token 生命周期管理 (自动刷新与清理)

为了确保 OAuth 凭证的持续有效性和安全性，系统实现了智能的 Token 生命周期管理机制。

**核心机制**:

1.  **被动刷新**: `ApiKeySelectService` 在代理请求时，如果发现 OAuth Access Token 即将过期，会触发 `ApiKeyRefreshService` 进行被动刷新。
2.  **主动刷新**: `OAuthTokenRefreshTask` 作为后台任务，定期扫描数据库中即将过期的 OAuth 会话，并主动调用 `ApiKeyRefreshService` 进行刷新。
3.  **刷新锁**: 使用 `tokio::sync::Mutex` 防止同一 OAuth 会话的并发刷新，避免竞态条件。
4.  **孤立会话清理**: `AutoRefreshManager` 会定期检查 OAuth 会话是否仍然被 `user_provider_keys` 引用。对于不再有引用的孤立会话，系统会自动将其删除，避免存储废弃或潜在不安全的凭证。

**实现**:

- `src/auth/api_key_refresh_service.rs`: `ApiKeyRefreshService` 实现了被动和主动刷新逻辑。
- `src/auth/api_key_refresh_task.rs`: `OAuthTokenRefreshTask` 负责后台调度和执行主动刷新。
- `src/auth/oauth_client/auto_refresh.rs`: `AutoRefreshManager` 封装了 Token 刷新和孤立会话清理的核心逻辑。
- `oauth_client_sessions` 表存储 OAuth 会话的 Access Token、Refresh Token 和过期时间。

```rust
// src/auth/api_key_refresh_service.rs
pub struct ApiKeyRefreshService { /* ... */ }
impl ApiKeyRefreshService {
    pub async fn passive_refresh_if_needed(&self, session_id: &str) -> Result<TokenRefreshResult> { /* ... */ }
    pub async fn refresh_token_with_lock(&self, session_id: &str, refresh_type: RefreshType) -> Result<TokenRefreshResult> { /* ... */ }
    pub async fn cleanup_stale_sessions(&self) -> Result<()> { /* ... */ }
}

// src/auth/api_key_refresh_task.rs
pub struct OAuthTokenRefreshTask { /* ... */ }
impl OAuthTokenRefreshTask {
    pub async fn start(&self) -> Result<()> { /* ... */ }
}

// src/auth/oauth_client/auto_refresh.rs
impl AutoRefreshManager {
    async fn validate_session_association(&self, session: &oauth_client_sessions::Model) -> AuthResult<bool> { /* ... */ }
}
```

### 6.5 分布式速率限制

为了保护系统资源和上游 AI 服务商，系统实现了分布式速率限制机制，支持 QPS、每日 Token 和每日成本限制。

**核心机制**:

1.  **Redis 后端**: 利用 Redis 的原子操作（`INCR`）和过期时间（`EXPIRE`）实现高并发、低延迟的分布式计数。
2.  **QPS 限制**: 对每个用户+端点维度，限制每分钟的请求数。
3.  **每日配额**: 对每个用户服务 API，限制每日的总 Token 使用量和总成本。
4.  **缓存预热**: 服务启动时，从数据库加载当日已使用的 Token 和成本数据到 Redis，避免冷启动问题。
5.  **实时更新**: 每次成功请求后，实时更新 Redis 中的 Token 和成本计数。

**实现**:

- `src/auth/rate_limit_dist.rs`: `RateLimiter` 结构体封装了所有速率限制逻辑。
- `CacheManager` 提供底层的 Redis 操作。
- `proxy_tracing` 表用于持久化请求追踪数据，作为每日配额的统计来源。

```rust
// src/auth/rate_limit_dist.rs
pub struct RateLimiter {
    cache: Arc<CacheManager>,
    db: Arc<DatabaseConnection>,
}

impl RateLimiter {
    pub async fn check_per_minute(&self, user_id: i32, endpoint: &str, limit: i64) -> Result<DistRateLimitOutcome> { /* ... */ }
    pub async fn check_daily_token_limit(&self, user_api_id: i32, limit: i64) -> Result<()> { /* ... */ }
    pub async fn check_daily_cost_limit(&self, user_api_id: i32, limit: Decimal) -> Result<()> { /* ... */ }
    pub async fn warmup_daily_usage_cache(&self) -> Result<()> { /* ... */ }
}
```

### 6.6 数据安全

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

```

**缓存策略优化**

- **认证结果缓存**: `UnifiedAuthCacheManager` (基于 Redis/Moka) 缓存 JWT、API Key 和 OAuth 认证结果，减少数据库查询和计算开销。

- **智能凭证内存缓存**: `ApiKeySelectService` 维护一个内存缓存，存储已获取的有效 API 密钥或 OAuth Access Token，避免频繁的数据库/OAuth 服务交互。

- **健康状态缓存**: `ApiKeyHealthService` 在内存中缓存 API 密钥的健康状态，供调度器快速查询。

**分布式速率限制**

- `RateLimiter` 利用 Redis 的原子操作实现高并发、低延迟的 QPS 和每日配额检查，避免数据库成为性能瓶颈。

**并发数据结构**

- `DashMap`: 在调度算法（如 `RoundRobinApiKeySelector`, `WeightedApiKeySelector`）中使用 `DashMap` 维护并发计数器，减少锁竞争，提高并发性能。

**高效任务调度**

- `tokio-util::time::DelayQueue`: 在 `OAuthTokenRefreshTask` 和 `RateLimitResetTask` 中使用 `DelayQueue` 高效调度后台任务，避免频繁的定时器创建和管理开销。

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

**Prometheus 指标导出**

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

### 8.1 Docker 容器化

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
version: "3.8"

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
      - "--config.file=/etc/prometheus/prometheus.yml"
      - "--storage.tsdb.path=/prometheus"
      - "--web.console.libraries=/etc/prometheus/console_libraries"
      - "--web.console.templates=/etc/prometheus/consoles"

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

**Kubernetes 部署清单**

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
// tests/unit/key_pool_test.rs
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_round_robin_key_pool() {
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

        let key_pool = RoundRobinScheduler::new(Arc::new(mock_db), Arc::new(mock_redis));
        let user_api = create_test_user_api();

        let result = key_pool.select_backend(&user_api).await;

        assert!(result.is_ok());
        let selected = result.unwrap();
        assert_eq!(selected.id, 1);
    }

    #[tokio::test]
    async fn test_weighted_key_pool() {
        // 测试权重调度逻辑
        let key_pool = WeightedScheduler::new(mock_db(), mock_redis());
        // ... 测试实现
    }

    #[tokio::test]
    async fn test_health_best_key_pool() {
        // 测试健康度最佳调度逻辑
        let key_pool = HealthBestScheduler::new(mock_db(), mock_redis(), mock_health_checker());
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

**API 集成测试**

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

**使用 Artillery 进行负载测试**

```yaml
# artillery-load-test.yml
config:
  target: "https://localhost"
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

**使用 Playwright 的 E2E 测试**

```javascript
// tests/e2e/user-flow.spec.js
const { test, expect } = require("@playwright/test");

test.describe("Complete User Flow", () => {
  test("user can register, create API, and make proxy request", async ({
    page,
  }) => {
    // 用户注册
    await page.goto("/register");
    await page.fill('[data-testid="username"]', "testuser");
    await page.fill('[data-testid="email"]', "test@example.com");
    await page.fill('[data-testid="password"]', "password123");
    await page.click('[data-testid="register-button"]');

    await expect(page).toHaveURL("/dashboard");

    // 创建API服务
    await page.click('[data-testid="create-api-button"]');
    await page.selectOption('[data-testid="provider-select"]', "openai");
    await page.fill('[data-testid="api-name"]', "My Test API");
    await page.click('[data-testid="submit-api"]');

    // 添加提供商密钥
    await page.click('[data-testid="add-key-button"]');
    await page.fill('[data-testid="provider-key"]', "sk-test123456789");
    await page.fill('[data-testid="key-name"]', "Test Key");
    await page.click('[data-testid="save-key"]');

    // 测试代理请求
    await page.click('[data-testid="test-api-button"]');
    await page.fill('[data-testid="test-message"]', "Hello, test!");
    await page.click('[data-testid="send-test"]');

    await expect(page.locator('[data-testid="test-response"]')).toBeVisible();
    await expect(page.locator('[data-testid="test-response"]')).toContainText(
      "response"
    );
  });

  test("admin can view system metrics", async ({ page }) => {
    // 管理员登录
    await page.goto("/admin/login");
    await page.fill('[data-testid="email"]', "admin@example.com");
    await page.fill('[data-testid="password"]', "admin123");
    await page.click('[data-testid="login-button"]');

    // 查看系统监控
    await page.goto("/admin/monitoring");
    await expect(page.locator('[data-testid="system-status"]')).toContainText(
      "Healthy"
    );
    await expect(
      page.locator('[data-testid="active-connections"]')
    ).toBeVisible();
    await expect(page.locator('[data-testid="request-rate"]')).toBeVisible();
  });
});
```

---

## 10. 项目实施计划

### 10.1 项目阶段规划

#### **第一阶段：基础架构 (6 周)**

**目标**：建立项目基础框架和核心数据层

**第 1-2 周：项目搭建**

- [ ] Rust 项目初始化和依赖配置
- [ ] 项目目录结构搭建
- [ ] CI/CD 流水线配置
- [ ] 开发环境 Docker 化
- [ ] 代码规范和 lint 配置

**第 3-4 周：数据库设计与实现**

- [ ] SQLite 数据库结构设计
- [ ] Sea-ORM 实体模型创建
- [ ] 数据库迁移脚本编写
- [ ] Redis 缓存层集成
- [ ] 基础 CRUD 操作实现

**第 5-6 周：认证系统**

- [ ] JWT 认证服务实现
- [ ] 用户注册/登录功能
- [ ] 密码加密和验证
- [ ] 认证中间件开发
- [ ] 单元测试编写

**里程碑检查**：

- [x] 用户可以注册和登录
- [x] 数据库连接正常工作
- [x] 基础测试用例通过
- [x] CI/CD 流水线运行成功

---

#### **第二阶段：Pingora 集成与代理核心 (8 周)**

**目标**：实现统一入口和 AI 服务代理功能

**第 7-9 周：Pingora 服务集成**

- [ ] Pingora 框架集成
- [ ] 统一入口服务实现
- [ ] 路由分发机制
- [ ] 请求上下文管理
- [ ] 基础 HTTP 代理功能

**第 10-12 周：AI 服务商适配**

- [ ] OpenAI API 接口适配
- [ ] Gemini API 接口适配
- [ ] Claude API 接口适配
- [ ] 请求/响应格式转换
- [ ] 上游服务连接管理

**第 13-14 周：负载均衡实现**

- [ ] 轮询调度器
- [ ] 权重调度器
- [ ] 健康度最佳调度器
- [ ] 调度策略切换
- [ ] 故障转移机制

**里程碑检查**：

- [x] 可以成功代理转发到三个 AI 服务商
- [x] 负载均衡策略正常工作
- [x] 支持故障自动切换
- [x] 基本的请求统计功能

---

#### **第三阶段：管理功能与监控 (6 周)**

**目标**：完成用户管理界面和监控系统

**第 15-17 周：Axum 管理 API**

- [ ] 内嵌 Axum 服务实现
- [ ] 用户 API 密钥管理接口
- [ ] 提供商密钥池管理接口
- [ ] 统计数据查询接口
- [ ] API 文档生成

**第 18-20 周：健康检查与监控**

- [ ] 健康检查服务实现
- [ ] 后台定时任务
- [ ] 统计数据收集
- [ ] Prometheus 指标导出
- [ ] 告警机制实现

**里程碑检查**：

- [x] 完整的管理 API 功能
- [x] 健康检查正常工作
- [x] 监控数据可查看
- [x] 告警通知正常

---

#### **第四阶段：安全与 TLS (4 周)**

**目标**：实现安全传输和证书管理

**第 21-22 周：TLS 证书管理**

- [ ] TLS 证书管理器
- [ ] Let's Encrypt 集成
- [ ] 自动证书续期
- [ ] SNI 多域名支持

**第 23-24 周：安全强化**

- [ ] 源信息隐藏机制
- [ ] 速率限制实现
- [ ] 安全头部设置
- [ ] 数据加密存储

**里程碑检查**：

- [x] HTTPS 正常工作
- [x] 证书自动续期
- [x] 安全防护到位
- [x] 通过安全测试

---

#### **第五阶段：前端开发 (6 周)**

**目标**：构建完整的 Web 管理界面

**第 25-27 周：Vue 项目搭建**

- [ ] Vue 3 + Element Plus 搭建
- [ ] 路由和状态管理
- [ ] API 客户端封装
- [ ] 组件库搭建

**第 28-30 周：核心页面开发**

- [ ] 登录注册页面
- [ ] 仪表板页面
- [ ] API 密钥管理页面
- [ ] 统计监控页面
- [ ] 系统设置页面

**里程碑检查**：

- [x] 完整的前端管理界面
- [x] 用户体验友好
- [x] 响应式设计
- [x] 前后端联调成功

---

#### **第六阶段：测试与优化 (4 周)**

**目标**：全面测试和性能优化

**第 31-32 周：测试完善**

- [ ] 单元测试补充
- [ ] 集成测试编写
- [ ] E2E 测试实现
- [ ] 负载测试执行

**第 33-34 周：性能优化与部署**

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

**核心团队 (4-5 人)**

- **后端工程师 (2 人)**

  - Rust 开发经验 > 2 年
  - 熟悉异步编程和网络编程
  - 了解代理服务器原理
  - 负责 Pingora 集成和代理逻辑

- **全栈工程师 (1 人)**

  - Rust + Vue 开发经验
  - 熟悉 API 设计和前端开发
  - 负责管理 API 和前端界面

- **DevOps 工程师 (1 人)**

  - Docker 和 Kubernetes 经验
  - 熟悉 CI/CD 流程
  - 负责部署和运维

- **测试工程师 (1 人，可选)**
  - 自动化测试经验
  - 性能测试和安全测试
  - 负责测试策略和执行

### 10.3 风险评估与应对

#### **技术风险**

| 风险                     | 影响度 | 概率 | 应对措施                      |
| ------------------------ | ------ | ---- | ----------------------------- |
| Pingora 集成复杂度超预期 | 高     | 中   | 预研 POC，准备 hyper 替代方案 |
| AI 服务商 API 变更       | 中     | 高   | 接口版本管理，适配器模式      |
| TLS 证书续期失败         | 高     | 低   | 多重备份，手动证书支持        |
| 高并发性能问题           | 高     | 中   | 早期压力测试，架构优化        |

#### **项目风险**

| 风险           | 影响度 | 概率 | 应对措施                     |
| -------------- | ------ | ---- | ---------------------------- |
| 关键人员离职   | 高     | 中   | 知识文档化，技能交叉培训     |
| 需求变更频繁   | 中     | 高   | 敏捷开发，需求优先级管理     |
| 第三方依赖风险 | 中     | 中   | 依赖版本锁定，替代方案调研   |
| 安全漏洞发现   | 高     | 中   | 代码审查，安全测试，及时修补 |

### 10.4 质量保证措施

#### **代码质量**

- 代码审查(Code Review)必须通过
- 测试覆盖率要求 > 80%
- 自动化代码格式检查
- 静态代码分析

#### **性能标准**

- API 响应时间 < 500ms (P95)
- 代理转发延迟 < 100ms
- 支持并发连接 > 10,000
- 可用性 > 99.9%

#### **安全标准**

- 所有敏感数据加密存储
- 通过 OWASP 安全测试
- 定期安全漏洞扫描
- 渗透测试通过

### 10.5 发布计划

#### **Alpha 版本 (第 20 周)**

- 核心代理功能
- 基础管理 API
- 内部测试使用

#### **Beta 版本 (第 28 周)**

- 完整功能实现
- 前端管理界面
- 小范围用户测试

#### **正式版本 (第 34 周)**

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

- 验证 Rust 1.75+版本环境
- 配置 VS Code + rust-analyzer 开发工具链
- 设置 clippy、rustfmt 代码质量工具
- 配置 cargo audit 安全审计工具

**关键输出**:

- 标准化的 Rust 开发环境
- 代码质量检查配置
- 安全审计流程

#### 🟢 项目目录结构

**优先级**: 高  
**状态**: 已完成

**核心描述**:

- 按照 DESIGN.md 创建完整的目录结构
- 建立 src/、migration/、entity/、frontend/等核心目录
- 定义模块化的代码组织方式

**关键输出**:

- 清晰的项目目录结构
- 模块化的代码组织
- 便于维护的架构布局

#### 🟢 错误处理框架

**优先级**: 高  
**状态**: 已完成

**核心描述**:

- 使用 thiserror + anyhow 构建错误处理体系
- 定义 ProxyError、DatabaseError 等专用错误类型
- 实现统一的错误传播和处理机制

**关键输出**:

- 完整的错误类型定义 (`src/error/`)
- 统一的错误处理接口
- 结构化错误信息

#### 🟢 数据库实体定义

**优先级**: 高  
**状态**: 已完成

**核心描述**:

- 使用 Sea-ORM 定义 8 个核心表的实体模型
- 建立用户、API 密钥、统计等核心实体
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

- 实现 API 密钥验证机制
- 支持 JWT 令牌处理
- 基于角色的权限检查(RBAC)

**关键输出**:

- 认证服务 (`src/auth/`)
- JWT 管理器
- 权限检查系统

**技术实现**:

- 支持 Bearer Token、API Key、Basic Auth
- 17 种权限类型，7 种预定义角色
- 数据库集成的 API 密钥管理
- 缓存优化的认证性能

#### ⚪ 动态配置管理系统

**优先级**: 中  
**状态**: 待重构

**说明**:

- 旧的 `ProviderConfigManager` 已移除，后续将直接基于 `provider_types` 表按需查询
- 如需高级缓存/热更新能力，将在未来迭代中重新规划

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

#### ⚪ AI 服务商适配器

**优先级**: 高  
**状态**: 待完成

**核心描述**:

- 实现 OpenAI API 格式标准化
- Google Gemini 适配器实现
- Anthropic Claude 适配器实现

**待实现功能**:

- Chat Completions API 适配
- 流式响应处理
- 模型参数转换
- 错误处理和重试

### 11.4 管理和监控阶段 📊

#### ⚪ Axum 管理 API

**优先级**: 中  
**状态**: 待完成

**核心描述**:

- 实现内嵌 HTTP 服务
- 设计 RESTful API 接口
- 提供管理功能端点

**待实现功能**:

- 用户管理 API
- API 密钥管理 API
- 统计查询 API
- 系统配置 API

#### ⚪ 监控统计系统

**优先级**: 中  
**状态**: 待完成

**核心描述**:

- 实时数据收集和聚合
- 性能指标监控
- Token 使用统计和分析

**待实现功能**:

- 实时指标收集
- 历史数据分析
- 性能监控面板
- 告警规则配置

### 11.5 前端界面开发 🎨

#### ✅ React 18 前端应用

**优先级**: 高  
**状态**: 已完成

**核心描述**:

- [x] 搭建 React 18 + TypeScript 技术栈
- [x] 集成 ESBuild 构建工具和 shadcn/ui
- [x] 使用 Zustand 状态管理和 React Router 7 路由

**已实现功能**:

- [x] 现代化前端架构和主题系统
- [x] 响应式用户界面和移动端适配
- [x] 组件化开发(基于 shadcn/ui)
- [x] TypeScript 类型安全保障

### 11.6 当前状态总结

#### 📊 任务统计

- ✅ **已完成**: 6 个核心任务 (基础设施 + 核心认证 + 动态配置)
- 🔄 **重构中**: 1 个任务 (架构升级)
- ⏳ **待完成**: 15 个主要任务

#### 🎯 当前重点任务

1. **负载均衡调度器** - 实现多种调度算法
2. **AI 服务商适配器** - 完成主要 AI 提供商适配
3. **健康检查系统** - 建立服务监控机制
4. **请求转发处理** - 完善代理转发逻辑

#### 📈 重要里程碑

- **Phase 0 完成**: 动态配置系统替代硬编码地址 ✅
- **Phase 1 完成**: 基础认证和权限系统 ✅
- **Phase 2 目标**: 完整的 AI 代理核心功能
- **Phase 3 目标**: 管理界面和监控系统

#### 🔄 下一步行动

1. **立即开始**: 负载均衡器核心算法实现
2. **并行开发**: OpenAI 适配器和健康检查系统
3. **重点测试**: 端到端代理功能验证
4. **性能验证**: 高并发场景压力测试

---

## 12. 新增核心功能详细设计

### 12.1 OAuth 2.0 授权系统设计

#### 12.1.1 系统架构

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Client App    │    │  AI Proxy       │    │  OAuth Provider │
│                 │    │  (OAuthClient)  │    │                 │
│ • 用户界面       │    │ • 客户端ID       │    │ • 授权服务器     │
│ • 重定向处理     │    │ • PKCE管理       │    │ • 令牌端点       │
│ • Token存储     │    │ • 会话管理       │    │ • 用户信息       │
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
                                │◀───────────────────────┘ (后台/被动)
```

#### 12.1.2 数据库设计

OAuth 会话信息存储在 `oauth_client_sessions` 表中。该表记录了授权流程中的关键数据，包括 PKCE 参数、Access Token、Refresh Token 和会话状态。

```sql
-- OAuth客户端会话表
CREATE TABLE oauth_client_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id VARCHAR(255) UNIQUE NOT NULL,    -- 唯一会话ID，作为user_provider_keys.api_key
    user_id INTEGER NOT NULL,
    provider_name VARCHAR(50) NOT NULL,
    provider_type_id INTEGER,                   -- 对应的provider_types.id
    code_verifier VARCHAR(255) NOT NULL,        -- PKCE code verifier
    code_challenge VARCHAR(255) NOT NULL,       -- PKCE code challenge
    state VARCHAR(255) NOT NULL,                -- 用于防CSRF，与授权码流程的state参数一致
    name VARCHAR(100) NOT NULL,                 -- 会话名称
    description TEXT,
    access_token TEXT,                          -- 访问令牌
    refresh_token TEXT,                         -- 刷新令牌
    id_token TEXT,                              -- ID令牌（OpenID Connect）
    token_type VARCHAR(50),                     -- 令牌类型 (e.g., Bearer)
    expires_in INTEGER,                         -- 访问令牌过期秒数
    expires_at DATETIME,                        -- 访问令牌过期时间
    scope TEXT,                                 -- 授权范围
    status VARCHAR(20) NOT NULL,                -- 会话状态 (pending, authorized, failed, expired, error, revoked)
    error_message TEXT,                         -- 错误消息
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,                      -- 授权完成时间
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id),
    INDEX idx_session_id (session_id),
    INDEX idx_user_provider_session (user_id, provider_name),
    INDEX idx_oauth_expires_at (expires_at),
    INDEX idx_oauth_status (status)
);
```

**注意**: 早期设计中曾考虑 `oauth_token_refresh_tasks` 表，但当前实现中，Token 刷新任务的调度是基于 `tokio-util::time::DelayQueue` 在内存中进行的，并结合 `oauth_client_sessions` 和 `user_provider_keys` 表的状态进行管理，无需独立的任务表。

#### 12.1.3 核心组件设计

**OAuth 客户端 (OAuthClient)**
`OAuthClient` 是 OAuth 模块的入口，封装了与 OAuth Provider 交互的所有逻辑，包括授权流程的启动、会话状态的轮询、Token 的交换和刷新。

```rust
// src/auth/oauth_client/mod.rs
pub struct OAuthClient {
    provider_manager: OAuthProviderManager,
    session_manager: SessionManager,
    polling_client: OAuthPollingClient,
    token_exchange_client: TokenExchangeClient,
    auto_refresh_manager: AutoRefreshManager, // 智能自动刷新管理器
}

impl OAuthClient {
    // 创建新的OAuth客户端
    pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self { /* ... */ }

    // 开始OAuth授权流程，返回授权URL和会话ID
    pub async fn start_authorization(
        &self,
        user_id: i32,
        provider_name: &str,
        name: &str,
        description: Option<&str>,
    ) -> AuthResult<AuthorizeUrlResponse> { /* ... */ }

    // 轮询会话状态
    pub async fn poll_session(&self, session_id: &str) -> AuthResult<OAuthPollingResponse> { /* ... */ }

    // 完成Token交换
    pub async fn exchange_token(
        &self,
        session_id: &str,
        authorization_code: &str,
    ) -> AuthResult<OAuthTokenResponse> { /* ... */ }

    // 智能获取有效的访问令牌 (自动刷新)
    pub async fn get_valid_access_token(&self, session_id: &str) -> AuthResult<Option<String>> { /* ... */ }

    // 批量刷新用户的即将过期token
    pub async fn refresh_user_expiring_tokens(
        &self,
        user_id: i32,
        policy: Option<RefreshPolicy>,
    ) -> AuthResult<Vec<(String, AuthResult<OAuthTokenResponse>)>> { /* ... */ }
}
```

**OAuth Token 智能刷新服务 (ApiKeyRefreshService)**
`ApiKeyRefreshService` 负责 OAuth Access Token 的智能刷新逻辑，支持被动刷新（使用时检查过期并刷新）和主动刷新（后台任务定期刷新）。它还处理刷新锁和失败重试。

```rust
// src/auth/api_key_refresh_service.rs
pub struct ApiKeyRefreshService {
    db: Arc<DatabaseConnection>,
    oauth_client: Arc<OAuthClient>,
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>, // 刷新锁
    refresh_stats: Arc<RwLock<RefreshStats>>, // 刷新统计
}

impl ApiKeyRefreshService {
    // 被动刷新：检查token是否需要刷新，如果需要则刷新
    pub async fn passive_refresh_if_needed(&self, session_id: &str) -> Result<TokenRefreshResult> { /* ... */ }

    // 使用锁进行token刷新，防止并发刷新
    async fn refresh_token_with_lock(
        &self,
        session_id: &str,
        refresh_type: RefreshType,
    ) -> Result<TokenRefreshResult> { /* ... */ }

    // 清理过期或孤立的OAuth会话记录
    pub async fn cleanup_stale_sessions(&self) -> Result<()> { /* ... */ }

    // 获取刷新统计信息
    pub async fn get_refresh_stats(&self) -> RefreshStats { /* ... */ }
}
```

**OAuth Token 刷新后台任务 (OAuthTokenRefreshTask)**
`OAuthTokenRefreshTask` 是一个后台任务，它使用 `tokio-util::time::DelayQueue` 来调度和执行 OAuth Access Token 的主动刷新。它定期扫描数据库中即将过期的 Token，并将其加入刷新队列。

```rust
// src/auth/api_key_refresh_task.rs
pub struct OAuthTokenRefreshTask {
    refresh_service: Arc<ApiKeyRefreshService>,
    task_state: Arc<RwLock<TaskState>>, // 任务状态
    // ... 内部调度机制 (DelayQueue)
}

impl OAuthTokenRefreshTask {
    // 启动后台任务
    pub async fn start(&self) -> Result<()> { /* ... */ }

    // 停止后台任务
    pub async fn stop(&self) -> Result<()> { /* ... */ }

    // 预计算会话的刷新计划，并入队
    pub async fn enqueue_schedule(&self, schedule: ScheduledTokenRefresh) -> Result<()> { /* ... */ }

    // 从调度器中移除会话
    pub async fn remove_session(&self, session_id: &str) -> Result<()> { /* ... */ }
}
```

**智能 API 密钥提供者 (ApiKeySelectService)**
`ApiKeySelectService` 提供了一个统一的接口来获取有效的 API 凭证，它透明地处理传统 API 密钥和 OAuth Token 的获取与刷新。

```rust
// src/auth/api_key_select_service.rs
pub struct ApiKeySelectService {
    db: Arc<DatabaseConnection>,
    oauth_client: Arc<OAuthClient>,
    refresh_service: Arc<ApiKeyRefreshService>,
    credential_cache: Arc<RwLock<HashMap<i32, CachedCredential>>>,
    refresh_locks: Arc<RwLock<HashMap<i32, Arc<tokio::sync::Mutex<()>>>>>, // 刷新锁
}

impl ApiKeySelectService {
    // 获取有效的API凭证 (核心方法)
    pub async fn get_valid_credential(&self, provider_key_id: i32) -> Result<CredentialResult> { /* ... */ }

    // 处理OAuth凭证获取，并使用刷新服务
    async fn handle_oauth_credential_with_refresh_service(
        &self,
        provider_key_id: i32,
        provider_key: &user_provider_keys::Model,
    ) -> Result<CredentialResult> { /* ... */ }

    // 清除指定provider key的缓存
    pub async fn invalidate_cache(&self, provider_key_id: i32) { /* ... */ }
}
```

### 12.2 智能 API 密钥管理系统

#### 12.2.1 系统架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    API密钥调度服务 (ApiKeySchedulerService)     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   密钥选择器     │  │   健康检查器     │  │   凭证提供者     │ │
│  │                 │  │                 │  │                 │ │
│  │ • 轮询策略       │  │ • 实时监控       │  │ • API Key获取   │ │
│  │ • 权重策略       │  │ • 故障检测       │  │ • OAuth Token获取│ │
│  │                 │  │ • 健康评分       │  │ • Token自动刷新  │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │
              ┌─────────────────────────────────────────────────────────────────┐
              │                    API密钥池 (user_provider_keys)               │
              │                                                                 │
              │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
              │  │   OpenAI    │ │   Gemini    │ │   Claude    │ │   Custom    │ │
              │  │             │ │             │ │             │ │             │ │
              │  │ • API Key   │ │ • API Key   │ │ • API Key   │ │ • API Key   │ │
              │  │ • 权重      │ │ • 权重      │ │ • 权重      │ │ • 权重      │ │
              │  │ • 健康状态  │ │ • 健康状态  │ │ • 健康状态  │ │ • 健康状态  │ │
              │  │ • 认证状态  │ │ • 认证状态  │ │ • 认证状态  │ │ • 认证状态  │ │
              │  │ • 过期时间  │ │ • 过期时间  │ │ • 过期时间  │ │ • 过期时间  │ │
              │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘ │
              └─────────────────────────────────────────────────────────────────┘
```

#### 12.2.2 核心算法设计

**API 密钥选择算法 (`ApiKeySchedulerService`)**
`ApiKeySchedulerService` 负责从用户的 API 密钥池中智能选择最合适的密钥。它通过多阶段过滤和调度算法，确保选出的密钥是活跃、健康且认证有效的。

**选择流程**:

1.  **加载候选密钥**: 根据 `user_service_apis` 配置，从数据库加载所有相关的 `user_provider_keys`。
2.  **初步有效性过滤 (`filter_valid_keys`)**:
    - **活跃状态**: 密钥必须是 `is_active = TRUE`。
    - **认证状态**: 对于 OAuth 类型的密钥，其 `auth_status` 必须为 `authorized`。
    - **过期时间**: 密钥的 `expires_at` 必须未过期。
    - **健康状态**: 密钥的 `health_status` 不能是 `unhealthy`。对于 `rate_limited` 的密钥，会检查 `rate_limit_resets_at` 是否已过。
3.  **健康状态过滤 (`filter_healthy_keys`)**:
    - 调用 `ApiKeyHealthService` 获取当前所有密钥的实时健康状态。
    - 进一步过滤掉不健康的密钥。
    - **降级策略**: 如果经过所有过滤后没有健康的密钥可用，系统将进入降级模式，使用所有通过初步有效性过滤的密钥进行尝试，以避免服务完全中断。
4.  **凭证获取**: 对于 OAuth 类型的密钥，`ApiKeySelectService` 会在选择前确保获取到有效的 Access Token（可能触发自动刷新）。
5.  **调度算法**: 根据 `user_service_apis` 配置的 `scheduling_strategy` (目前支持 `RoundRobin` 和 `Weighted`)，选择相应的调度算法。

**调度算法 (`src/key_pool/algorithms.rs`)**:

- **轮询调度 (RoundRobin)**: 简单地按顺序循环选择密钥。适用于密钥性能相近的场景。
- **权重调度 (Weighted)**: 根据密钥配置的 `weight` 属性进行选择，权重高的密钥被选中的概率更大。适用于需要根据密钥容量或优先级进行分配的场景。

```rust
// src/key_pool/pool_manager.rs
pub struct ApiKeySchedulerService {
    // ... 依赖 ApiKeyHealthService, ApiKeySelectService
}

impl ApiKeySchedulerService {
    pub async fn select_api_key_from_service_api(
        &self,
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> { /* ... */ }

    // 内部方法：初步有效性过滤
    fn filter_valid_keys(keys: &[user_provider_keys::Model]) -> Vec<user_provider_keys::Model> { /* ... */ }

    // 内部方法：健康状态过滤
    async fn filter_healthy_keys(
        &self,
        keys: &[user_provider_keys::Model],
    ) -> Vec<user_provider_keys::Model> { /* ... */ }
}

// src/key_pool/algorithms.rs
pub enum SchedulingStrategy {
    RoundRobin,      // 轮询调度
    Weighted,        // 权重调度
}

#[async_trait::async_trait]
pub trait ApiKeySelector: Send + Sync {
    async fn select_key(
        &self,
        keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult>;
    fn name(&self) -> &'static str;
    async fn reset(&self);
}

pub struct RoundRobinApiKeySelector { /* ... */ }
impl ApiKeySelector for RoundRobinApiKeySelector { /* ... */ }

pub struct WeightedApiKeySelector { /* ... */ }
impl ApiKeySelector for WeightedApiKeySelector { /* ... */ }
```

### 12.3 API 密钥健康监控系统

#### 12.3.1 监控架构

```
┌─────────────────────────────────────────────────────────────────┐
│                   API密钥健康检查服务 (ApiKeyHealthService)     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   检查调度器     │  │   状态评估器     │  │   故障处理器     │ │
│  │                 │  │                 │  │                 │ │
│  │ • 定时主动探测   │  │ • 错误分类       │  │ • 隔离不健康密钥 │ │
│  │ • 触发式检查     │  │ • 健康分数计算   │  │ • 自动限流恢复   │ │
│  │ • 速率限制处理   │  │ • 历史结果分析   │  │ • 告警通知       │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │
              ┌─────────────────────────────────────────────────────────────────┐
              │                   健康状态存储 (user_provider_keys 表 & 内存缓存) │
              │                                                                 │
              │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
              │  │  当前状态    │ │  检查历史    │ │  性能指标    │ │  故障日志    │ │
              │  │             │ │             │ │             │ │             │ │
              │  │ • is_healthy  │ │ • ApiKeyCheckResult │ • 响应时间    │ │ • 错误消息    │ │
              │  │ • health_score│ │ • 连续失败/成功 │ • 状态码      │ │ • 错误分类    │ │
              │  │ • last_check  │ │ • resets_at   │ │ • 成功率      │ │ • 最后错误时间│ │
              │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘ │
              └─────────────────────────────────────────────────────────────────┘
```

#### 12.3.2 健康检查算法

`ApiKeyHealthService` 对每个 API 密钥执行主动探测，并根据结果更新其健康状态。

**核心流程**:

1.  **执行 API 测试**: 根据 `provider_types` 配置的 `health_check_path` 和 `api_format`，构建并发送针对 AI 服务商的健康检查请求（例如，OpenAI 的 `/models` GET 请求，Claude 的 `/messages` POST 请求）。
2.  **结果分析**: 记录响应时间、HTTP 状态码。
3.  **错误分类**: 将失败的响应或网络错误分类为 `InvalidKey`, `QuotaExceeded`, `NetworkError`, `ServerError` 等。
4.  **更新内存状态**:
    - 更新 `ApiKeyHealth` 结构体中的 `is_healthy` 状态、连续成功/失败计数、平均响应时间、最后错误信息和最近检查结果历史。
    - 如果检测到 429 状态码，尝试从错误信息中解析 `resets_in_seconds`。
5.  **健康分数计算**: 根据最近的成功率、平均响应时间（惩罚高延迟）和连续失败次数（惩罚不稳定），计算一个介于 0 到 100 之间的 `health_score`。
6.  **同步到数据库**: 将更新后的健康状态（`health_status`, `health_status_detail`, `rate_limit_resets_at`, `last_error_time`）持久化到 `user_provider_keys` 表中。
7.  **限流重置调度**: 如果密钥被标记为 `rate_limited` 且解析到 `resets_in_seconds`，则向 `RateLimitResetTask` 发送命令，调度在指定时间后自动重置密钥状态。

**关键代码结构**:

```rust
// src/key_pool/api_key_health.rs
pub struct ApiKeyHealthService {
    db: Arc<DatabaseConnection>,
    client: Client, // reqwest HTTP客户端
    health_status: Arc<RwLock<HashMap<i32, ApiKeyHealth>>>,
    rate_limit_reset_sender: Arc<RwLock<Option<mpsc::Sender<ScheduleResetCommand>>>>, // 与限流重置任务通信
    // ... 配置参数 (request_timeout, failure_threshold, max_history_results)
}

impl ApiKeyHealthService {
    // 检查单个API密钥的健康状态，并更新内存和数据库
    pub async fn check_api_key(&self, key_model: &user_provider_keys::Model) -> Result<ApiKeyCheckResult> { /* ... */ }

    // 执行实际的HTTP请求进行健康检查
    async fn check_provider_key(&self, api_key: &str, provider_info: &provider_types::Model) -> Result<(u16, bool)> { /* ... */ }

    // 分类错误类型
    fn categorize_error(error: &crate::error::ProxyError) -> ApiKeyErrorCategory { /* ... */ }

    // 更新内存中的健康状态，并触发数据库同步
    async fn update_health_status(
        &self,
        key_id: i32,
        provider_type_id: ProviderTypeId,
        check_result: ApiKeyCheckResult,
    ) -> Result<()> { /* ... */ }

    // 将健康状态同步到数据库
    async fn sync_health_status_to_database(
        &self,
        key_id: i32,
        status: &ApiKeyHealth,
        check_result: &ApiKeyCheckResult,
    ) -> Result<()> { /* ... */ }

    // 计算API密钥的健康分数
    fn calculate_health_score(status: &ApiKeyHealth) -> f32 { /* ... */ }

    // 从错误消息中解析速率限制重置时间
    fn parse_resets_in_seconds_from_error(error_msg: &str) -> Option<i64> { /* ... */ }

    // 将密钥标记为速率受限，并调度重置任务
    pub async fn mark_key_as_rate_limited(
        &self,
        key_id: i32,
        resets_at: Option<chrono::NaiveDateTime>,
        details: &str,
    ) -> Result<()> { /* ... */ }
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

本架构设计文档详细描述了 AI 代理系统的完整技术方案，涵盖了从系统架构到具体实现的各个层面。该系统采用 Rust + Pingora 作为统一入口，内嵌 Axum 提供管理功能，支持多 AI 服务商代理、负载均衡、监控统计等企业级功能。

**核心优势**：

1. **统一入口**：Pingora 处理所有请求，简化部署
2. **高性能**：Rust 异步架构，支持高并发
3. **源信息隐藏**：完全保护客户端隐私
4. **智能负载均衡**：多种调度策略，自动故障转移
5. **完整监控**：实时统计，健康检查，告警通知
6. **OAuth 2.0 集成**：完整授权流程，自动 token 刷新
7. **智能密钥管理**：动态密钥选择，健康监控
8. **统一追踪系统**：数据驱动提取，完整请求记录
9. **安全可靠**：TLS 加密，证书自动续期，权限控制

**技术栈选择合理**，实施计划详细可行，风险评估全面，为项目成功交付提供了坚实的技术保障。建议严格按照阶段规划推进，重点关注 Pingora 集成和性能优化，确保系统稳定可靠运行。
