# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个基于 Rust + Pingora 的企业级 AI 服务代理平台，采用**双端口分离架构**设计，为用户提供统一的 AI 服务访问接口，支持多个主流 AI 服务提供商（OpenAI、Google Gemini、Anthropic Claude），具备负载均衡、监控统计、安全防护等功能。

## 核心架构

### 双端口分离架构原理
- **Pingora 代理服务** (端口8080): 专注高性能AI请求代理，基于Pingora 0.6.0原生性能
- **Axum 管理服务** (端口9090): 专注业务管理逻辑，用户管理、API密钥管理、统计查询
- **共享数据层**: SQLite数据库 + Redis缓存 + 统一认证系统

### 技术栈
- **核心框架**: Rust 2024 Edition + Pingora 0.6.0 + Axum 0.8.4
- **数据库**: SQLite + Sea-ORM 1.1.13 + Sea-ORM-Migration
- **缓存**: Redis with connection manager
- **认证**: JWT + API Key + RBAC (17种权限类型)
- **前端**: React 18 + TypeScript + shadcn/ui (已完成)

## 规范标准
- **代码风格**: 使用 `cargo fmt` 进行代码格式化，遵循 Rust 官方编码规范
- **Linting**: 使用 `cargo clippy` 进行静态代码分析，启用严格的 lint 规则
- **返回值**: 统一使用use crate::error::Result<T>作为函数返回类型
- **错误处理**: 使用自定义的 ProxyError 枚举进行错误分类和处理
- **日志记录**: 使用 `tracing` 进行结构化日志记录

## 常用开发命令

### 项目构建和运行
```bash
# 首次运行会自动初始化数据库（无需手动迁移）
cargo run

# 开发模式运行双端口服务
cargo run

# 后台运行服务（生产环境）
cargo run > /dev/null 2>&1 &

# 发布模式构建
cargo build --release

# 运行所有测试
cargo test

# 运行特定测试模块
cargo test auth::tests
cargo test proxy::tests

# 运行单个测试函数
cargo test test_auth_middleware -- --nocapture

# 运行集成测试
cargo test --test integration_test

# 运行基准测试
cargo bench
```

### 代码质量和维护
```bash
# 代码格式化
cargo fmt

# 严格代码检查（必须符合）
cargo clippy --all-targets -- -D warnings -A clippy::multiple-crate-versions

# 安全审计
cargo audit

# 依赖更新
cargo update

# 检查未使用的依赖
cargo machete

# 部署相关命令
./deploy.sh install              # 完整安装部署
./deploy.sh start                # 启动所有服务
./deploy.sh stop                 # 停止所有服务
./deploy.sh status               # 查看服务运行状态
./deploy.sh logs [proxy|caddy]   # 查看服务日志
./deploy.sh restart              # 重启服务
```

## 核心架构模式

### 数据流架构
```
管理API请求: Client → Pingora(8080) → Router → Axum(9090) → Business Logic → Database/Redis → Response
AI代理请求: Client → Pingora(8080) → Auth → LoadBalancer → UpstreamSelect → ProxyForward → AI Provider → Response → Stats
```

### 认证体系架构
- **四层认证**: JWT令牌 + API密钥 + OAuth 2.0 + RBAC权限控制(17种权限类型)
- **用户对外API**: 每种服务商类型只能创建一个对外API密钥
- **内部API密钥池**: 每种类型可创建多个内部密钥，组成负载均衡池
- **OAuth 2.0集成**: 完整授权流程，支持自动token刷新

### 负载均衡策略
- **轮询调度** (`round_robin`): 按顺序分配请求到不同的API密钥
- **权重调度** (`weighted`): 根据权重比例分配请求
- **健康度最佳** (`health_best`): 选择响应时间最短的健康节点
- **智能调度** (`smart`): SmartApiKeyProvider动态选择，考虑健康度、响应时间、负载等因素

### 追踪系统架构
- **UnifiedTraceSystem**: 统一追踪系统入口，管理所有请求追踪
- **ImmediateProxyTracer**: 即时写入追踪器，确保所有请求都被记录到数据库
- **数据驱动提取**: TokenFieldExtractor和ModelExtractor基于数据库配置提取token和模型信息
- **健康监控**: ApiKeyHealthChecker实时监控API密钥状态，自动故障恢复

## 关键模块说明

### 双端口启动流程 (`src/dual_port_setup.rs`)
1. 初始化共享服务（数据库、缓存、认证、追踪系统、OAuth服务）
2. 并发启动Pingora代理服务 (8080) 和Axum管理服务 (9090)
3. 两个服务共享数据层但职责完全分离

### AI代理处理器 (`src/proxy/request_handler.rs`)
- **核心步骤**: 请求验证 → 密钥选择 → 请求转发 → 响应处理 → 统计记录
- **请求上下文**: ProxyContext包含完整的请求生命周期数据
- **错误处理**: 自动转换Pingora错误为用户友好的响应
- **数据驱动**: 使用数据库配置的TokenFieldExtractor和ModelExtractor

### OAuth 2.0系统 (`src/auth/oauth_client/`)
- **OAuth客户端管理**: 完整的授权码流程和token交换
- **自动token刷新**: OAuthTokenRefreshService和后台刷新任务
- **会话管理**: oauth_client_sessions表管理OAuth会话状态

### 智能密钥管理 (`src/auth/smart_api_key_provider.rs`)
- **动态密钥选择**: SmartApiKeyProvider多种选择策略
- **健康监控**: 实时监控API密钥状态和性能指标
- **故障恢复**: 自动故障检测和恢复机制

### 配置管理系统 (`src/config/`)
- **动态配置**: ProviderConfigManager替代所有硬编码地址
- **热重载**: 支持配置文件变更实时生效
- **多环境**: dev/test/prod配置文件分离

### 缓存抽象层 (`src/cache/`)
- **统一缓存接口**: AbstractCache trait支持内存和Redis后端
- **智能缓存策略**: 不同数据类型使用不同的TTL和缓存策略
- **故障降级**: Redis不可用时自动降级到内存缓存

### 健康监控系统 (`src/scheduler/api_key_health.rs`)
- **实时健康检查**: ApiKeyHealthChecker定期检查API密钥状态
- **性能统计**: 收集响应时间、成功率等关键指标
- **自动恢复**: 故障自动检测和恢复机制

## 开发注意事项

### Rust 2024 Edition特性
- 项目使用Rust 2024 Edition，需要rustc 1.75+
- 启用了严格的linting规则，包括forbid unsafe_code
- 使用workspace结构管理多个crate

### 工作空间结构
```
api-proxy/
├── src/                    # 主应用代码
│   ├── app/               # 应用配置和启动
│   ├── auth/              # 认证授权系统
│   │   ├── oauth_client/  # OAuth 2.0客户端管理
│   │   └── strategies/    # 认证策略实现
│   ├── cache/             # 缓存抽象层
│   ├── config/            # 配置管理系统
│   ├── dual_port_setup.rs # 双端口服务启动
│   ├── error/             # 错误处理框架
│   ├── logging.rs         # 日志系统
│   ├── management/        # Axum管理服务
│   │   ├── handlers/      # API处理器
│   │   ├── middleware/    # 中间件
│   │   └── server.rs      # 服务器配置
│   ├── proxy/             # Pingora代理服务
│   │   └── provider_strategy/ # AI服务适配器
│   ├── scheduler/         # 后台任务调度
│   ├── statistics/        # 统计分析模块
│   ├── trace/             # 请求追踪系统
│   └── utils/             # 工具函数
├── entity/                # 数据库实体定义 (Sea-ORM)
├── migration/             # 数据库迁移脚本
├── web/                   # 前端应用 (React 18 + TypeScript)
├── test/                  # 单元测试
├── tests/                 # 集成测试
├── deploy/                 # 部署脚本
└── docs/                  # 完整架构文档
```

### 数据库设计原则
- 使用Sea-ORM进行类型安全的数据库操作
- 所有敏感数据都经过加密存储
- 支持数据库迁移的向前和向后兼容
- 实现了软删除机制保证数据完整性

### 安全设计要点
- **源信息隐藏**: AI服务商完全无法看到真实客户端信息
- **数据加密**: 敏感配置使用AES-GCM加密存储
- **权限控制**: RBAC系统支持17种细粒度权限类型
- **审计日志**: 所有用户操作都有详细的审计记录

### 性能优化策略
- Pingora原生性能处理AI代理请求
- Redis缓存热点数据减少数据库压力  
- 连接池复用减少连接开销
- 异步处理提高并发能力

## 测试策略

### 测试分类
- **单元测试**: 使用`cargo test`运行模块级测试
- **集成测试**: 测试完整的请求流程和服务间交互
- **性能测试**: 使用criterion进行基准测试
- **安全测试**: 验证认证、授权和数据加密

### 测试环境
- 使用`tempfile`创建临时测试数据库
- 使用`wiremock`模拟外部AI服务响应
- 使用`serial_test`确保数据库测试的隔离性
- 集成测试位于`tests/`目录，单元测试位于各模块的`tests/`子目录

## 故障诊断

### 常见问题
- **追踪数据丢失**: 检查UnifiedTraceSystem是否正确初始化并传递给ProxyServerBuilder
- **数据库连接失败**: 确认数据库路径配置和权限设置
- **Redis连接问题**: 检查Redis服务状态和连接参数
- **编译错误**: 确保Rust版本满足要求，运行`rustup update`

### 日志分析
- 使用结构化日志，关键字段包括request_id, user_id, provider等  
- 错误日志会记录详细的上下文信息和错误堆栈
- 性能日志包含响应时间、token使用量等指标

### 配置验证
```bash
# 验证配置文件语法
cargo run -- --check

# 查看当前配置
curl http://127.0.0.1:9090/api/system/info
```

## 部署指南

### 开发环境
- 确保安装Rust 1.75+和Redis (可选)
- 运行`cargo run`启动双端口服务
- 代理服务: http://localhost:8080，管理服务: http://localhost:9090

### 生产环境
- 使用`cargo build --release`构建优化版本
- 配置SSL证书和域名
- 设置环境变量和生产配置文件
- 使用进程管理工具(systemd/supervisor)管理服务

数据库配置路径优先级: 命令行参数 > 环境变量 > 配置文件 > 默认值

### 部署架构
- **开发环境**: 直接运行`cargo run`启动双端口服务
- **生产环境**: 使用`./deploy.sh install`一键部署，包含Docker容器化和Caddy反向代理
- **TLS支持**: 支持自签名证书(开发)和Let's Encrypt证书(生产)
- **健康检查**: 完整的服务健康状态监控和自动恢复机制