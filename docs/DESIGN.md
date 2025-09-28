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
- **代理服务**: Rust + Pingora (端口8080，专注AI代理)
- **管理服务**: Axum (端口9090，专注管理API)
- **数据库**: SQLite + Sea-ORM
- **缓存**: Redis
- **错误处理**: thiserror + anyhow
- **TLS**: rustls + acme-lib

**前端技术栈**
- **框架**: React 18 + TypeScript
- **UI库**: shadcn/ui + Radix UI
- **构建工具**: ESBuild (自定义构建脚本)
- **状态管理**: Zustand
- **路由**: React Router 7
- **样式**: Tailwind CSS + CSS Variables
- **主题**: next-themes (支持亮/暗模式)

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

```
                           ┌─────────────────────────────────┐
                           │         Client Applications      │
                           │    (Web, Mobile, API Clients)   │
                           └─────────────────┬───────────────┘
                                            │ HTTPS/HTTP
                                            │
                        ┌───────────────────┼───────────────────┐
                        │                   │                   │
                 AI代理请求                  │               管理API请求
            (/v1/*, /proxy/*)              │          (/api/*, /admin/*, /)
                        │                   │                   │
                        │              ┏━━━━▼━━━━┓              │
                        │              ┃ TLS终止  ┃              │
                        │              ┃ 证书管理  ┃              │
                        │              ┗━━━━━━━━━┛              │
                        │                   │                   │
                ┏━━━━━━━▼━━━━━━━┓            │            ┏━━━━━▼━━━━━┓
                ┃  Pingora 代理   ┃            │            ┃ Axum 管理  ┃
                ┃   端口 :8080    ┃            │            ┃ 端口 :9090 ┃
                ┗━━━━━━━┳━━━━━━━┛            │            ┗━━━━━┳━━━━━┛
                        │                   │                   │
        ┌───────────────┼───────────────────┼───────────────────┼──────────────┐
        │               │                   │                   │              │
  ┏━━━━▼━━━┓ ┏━━━━▼━━━━┓ ┏━━━━▼━━━━┓ ┏━━━━▼━━━━┓ ┏━━━▼━━━┓ ┏━━━▼━━━┓
  ┃负载均衡 ┃ ┃限速策略  ┃ ┃健康检查 ┃ ┃用户管理 ┃ ┃API管理 ┃ ┃统计查询┃
  ┃调度器   ┃ ┃熔断器   ┃ ┃请求转发 ┃ ┃       ┃ ┃       ┃ ┃系统配置┃
  ┗━━━━━━━┛ ┗━━━━━━━━┛ ┗━━━━━━━━┛ ┗━━━━━━━━┛ ┗━━━━━━━┛ ┗━━━━━━━┛
                                            │
                    ┏━━━━━━━━━━━━━━━━━━━━━━━━━▼━━━━━━━━━━━━━━━━━━━━━━━━━┓
                    ┃                   共享数据层                      ┃
                    ┃                                               ┃
                    ┃  ┏━━━━━━━━━━━┓    ┏━━━━━━━━━━━━━┓    ┏━━━━━━━━━┓  ┃
                    ┃  ┃ SQLite DB ┃    ┃ Redis Cache ┃    ┃ 文件存储 ┃  ┃
                    ┃  ┃用户数据    ┃    ┃认证缓存      ┃    ┃TLS证书  ┃  ┃
                    ┃  ┃配置数据    ┃    ┃健康状态缓存  ┃    ┃日志文件  ┃  ┃
                    ┃  ┃统计数据    ┃    ┃负载均衡状态  ┃    ┃         ┃  ┃
                    ┃  ┗━━━━━━━━━━━┛    ┗━━━━━━━━━━━━━┛    ┗━━━━━━━━━┛  ┃
                    ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
                                            │
                                            │ 代理转发 (仅从Pingora)
                                            │
                    ┏━━━━━━━━━━━━━━━━━━━━━━━━━▼━━━━━━━━━━━━━━━━━━━━━━━━━┓
                    ┃                外部AI服务商                      ┃
                    ┃                                               ┃
                    ┃  ┏━━━━━━━━━┓    ┏━━━━━━━━━┓    ┏━━━━━━━━━━━┓    ┃
                    ┃  ┃ OpenAI  ┃    ┃ Google  ┃    ┃ Anthropic ┃    ┃
                    ┃  ┃ChatGPT  ┃    ┃ Gemini  ┃    ┃  Claude   ┃    ┃
                    ┃  ┃   API   ┃    ┃   API   ┃    ┃   API     ┃    ┃
                    ┃  ┗━━━━━━━━━┛    ┗━━━━━━━━━┛    ┗━━━━━━━━━━━┛    ┃
                    ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
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

**AI代理请求流程（端口8080）** - 透明代理设计
```
Client → Pingora(8080) → PathMatch(/v1/*, /proxy/*) → Auth(API Key/JWT) → LoadBalancer → TransparentForward → AI Provider → Response → Stats
```

**代理透明性说明**：
- **PathMatch**: 仅检查路径前缀，不解析具体 URI 参数  
- **Auth**: 验证 API 密钥或 JWT 令牌，不关心具体业务逻辑
- **TransparentForward**: 完整透明转发原始请求，保持所有头信息和请求体
- **上游处理**: 所有 AI API 业务逻辑（模型选择、参数验证等）由上游 AI 服务商处理

**管理API请求流程（端口9090）**
```
Client → Axum(9090) → Auth → Business Logic → Database/Redis → Response
```

**认证统一流程**
```
服务(Pingora/Axum) → AuthService → Redis缓存查询 → Database验证 → 认证结果缓存 → Response
```

**双服务协同数据同步**
```
配置变更: Axum管理服务 → Database → Redis缓存 → Pingora代理服务自动更新
健康状态: Pingora代理服务 → Redis缓存 → Axum管理服务监控展示
统计数据: Pingora代理服务 → Database/Redis → Axum管理服务统计分析
```

## 2.4 Pingora 代理实现设计

### 2.4.1 透明代理核心逻辑

**设计原则**: Pingora 代理服务应保持最大透明性，专注于高性能转发，避免业务逻辑判断。

```rust
// 简化的代理核心逻辑
async fn proxy_request_filter(&self, session: &mut Session, ctx: &mut ProxyContext) -> Result<bool> {
    let path = session.req_header().uri.path();

    // 步骤1: 身份认证 (统一认证服务)
    self.authenticate_request(session, ctx).await?;

    // 步骤2: 其他全部透明转发，不做业务判断
    Ok(false) // 继续正常代理流程
}
```

### 2.4.2 路径匹配策略

**代理路径** (转发到 AI 服务商):
- `/v1/*` - OpenAI 兼容 API 格式
- `/proxy/*` - 通用代理路径

**非代理路径处理**:
- `/api/*`, `/admin/*` - 返回 404，提示使用管理端口
- `/*` - 其他路径返回 404

### 2.4.3 上游选择逻辑

```rust
// 透明的上游选择
async fn upstream_peer(&self, session: &mut Session, ctx: &ProxyContext) -> Result<Box<HttpPeer>> {
    // 从认证结果获取用户配置
    let user_config = &ctx.auth_result.user_config;
    
    // 根据负载均衡策略选择上游
    let upstream = self.load_balancer
        .select_upstream(user_config, &ctx.forwarding_context)
        .await?;
    
    // 创建上游连接 - 完全透明
    Ok(Box::new(HttpPeer::new(upstream, true, "".to_string())))
}
```

### 2.4.4 请求转发透明性

**保持完整透明**:
- 不修改请求 URI 和参数
- 不解析请求体内容  
- 保持所有原始头信息
- 仅添加必要的代理头信息

**仅添加的头信息**:
```
X-Forwarded-For: <client-ip>
X-Request-ID: <uuid>
Authorization: <upstream-api-key>  // 替换用户密钥为上游密钥
```

### 2.4.5 性能优化设计

**避免的重负载操作**:
- ❌ 复杂的 URI 解析和路由匹配
- ❌ 请求体内容解析和修改
- ❌ 复杂的业务逻辑判断
- ❌ 同步数据库查询

**采用的高性能策略**:
- ✅ 简单字符串前缀匹配
- ✅ Redis 缓存认证结果
- ✅ 异步非阻塞处理
- ✅ 连接池复用

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

-- 请求统计表
CREATE TABLE request_statistics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_service_api_id INTEGER NOT NULL,
    user_provider_key_id INTEGER,        -- 可能为空（如果请求在路由阶段失败）
    request_id VARCHAR(36),              -- UUID请求ID
    method VARCHAR(10) NOT NULL,         -- HTTP方法
    path VARCHAR(500),                   -- 请求路径
    status_code INTEGER,                 -- HTTP状态码
    response_time_ms INTEGER,            -- 响应时间（毫秒）
    request_size INTEGER DEFAULT 0,      -- 请求大小（字节）
    response_size INTEGER DEFAULT 0,     -- 响应大小（字节）
    tokens_prompt INTEGER DEFAULT 0,     -- 输入token数
    tokens_completion INTEGER DEFAULT 0, -- 输出token数
    tokens_total INTEGER DEFAULT 0,      -- 总token数
    model_used VARCHAR(100),             -- 使用的模型
    client_ip VARCHAR(45),               -- 客户端IP（脱敏）
    user_agent TEXT,                     -- User-Agent
    error_type VARCHAR(50),              -- 错误类型：TIMEOUT, AUTH_FAIL等
    error_message TEXT,                  -- 错误详情
    retry_count INTEGER DEFAULT 0,       -- 重试次数
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_service_api_id) REFERENCES user_service_apis(id) ON DELETE CASCADE,
    FOREIGN KEY (user_provider_key_id) REFERENCES user_provider_keys(id),
    INDEX idx_user_service_time (user_service_api_id, created_at),
    INDEX idx_status_time (status_code, created_at),
    INDEX idx_request_time (created_at)
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
├── Cargo.lock
├── README.md
├── docker-compose.yml             # 开发环境配置
├── Dockerfile                     # 生产环境镜像
├── migration/                     # 数据库迁移文件
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── m20231201_000001_create_users_table.rs
│   │   ├── m20231201_000002_create_provider_types_table.rs
│   │   └── ...
├── entity/                        # Sea-ORM实体定义
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── users.rs
│   │   ├── provider_types.rs
│   │   ├── user_provider_keys.rs
│   │   └── ...
├── src/                          # 主应用源码
│   ├── main.rs                   # 程序入口
│   ├── lib.rs                    # 库入口
│   ├── config/                   # 配置管理
│   │   ├── mod.rs
│   │   ├── app_config.rs         # 应用配置结构
│   │   └── database.rs           # 数据库配置
│   ├── auth/                     # 认证授权
│   │   ├── mod.rs
│   │   ├── jwt.rs                # JWT处理
│   │   ├── middleware.rs         # 认证中间件
│   │   └── password.rs           # 密码处理
│   ├── proxy/                    # Pingora代理服务
│   │   ├── mod.rs
│   │   ├── service.rs            # 主代理服务
│   │   ├── router.rs             # 路由分发
│   │   ├── context.rs            # 请求上下文
│   │   └── ai_handler.rs         # AI请求处理
│   ├── management/               # Axum管理API
│   │   ├── mod.rs
│   │   ├── routes/               # 路由定义
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs
│   │   │   ├── users.rs
│   │   │   ├── apis.rs
│   │   │   └── statistics.rs
│   │   ├── handlers/             # 请求处理器
│   │   │   ├── mod.rs
│   │   │   ├── auth_handler.rs
│   │   │   ├── user_handler.rs
│   │   │   └── ...
│   │   └── middleware/           # 中间件
│   │       ├── mod.rs
│   │       ├── auth.rs
│   │       └── cors.rs
│   ├── scheduler/                # 负载均衡调度
│   │   ├── mod.rs
│   │   ├── trait.rs              # 调度器trait
│   │   ├── round_robin.rs        # 轮询调度
│   │   ├── weighted.rs           # 权重调度
│   │   └── health_best.rs        # 健康度最佳调度
│   ├── health/                   # 健康检查
│   │   ├── mod.rs
│   │   ├── checker.rs            # 健康检查器
│   │   └── models.rs             # 健康状态模型
│   ├── statistics/               # 统计监控
│   │   ├── mod.rs
│   │   ├── collector.rs          # 统计数据收集
│   │   ├── aggregator.rs         # 数据聚合
│   │   └── exporter.rs           # 数据导出
│   ├── tls/                      # TLS证书管理
│   │   ├── mod.rs
│   │   ├── manager.rs            # 证书管理器
│   │   ├── acme.rs               # ACME协议实现
│   │   └── storage.rs            # 证书存储
│   ├── providers/                # AI服务商适配
│   │   ├── mod.rs
│   │   ├── trait.rs              # 服务商trait
│   │   ├── openai.rs             # OpenAI适配器
│   │   ├── gemini.rs             # Gemini适配器
│   │   └── claude.rs             # Claude适配器
│   ├── cache/                    # Redis缓存
│   │   ├── mod.rs
│   │   ├── client.rs             # Redis客户端
│   │   └── keys.rs               # 缓存键定义
│   ├── utils/                    # 工具函数
│   │   ├── mod.rs
│   │   ├── crypto.rs             # 加密工具
│   │   ├── time.rs               # 时间工具
│   │   └── validation.rs         # 验证工具
│   └── error/                    # 错误处理
│       ├── mod.rs
│       └── types.rs              # 错误类型定义
├── tests/                        # 测试代码
│   ├── integration/              # 集成测试
│   └── fixtures/                 # 测试数据
├── frontend/                     # Vue前端
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── main.ts
│   │   ├── App.vue
│   │   ├── router/
│   │   ├── stores/
│   │   ├── views/
│   │   ├── components/
│   │   └── api/
│   └── public/
├── config/                       # 配置文件
│   ├── config.toml               # 默认配置
│   ├── config.dev.toml           # 开发配置
│   └── config.prod.toml          # 生产配置
└── docs/                         # 文档
    ├── api.md                    # API文档
    ├── deployment.md             # 部署文档
    └── development.md            # 开发文档
```

### 4.2 Pingora统一入口服务

#### 4.2.1 主服务实现

```rust
// src/main.rs
use anyhow::Result;
use pingora::prelude::*;
use std::sync::Arc;
use tokio::signal;

mod config;
mod proxy;
mod management;
mod auth;
mod scheduler;
mod health;
mod statistics;
mod tls;
mod cache;
mod utils;
mod error;

use config::AppConfig;
use proxy::UnifiedProxyService;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    tracing_subscriber::fmt::init();
    
    // 加载配置
    let config = AppConfig::load()?;
    tracing::info!("Loaded configuration: {:?}", config);

    // 创建Pingora服务器
    let mut server = Server::new(Some(Opt::default()))?;
    server.bootstrap();

    // 初始化应用状态
    let app_state = Arc::new(AppState::new(&config).await?);
    
    // 启动后台任务
    start_background_tasks(app_state.clone()).await;

    // 创建统一代理服务
    let proxy_service = Arc::new(UnifiedProxyService::new(app_state.clone()));
    let mut http_proxy = http_proxy_service(&server.configuration, proxy_service);

    // 配置HTTPS监听
    if config.tls.enabled {
        http_proxy.add_tls(
            &config.server.https_bind_address,
            None,
            tls::create_tls_callback(app_state.clone()),
        );
    }

    // 配置HTTP监听
    http_proxy.add_tcp(&config.server.http_bind_address);

    server.add_service(http_proxy);

    // 优雅关闭处理
    let shutdown_signal = async {
        signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        tracing::info!("Received shutdown signal");
    };

    tokio::select! {
        _ = server.run_forever() => {},
        _ = shutdown_signal => {
            tracing::info!("Shutting down gracefully...");
        }
    }

    Ok(())
}

// 启动后台任务
async fn start_background_tasks(app_state: Arc<AppState>) {
    // 健康检查任务
    let health_checker = app_state.health_checker.clone();
    tokio::spawn(async move {
        health_checker.start_background_checks().await;
    });

    // 统计数据聚合任务
    let statistics = app_state.statistics.clone();
    tokio::spawn(async move {
        statistics.start_aggregation_task().await;
    });

    // TLS证书续期任务
    let tls_manager = app_state.tls_manager.clone();
    tokio::spawn(async move {
        tls_manager.start_renewal_task().await;
    });

    // 数据清理任务
    let db = app_state.db.clone();
    tokio::spawn(async move {
        cleanup_old_data(db).await;
    });
}

// 应用状态结构
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Arc<DatabaseConnection>,
    pub redis: Arc<redis::Client>,
    pub health_checker: Arc<health::HealthChecker>,
    pub tls_manager: Arc<tls::TlsManager>,
    pub schedulers: Arc<scheduler::SchedulerRegistry>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        // 初始化数据库
        let db = Arc::new(sea_orm::Database::connect(&config.database.url).await?);
        
        // 运行数据库迁移
        migration::run_migrations(&db).await?;

        // 初始化Redis
        let redis = Arc::new(redis::Client::open(config.redis.url.clone())?);

        // 创建各个服务实例
        let health_checker = Arc::new(health::HealthChecker::new(db.clone(), redis.clone()));
        let tls_manager = Arc::new(tls::TlsManager::new(db.clone(), config.clone()));
        let schedulers = Arc::new(scheduler::SchedulerRegistry::new(
            db.clone(), 
            redis.clone(),
            health_checker.clone()
        ));

        Ok(Self {
            config: Arc::new(config.clone()),
            db,
            redis,
            health_checker,
            tls_manager,
            schedulers,
        })
    }
}
```

#### 4.2.2 统一代理服务

```rust
// src/proxy/service.rs
use pingora::prelude::*;
use std::sync::Arc;
use crate::{AppState, error::ProxyError};

pub struct UnifiedProxyService {
    app_state: Arc<AppState>,
    management_service: Arc<management::ManagementService>,
    ai_proxy_handler: Arc<AIProxyHandler>,
}

impl UnifiedProxyService {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let management_service = Arc::new(management::ManagementService::new(app_state.clone()));
        let ai_proxy_handler = Arc::new(AIProxyHandler::new(app_state.clone()));

        Self {
            app_state,
            management_service,
            ai_proxy_handler,
        }
    }
}

#[async_trait]
impl ProxyHttp for UnifiedProxyService {
    type CTX = ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        ProxyContext::new()
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool, pingora::Error> {
        let path = session.req_header().uri.path();
        
        // 生成请求ID用于追踪
        ctx.request_id = uuid::Uuid::new_v4().to_string();
        ctx.start_time = std::time::Instant::now();
        
        // 记录请求日志
        tracing::info!(
            request_id = %ctx.request_id,
            method = %session.req_header().method,
            path = %path,
            "Processing request"
        );

        // 路由决策
        ctx.route_type = self.determine_route_type(path);

        match ctx.route_type {
            RouteType::Management | RouteType::Static => {
                // 处理管理API或静态文件请求
                match self.handle_management_request(session, ctx).await {
                    Ok(_) => Ok(true), // 请求已处理，终止代理流程
                    Err(e) => {
                        tracing::error!(
                            request_id = %ctx.request_id,
                            error = %e,
                            "Management request failed"
                        );
                        self.send_error_response(session, e).await;
                        Ok(true)
                    }
                }
            },
            RouteType::AIProxy => {
                // 处理AI代理请求
                // 早期设计曾使用 Pipeline（认证→限流→配置→选 key）模式。
                // 现已改为由 ProxyService 内部按阶段顺序编排执行，并在顶层统一映射错误。
                match self.prepare_proxy(session, ctx).await {
                    Ok(_) => Ok(false),
                    Err(e) => { self.send_error_response(session, e).await; Ok(true) }
                }
            }
        }
    }

    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>, pingora::Error> {
        // 只有AI代理请求会到达这里
        self.ai_proxy_handler.select_upstream_peer(ctx).await
            .map_err(|e| pingora::Error::new_str(&e.to_string()))
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<(), pingora::Error> {
        // 处理上游请求，隐藏源信息
        self.ai_proxy_handler
            .filter_upstream_request(session, upstream_request, ctx)
            .await
            .map_err(|e| pingora::Error::new_str(&e.to_string()))
    }

    async fn upstream_response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<(), pingora::Error> {
        // 处理上游响应
        self.ai_proxy_handler
            .filter_upstream_response(upstream_response, ctx)
            .await
            .map_err(|e| pingora::Error::new_str(&e.to_string()))
    }

    async fn logging(&self, session: &mut Session, e: Option<&pingora::Error>, ctx: &mut Self::CTX) {
        let status_code = session.response_written()
            .map(|resp| resp.status.as_u16())
            .unwrap_or(500);
            
        let response_time = ctx.start_time.elapsed().as_millis() as u32;

        // 记录访问日志
        if let Some(error) = e {
            tracing::error!(
                request_id = %ctx.request_id,
                status_code = status_code,
                response_time_ms = response_time,
                error = %error,
                "Request completed with error"
            );
        } else {
            tracing::info!(
                request_id = %ctx.request_id,
                status_code = status_code,
                response_time_ms = response_time,
                "Request completed successfully"
            );
        }

        // 记录统计数据
        if let Err(e) = self.record_request_stats(session, ctx, e).await {
            tracing::warn!(
                request_id = %ctx.request_id,
                error = %e,
                "Failed to record request statistics"
            );
        }
    }
}

impl UnifiedProxyService {
    fn determine_route_type(&self, path: &str) -> RouteType {
        if path.starts_with("/api/") || path.starts_with("/admin/") {
            RouteType::Management
        } else if path.starts_with("/v1/") || path.starts_with("/proxy/") {
            RouteType::AIProxy
        } else {
            RouteType::Static
        }
    }

    async fn handle_management_request(
        &self,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        self.management_service.handle_request(session, ctx).await
    }

    async fn send_error_response(&self, session: &mut Session, error: ProxyError) {
        let (status, message) = match error {
            ProxyError::Authentication(_) => (401, "Authentication required"),
            ProxyError::Authorization(_) => (403, "Access denied"),
            ProxyError::NotFound(_) => (404, "Not found"),
            ProxyError::RateLimit(_) => (429, "Rate limit exceeded"),
            ProxyError::Timeout(_) => (408, "Request timeout"),
            ProxyError::BadGateway(_) => (502, "Bad gateway"),
            _ => (500, "Internal server error"),
        };

        let response_body = serde_json::json!({
            "error": {
                "message": message,
                "type": "api_error",
                "code": status
            }
        });

        session.set_response_status(status).ok();
        session.insert_header("content-type", "application/json").ok();
        session.write_response_body(Some(response_body.to_string().into())).await.ok();
    }

    async fn record_request_stats(
        &self,
        session: &Session,
        ctx: &ProxyContext,
        error: Option<&pingora::Error>,
    ) -> Result<(), ProxyError> {
        // 收集统计数据并异步记录
        let stats = RequestStats {
            request_id: ctx.request_id.clone(),
            route_type: ctx.route_type,
            user_service_api_id: ctx.user_service_api.as_ref().map(|api| api.id),
            user_provider_key_id: ctx.selected_backend.as_ref().map(|key| key.id),
            method: session.req_header().method.to_string(),
            path: session.req_header().uri.path().to_string(),
            status_code: session.response_written()
                .map(|resp| resp.status.as_u16())
                .unwrap_or(500),
            response_time_ms: ctx.start_time.elapsed().as_millis() as u32,
            error_message: error.map(|e| e.to_string()),
            created_at: chrono::Utc::now(),
        };

        // 异步记录统计数据，避免阻塞请求处理
        let statistics = self.app_state.statistics.clone();
        tokio::spawn(async move {
            if let Err(e) = statistics.record_request(stats).await {
                tracing::warn!("Failed to record request statistics: {}", e);
            }
        });

        Ok(())
    }
}

// 请求上下文
#[derive(Default)]
pub struct ProxyContext {
    pub request_id: String,
    pub route_type: RouteType,
    pub user_service_api: Option<entity::user_service_apis::Model>,
    pub selected_backend: Option<entity::user_provider_keys::Model>,
    pub provider_type: Option<entity::provider_types::Model>,
    pub start_time: std::time::Instant,
    pub retry_count: u32,
    pub tokens_used: u32,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum RouteType {
    #[default]
    Management,
    AIProxy,
    Static,
}

impl ProxyContext {
    pub fn new() -> Self {
        Self {
            request_id: String::new(),
            start_time: std::time::Instant::now(),
            ..Default::default()
        }
    }
}
```

### 4.3 AI代理处理器

```rust
// src/proxy/ai_handler.rs
use std::sync::Arc;
use pingora::prelude::*;
use crate::{AppState, error::ProxyError, scheduler::LoadBalancer};

pub struct AIProxyHandler {
    app_state: Arc<AppState>,
}

impl AIProxyHandler {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self { app_state }
    }

    // 旧式 prepare_proxy_request 已废弃：改为 ProxyService + Pipeline
        // 2. 检查速率限制
        self.check_rate_limit(&user_service_api).await?;

        // 3. 获取提供商类型信息
        let provider_type = self.get_provider_type(user_service_api.provider_type_id).await?;
        ctx.provider_type = Some(provider_type);

        // 4. 根据调度策略选择后端API密钥
        let scheduler = self.app_state.schedulers.get(&user_service_api.scheduling_strategy)?;
        let selected_backend = scheduler.select_backend(&user_service_api).await?;
        ctx.selected_backend = Some(selected_backend);

        tracing::debug!(
            request_id = %ctx.request_id,
            user_api_id = user_service_api.id,
            backend_key_id = ctx.selected_backend.as_ref().unwrap().id,
            strategy = %user_service_api.scheduling_strategy,
            "Backend selected successfully"
        );

        Ok(())
    }

    pub async fn select_upstream_peer(&self, ctx: &ProxyContext) -> Result<Box<HttpPeer>, ProxyError> {
        let provider_type = ctx.provider_type.as_ref()
            .ok_or(ProxyError::Internal("Provider type not set".into()))?;

        let upstream_addr = format!("{}:443", provider_type.base_url);
        
        tracing::debug!(
            request_id = %ctx.request_id,
            upstream = %upstream_addr,
            "Selected upstream peer"
        );

        let peer = HttpPeer::new(upstream_addr, true, String::new());
        Ok(Box::new(peer))
    }

    pub async fn filter_upstream_request(
        &self,
        _session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &ProxyContext,
    ) -> Result<(), ProxyError> {
        let selected_backend = ctx.selected_backend.as_ref()
            .ok_or(ProxyError::Internal("Backend not selected".into()))?;
        let provider_type = ctx.provider_type.as_ref()
            .ok_or(ProxyError::Internal("Provider type not set".into()))?;

        // 替换Authorization头
        upstream_request.remove_header("authorization");
        let auth_value = self.build_auth_header_from_config(&provider_type.config_json, &selected_backend.api_key);
        upstream_request.insert_header("authorization", &auth_value)
            .map_err(|e| ProxyError::Internal(format!("Failed to set auth header: {}", e)))?;

        // 设置正确的Host头
        upstream_request.insert_header("host", &provider_type.base_url)
            .map_err(|e| ProxyError::Internal(format!("Failed to set host header: {}", e)))?;

        // 移除可能暴露客户端信息的头部
        let headers_to_remove = [
            "x-forwarded-for",
            "x-real-ip", 
            "x-forwarded-proto",
            "x-original-forwarded-for",
            "x-client-ip",
            "cf-connecting-ip",
        ];

        for header in &headers_to_remove {
            upstream_request.remove_header(header);
        }

        // 添加代理标识
        upstream_request.insert_header("user-agent", "AI-Proxy-Service/1.0")
            .map_err(|e| ProxyError::Internal(format!("Failed to set user-agent: {}", e)))?;

        // 添加请求ID用于追踪
        upstream_request.insert_header("x-request-id", &ctx.request_id)
            .map_err(|e| ProxyError::Internal(format!("Failed to set request-id: {}", e)))?;

        tracing::debug!(
            request_id = %ctx.request_id,
            backend_key_id = selected_backend.id,
            provider = %provider_type.name,
            "Upstream request filtered"
        );

        Ok(())
    }

    pub async fn filter_upstream_response(
        &self,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 提取token使用信息
        ctx.tokens_used = self.extract_token_usage(upstream_response);

        // 移除可能暴露上游服务器信息的头部
        upstream_response.remove_header("server");
        upstream_response.remove_header("x-powered-by");

        // 添加自己的服务器标识
        upstream_response.insert_header("server", "AI-Proxy-Service")
            .map_err(|e| ProxyError::Internal(format!("Failed to set server header: {}", e)))?;

        tracing::debug!(
            request_id = %ctx.request_id,
            status = upstream_response.status.as_u16(),
            tokens_used = ctx.tokens_used,
            "Upstream response filtered"
        );

        Ok(())
    }

    // 私有辅助方法
    async fn extract_api_key(&self, session: &Session) -> Result<String, ProxyError> {
        // 从Authorization头提取API密钥
        if let Some(auth_header) = session.req_header().headers.get("authorization") {
            let auth_str = std::str::from_utf8(auth_header.as_bytes())
                .map_err(|_| ProxyError::Authentication("Invalid authorization header encoding".into()))?;
            
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Ok(token.to_string());
            }
        }

        // 从查询参数提取API密钥
        if let Some(query) = session.req_header().uri.query() {
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    if key == "api_key" {
                        return Ok(value.to_string());
                    }
                }
            }
        }

        Err(ProxyError::Authentication("API key not found".into()))
    }

    async fn authenticate_api_key(&self, api_key: &str) -> Result<entity::user_service_apis::Model, ProxyError> {
        use entity::user_service_apis::{Entity as UserServiceApis, Column};
        use sea_orm::EntityTrait;

        let user_api = UserServiceApis::find()
            .filter(Column::ApiKey.eq(api_key))
            .filter(Column::IsActive.eq(true))
            .one(&*self.app_state.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::Authentication("Invalid API key".into()))?;

        // 检查API密钥是否过期
        if let Some(expires_at) = user_api.expires_at {
            if expires_at < chrono::Utc::now().naive_utc() {
                return Err(ProxyError::Authentication("API key expired".into()));
            }
        }

        Ok(user_api)
    }

    async fn check_rate_limit(&self, user_api: &entity::user_service_apis::Model) -> Result<(), ProxyError> {
        let cache_key = format!("rate_limit:api:{}:minute", user_api.id);
        
        let mut redis_conn = self.app_state.redis.get_connection()
            .map_err(|e| ProxyError::Internal(format!("Redis connection error: {}", e)))?;

        // 使用Redis的滑动窗口算法实现速率限制
        let current_count: i32 = redis::cmd("INCR")
            .arg(&cache_key)
            .query(&mut redis_conn)
            .unwrap_or(1);

        if current_count == 1 {
            // 设置过期时间为60秒
            redis::cmd("EXPIRE")
                .arg(&cache_key)
                .arg(60)
                .execute(&mut redis_conn)
                .ok();
        }

        if current_count > user_api.rate_limit {
            return Err(ProxyError::RateLimit(format!(
                "Rate limit exceeded: {} requests per minute",
                user_api.rate_limit
            )));
        }

        Ok(())
    }

    async fn get_provider_type(&self, provider_type_id: i32) -> Result<entity::provider_types::Model, ProxyError> {
        use entity::provider_types::{Entity as ProviderTypes};
        use sea_orm::EntityTrait;

        ProviderTypes::find_by_id(provider_type_id)
            .one(&*self.app_state.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::Internal("Provider type not found".into()))
    }

    fn build_auth_header(&self, format: &str, api_key: &str) -> String {
        format.replace("{key}", api_key)
    }

    fn extract_token_usage(&self, response: &ResponseHeader) -> u32 {
        // 尝试从不同的响应头中提取token使用信息
        let token_headers = [
            "x-openai-total-tokens",
            "x-anthropic-total-tokens", 
            "x-google-total-tokens",
        ];

        for header_name in &token_headers {
            if let Some(header_value) = response.headers.get(*header_name) {
                if let Ok(tokens_str) = std::str::from_utf8(header_value.as_bytes()) {
                    if let Ok(tokens) = tokens_str.parse::<u32>() {
                        return tokens;
                    }
                }
            }
        }

        0
    }
}
```

### 4.4 负载均衡调度器

```rust
// src/scheduler/mod.rs
use async_trait::async_trait;
use std::{sync::Arc, collections::HashMap};
use crate::{AppState, error::ProxyError};

// 调度器trait定义
#[async_trait]
pub trait LoadBalancer: Send + Sync {
    async fn select_backend(
        &self,
        user_service_api: &entity::user_service_apis::Model,
    ) -> Result<entity::user_provider_keys::Model, ProxyError>;
}

// 调度器注册表
pub struct SchedulerRegistry {
    schedulers: HashMap<String, Arc<dyn LoadBalancer>>,
}

impl SchedulerRegistry {
    pub fn new(
        db: Arc<sea_orm::DatabaseConnection>,
        redis: Arc<redis::Client>,
        health_checker: Arc<crate::health::HealthChecker>,
    ) -> Self {
        let mut schedulers: HashMap<String, Arc<dyn LoadBalancer>> = HashMap::new();

        // 注册轮询调度器
        schedulers.insert(
            "round_robin".to_string(),
            Arc::new(RoundRobinScheduler::new(db.clone(), redis.clone())),
        );

        // 注册权重调度器
        schedulers.insert(
            "weighted".to_string(),
            Arc::new(WeightedScheduler::new(db.clone(), redis.clone())),
        );

        // 注册健康度最佳调度器
        schedulers.insert(
            "health_best".to_string(),
            Arc::new(HealthBestScheduler::new(db.clone(), redis.clone(), health_checker)),
        );

        Self { schedulers }
    }

    pub fn get(&self, strategy: &str) -> Result<Arc<dyn LoadBalancer>, ProxyError> {
        self.schedulers
            .get(strategy)
            .cloned()
            .ok_or_else(|| ProxyError::Internal(format!("Unknown scheduling strategy: {}", strategy)))
    }
}

// 轮询调度器实现
pub struct RoundRobinScheduler {
    db: Arc<sea_orm::DatabaseConnection>,
    redis: Arc<redis::Client>,
}

impl RoundRobinScheduler {
    pub fn new(db: Arc<sea_orm::DatabaseConnection>, redis: Arc<redis::Client>) -> Self {
        Self { db, redis }
    }
}

#[async_trait]
impl LoadBalancer for RoundRobinScheduler {
    async fn select_backend(
        &self,
        user_service_api: &entity::user_service_apis::Model,
    ) -> Result<entity::user_provider_keys::Model, ProxyError> {
        use entity::user_provider_keys::{Entity as UserProviderKeys, Column};
        use sea_orm::{EntityTrait, QueryFilter, QueryOrder};

        // 获取该用户该服务商的所有活跃API密钥
        let available_keys = UserProviderKeys::find()
            .filter(Column::UserId.eq(user_service_api.user_id))
            .filter(Column::ProviderTypeId.eq(user_service_api.provider_type_id))
            .filter(Column::IsActive.eq(true))
            .order_by_asc(Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::BadGateway("No available API keys".into()));
        }

        // 从Redis获取当前轮询位置
        let cache_key = format!("round_robin:{}:{}", user_service_api.user_id, user_service_api.provider_type_id);
        let mut redis_conn = self.redis.get_connection()
            .map_err(|e| ProxyError::Internal(format!("Redis connection error: {}", e)))?;

        let current_index: i32 = redis::cmd("GET")
            .arg(&cache_key)
            .query(&mut redis_conn)
            .unwrap_or(0);

        let next_index = (current_index + 1) % available_keys.len() as i32;
        
        // 更新轮询位置
        redis::cmd("SET")
            .arg(&cache_key)
            .arg(next_index)
            .arg("EX")
            .arg(3600) // 1小时过期
            .execute(&mut redis_conn)
            .ok();

        let selected_key = available_keys[current_index as usize % available_keys.len()].clone();

        tracing::debug!(
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            selected_key_id = selected_key.id,
            current_index = current_index,
            total_keys = available_keys.len(),
            "Round robin selection"
        );

        Ok(selected_key)
    }
}

// 权重调度器实现
pub struct WeightedScheduler {
    db: Arc<sea_orm::DatabaseConnection>,
    redis: Arc<redis::Client>,
}

impl WeightedScheduler {
    pub fn new(db: Arc<sea_orm::DatabaseConnection>, redis: Arc<redis::Client>) -> Self {
        Self { db, redis }
    }
}

#[async_trait]
impl LoadBalancer for WeightedScheduler {
    async fn select_backend(
        &self,
        user_service_api: &entity::user_service_apis::Model,
    ) -> Result<entity::user_provider_keys::Model, ProxyError> {
        use entity::user_provider_keys::{Entity as UserProviderKeys, Column};
        use sea_orm::{EntityTrait, QueryFilter, QueryOrder};
        use rand::Rng;

        // 获取该用户该服务商的所有活跃API密钥
        let available_keys = UserProviderKeys::find()
            .filter(Column::UserId.eq(user_service_api.user_id))
            .filter(Column::ProviderTypeId.eq(user_service_api.provider_type_id))
            .filter(Column::IsActive.eq(true))
            .order_by_asc(Column::Weight.desc())
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::BadGateway("No available API keys".into()));
        }

        // 计算总权重
        let total_weight: i32 = available_keys.iter().map(|key| key.weight).sum();
        
        if total_weight <= 0 {
            // 如果总权重为0，使用轮询算法
            let round_robin = RoundRobinScheduler::new(self.db.clone(), self.redis.clone());
            return round_robin.select_backend(user_service_api).await;
        }

        // 使用加权随机算法选择
        let mut rng = rand::thread_rng();
        let mut random_weight = rng.gen_range(1..=total_weight);

        for key in &available_keys {
            random_weight -= key.weight;
            if random_weight <= 0 {
                tracing::debug!(
                    user_id = user_service_api.user_id,
                    provider_type_id = user_service_api.provider_type_id,
                    selected_key_id = key.id,
                    key_weight = key.weight,
                    total_weight = total_weight,
                    Weighted selection
                );
                return Ok(key.clone());
            }
        }

        // 理论上不应该到达这里，但为了安全返回第一个
        Ok(available_keys[0].clone())
    }
}

// 健康度最佳调度器实现
pub struct HealthBestScheduler {
    db: Arc<sea_orm::DatabaseConnection>,
    redis: Arc<redis::Client>,
    health_checker: Arc<crate::health::HealthChecker>,
}

impl HealthBestScheduler {
    pub fn new(
        db: Arc<sea_orm::DatabaseConnection>, 
        redis: Arc<redis::Client>,
        health_checker: Arc<crate::health::HealthChecker>,
    ) -> Self {
        Self { db, redis, health_checker }
    }
}

#[async_trait]
impl LoadBalancer for HealthBestScheduler {
    async fn select_backend(
        &self,
        user_service_api: &entity::user_service_apis::Model,
    ) -> Result<entity::user_provider_keys::Model, ProxyError> {
        // 获取所有健康的API密钥
        let healthy_keys = self.health_checker
            .get_healthy_keys(user_service_api.user_id, user_service_api.provider_type_id)
            .await?;

        if healthy_keys.is_empty() {
            return Err(ProxyError::BadGateway("No healthy API keys available".into()));
        }

        // 选择响应时间最短的健康节点
        let best_key = healthy_keys
            .into_iter()
            .min_by_key(|key| key.response_time_ms)
            .ok_or_else(|| ProxyError::Internal("Failed to select best key".into()))?;

        tracing::debug!(
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            selected_key_id = best_key.id,
            response_time_ms = best_key.response_time_ms,
            "Health-best selection"
        );

        Ok(best_key)
    }
}
```

### 4.5 健康检查模块

```rust
// src/health/mod.rs
use std::sync::Arc;
use tokio::time::{interval, Duration};
use sea_orm::{EntityTrait, QueryFilter};
use crate::{AppState, error::ProxyError};

pub struct HealthChecker {
    db: Arc<sea_orm::DatabaseConnection>,
    redis: Arc<redis::Client>,
}

impl HealthChecker {
    pub fn new(
        db: Arc<sea_orm::DatabaseConnection>,
        redis: Arc<redis::Client>,
    ) -> Self {
        Self { db, redis }
    }

    // 启动后台健康检查任务
    pub async fn start_background_checks(&self) {
        let mut interval = interval(Duration::from_secs(30)); // 每30秒检查一次
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_all_apis().await {
                tracing::error!("Health check cycle failed: {}", e);
            }
        }
    }

    // 检查所有活跃的API密钥
    async fn check_all_apis(&self) -> Result<(), ProxyError> {
        use entity::user_provider_keys::{Entity as UserProviderKeys, Column};

        let provider_keys = UserProviderKeys::find()
            .filter(Column::IsActive.eq(true))
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?;

        tracing::info!("Starting health check for {} API keys", provider_keys.len());

        // 并发检查所有API密钥
        let tasks: Vec<_> = provider_keys
            .into_iter()
            .map(|key| {
                let checker = self.clone();
                tokio::spawn(async move {
                    checker.check_single_api(&key).await
                })
            })
            .collect();

        let results = futures::future::join_all(tasks).await;
        
        let mut success_count = 0;
        let mut error_count = 0;

        for result in results {
            match result {
                Ok(Ok(_)) => success_count += 1,
                Ok(Err(e)) => {
                    error_count += 1;
                    tracing::warn!("Health check failed for API key: {}", e);
                }
                Err(e) => {
                    error_count += 1;
                    tracing::error!("Health check task panicked: {}", e);
                }
            }
        }

        tracing::info!(
            "Health check completed: {} successful, {} failed",
            success_count,
            error_count
        );

        Ok(())
    }

    // 检查单个API密钥
    async fn check_single_api(&self, key: &entity::user_provider_keys::Model) -> Result<(), ProxyError> {
        let start_time = std::time::Instant::now();
        
        // 获取提供商类型信息
        let provider_type = self.get_provider_type(key.provider_type_id).await?;
        
        // 构建健康检查URL
        let health_check_url = format!(
            "https://{}{}", 
            provider_type.base_url, 
            provider_type.health_check_path
        );

        // 创建HTTP客户端
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(provider_type.timeout_seconds as u64))
            .build()
            .map_err(|e| ProxyError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        // 构建请求
        let auth_header = extract_auth_header_from_config(&provider_type.config_json, &key.api_key);
        let request = client
            .get(&health_check_url)
            .header("Authorization", &auth_header)
            .header("User-Agent", "AI-Proxy-HealthCheck/1.0");

        // 发送健康检查请求
        let response = request.send().await;
        let response_time = start_time.elapsed().as_millis() as i32;

        let (is_healthy, error_message) = match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    (true, None)
                } else {
                    (false, Some(format!("HTTP {}: {}", resp.status(), resp.status().canonical_reason().unwrap_or("Unknown"))))
                }
            }
            Err(e) => (false, Some(e.to_string())),
        };

        // 更新健康状态
        self.update_health_status(key.id, is_healthy, response_time, error_message).await?;

        tracing::debug!(
            key_id = key.id,
            provider = %provider_type.name,
            is_healthy = is_healthy,
            response_time_ms = response_time,
            "Health check completed"
        );

        Ok(())
    }

    // 更新健康状态
    async fn update_health_status(
        &self,
        key_id: i32,
        is_healthy: bool,
        response_time_ms: i32,
        error_message: Option<String>,
    ) -> Result<(), ProxyError> {
        use entity::api_health_status::{Entity as ApiHealthStatus, Column, ActiveModel};
        use sea_orm::{Set, ActiveModelTrait, EntityTrait};

        // 查找或创建健康状态记录
        let existing_status = ApiHealthStatus::find()
            .filter(Column::UserProviderKeyId.eq(key_id))
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?;

        let now = chrono::Utc::now().naive_utc();

        let mut status_model = if let Some(status) = existing_status {
            let consecutive_failures = if is_healthy {
                0
            } else {
                status.consecutive_failures + 1
            };

            ActiveModel {
                id: Set(status.id),
                user_provider_key_id: Set(status.user_provider_key_id),
                is_healthy: Set(is_healthy),
                response_time_ms: Set(response_time_ms),
                success_rate: Set(self.calculate_success_rate(status.total_checks + 1, status.successful_checks + if is_healthy { 1 } else { 0 })),
                last_success: Set(if is_healthy { Some(now) } else { status.last_success }),
                last_failure: Set(if !is_healthy { Some(now) } else { status.last_failure }),
                consecutive_failures: Set(consecutive_failures),
                total_checks: Set(status.total_checks + 1),
                successful_checks: Set(status.successful_checks + if is_healthy { 1 } else { 0 }),
                last_error_message: Set(error_message),
                updated_at: Set(now),
                ..Default::default()
            }
        } else {
            ActiveModel {
                user_provider_key_id: Set(key_id),
                is_healthy: Set(is_healthy),
                response_time_ms: Set(response_time_ms),
                success_rate: Set(if is_healthy { 1.0 } else { 0.0 }),
                last_success: Set(if is_healthy { Some(now) } else { None }),
                last_failure: Set(if !is_healthy { Some(now) } else { None }),
                consecutive_failures: Set(if is_healthy { 0 } else { 1 }),
                total_checks: Set(1),
                successful_checks: Set(if is_healthy { 1 } else { 0 }),
                last_error_message: Set(error_message),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
        };

        status_model.save(&*self.db).await
            .map_err(|e| ProxyError::Internal(format!("Failed to save health status: {}", e)))?;

        // 更新Redis缓存
        self.cache_health_status(key_id, is_healthy, response_time_ms).await?;

        Ok(())
    }

    // 获取健康的API密钥列表
    pub async fn get_healthy_keys(
        &self,
        user_id: i32,
        provider_type_id: i32,
    ) -> Result<Vec<HealthyKey>, ProxyError> {
        use entity::user_provider_keys::{Entity as UserProviderKeys, Column as KeyColumn};
        use entity::api_health_status::{Entity as ApiHealthStatus, Column as StatusColumn};
        use sea_orm::{QueryFilter, QuerySelect, JoinType};

        let healthy_keys = UserProviderKeys::find()
            .filter(KeyColumn::UserId.eq(user_id))
            .filter(KeyColumn::ProviderTypeId.eq(provider_type_id))
            .filter(KeyColumn::IsActive.eq(true))
            .join(JoinType::LeftJoin, entity::user_provider_keys::Relation::ApiHealthStatus.def())
            .filter(StatusColumn::IsHealthy.eq(true).or(StatusColumn::IsHealthy.is_null()))
            .select_also(ApiHealthStatus)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?;

        let result: Vec<HealthyKey> = healthy_keys
            .into_iter()
            .map(|(key, health_status)| HealthyKey {
                id: key.id,
                api_key: key.api_key,
                name: key.name,
                weight: key.weight,
                response_time_ms: health_status
                    .as_ref()
                    .map(|h| h.response_time_ms)
                    .unwrap_or(1000), // 默认1秒响应时间
                success_rate: health_status
                    .as_ref()
                    .map(|h| h.success_rate)
                    .unwrap_or(1.0), // 默认100%成功率
            })
            .collect();

        Ok(result)
    }

    // 私有辅助方法
    async fn get_provider_type(&self, provider_type_id: i32) -> Result<entity::provider_types::Model, ProxyError> {
        use entity::provider_types::Entity as ProviderTypes;

        ProviderTypes::find_by_id(provider_type_id)
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::Internal("Provider type not found".into()))
    }

    fn calculate_success_rate(&self, total_checks: i32, successful_checks: i32) -> f64 {
        if total_checks == 0 {
            0.0
        } else {
            successful_checks as f64 / total_checks as f64
        }
    }

    async fn cache_health_status(
        &self,
        key_id: i32,
        is_healthy: bool,
        response_time_ms: i32,
    ) -> Result<(), ProxyError> {
        let cache_key = format!("health:key:{}", key_id);
        let cache_value = serde_json::json!({
            "is_healthy": is_healthy,
            "response_time_ms": response_time_ms,
            "updated_at": chrono::Utc::now().timestamp()
        });

        let mut redis_conn = self.redis.get_connection()
            .map_err(|e| ProxyError::Internal(format!("Redis connection error: {}", e)))?;

        redis::cmd("SET")
            .arg(&cache_key)
            .arg(cache_value.to_string())
            .arg("EX")
            .arg(300) // 5分钟过期
            .execute(&mut redis_conn)
            .map_err(|e| ProxyError::Internal(format!("Redis SET error: {}", e)))?;

        Ok(())
    }
}

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
                tracing::error!("Certificate renewal check failed: {}", e);
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

        tracing::info!("Found {} certificates to renew", expiring_certs.len());

        for cert in expiring_certs {
            if let Err(e) = self.renew_certificate(&cert).await {
                tracing::error!(
                    domain = %cert.domain,
                    error = %e,
                    "Failed to renew certificate"
                );
            }
        }

        Ok(())
    }

    // 续期单个证书
    async fn renew_certificate(&self, cert: &entity::tls_certificates::Model) -> Result<(), ProxyError> {
        tracing::info!(domain = %cert.domain, "Renewing certificate");

        match cert.cert_type.as_str() {
            "acme" => self.renew_acme_certificate(cert).await,
            "self_signed" => self.renew_self_signed_certificate(cert).await,
            _ => {
                tracing::warn!(
                    domain = %cert.domain,
                    cert_type = %cert.cert_type,
                    "Cannot auto-renew certificate of this type"
                );
                Ok(())
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

        tracing::info!(
            domain = %cert.domain,
            expires_at = %expires_at,
            "Certificate renewed successfully"
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

### 6.3 传输安全

**TLS配置**
```rust
use rustls::{ServerConfig, ClientConfig, Certificate, PrivateKey};

pub fn create_tls_config() -> Result<ServerConfig, TlsError> {
    let config = ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])?
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;

    Ok(config)
}
```

**HSTS配置**
```rust
// 添加安全头部中间件
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    headers.insert("Strict-Transport-Security", "max-age=31536000; includeSubDomains".parse().unwrap());
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    
    response
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
