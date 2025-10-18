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

### 数据流架构
```
管理API请求: Client → Pingora(8080) → Router → Axum(9090) → Business Logic → Database/Redis → Response
AI代理请求: Client → Pingora(8080) → Auth → ProviderStrategy → Request/Response Transform → Upstream → AI Provider → Collect → Trace
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

### Collect & Trace 架构
- **CollectService**: 负责请求/响应数据采集、用量聚合、成本计算
- **TraceManager**: 统一追踪入口，协调限流缓存、成本与错误记录
- **ImmediateProxyTracer**: 即时写入追踪器，确保所有请求被持久化
- **数据驱动提取**: `TokenFieldExtractor` 基于数据库配置提取 token 与模型信息
- **健康监控**: ApiKeyHealthChecker 实时监控 API 密钥状态，自动故障恢复

## 开发规范标准

### 沟通语言
**使用中文进行所有对话和代码注释**

### 代码质量要求
每次代码修改后，必须按以下顺序完成所有检查：

1. **编译检查**：`cargo build`
2. **代码格式化**：`cargo fmt`
3. **静态分析**：`cargo clippy --all-targets -- -D warnings`
4. **单元测试**：`cargo test`

**注意**：以上所有步骤都必须通过，不允许提交未通过检查的代码。

### 代码风格规范
- **返回值类型**：统一使用 `crate::error::Result<T>` 作为函数返回类型
- **错误处理**：使用自定义的 ProxyError 枚举进行错误分类和处理
- **日志记录**：使用 `tracing` 进行结构化日志记录

### 错误处理最佳实践（参见 `src/error/`）
- **统一返回类型**：所有可能失败的接口都应返回 `crate::error::Result<T>`。
- **领域枚举优先**：使用 `error!(Authentication, ...)` 等宏构造错误。
- **上下文增强**：使用 `Context`/`with_context` trait 补充定位信息。
- **快速返回**：使用 `ensure!` 和 `bail!` 减少样板代码。
- **稳定错误码**：在 `ProxyError::error_code`/`status_code` 中维护对外编号与状态码。

### 日志记录最佳实践（参见 `src/logging.rs`）
- **统一宏**：使用 `linfo!`、`ldebug!`、`lwarn!`、`lerror!`。
- **基础字段**：日志必须包含 `request_id`、`stage`、`component`、`operation`、`message`。
- **阶段 & 组件选择**：根据上下文选用合适的 `LogStage`、`LogComponent`。
- **错误日志**：捕获 `ProxyError` 时调用 `error.log()` 输出结构化错误信息。
- **初始化配置**：在 `init_logging` 中统一修改日志配置。

## 工作空间与关键模块

### 工作空间结构
```
.
├── src/                    # 主应用代码
│   ├── app/               # 应用配置和启动
│   ├── auth/              # 认证授权系统
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
│   ├── collect/           # 请求/响应采集与用量解析
│   ├── key_pool/          # 密钥池与健康检查
│   ├── trace/             # 请求追踪系统
│   └── utils/             # 工具函数
├── entity/                # 数据库实体定义 (Sea-ORM)
├── migration/             # 数据库迁移脚本
├── web/                   # 前端应用 (React 18 + TypeScript)
├── tests/                 # 集成测试
├── deploy/                # 部署脚本
└── docs/                  # 完整架构文档
```

### 关键模块说明
- **双端口启动流程 (`src/dual_port_setup.rs`)**: 并发启动Pingora代理服务 (8080) 和Axum管理服务 (9090)，共享数据层。
- **AI代理编排器 (`src/proxy/service.rs`)**: 核心步骤包括请求验证、密钥选择、上游转发、响应转换、Collect和Trace。
- **OAuth 2.0系统 (`src/auth/oauth_client/`)**: 完整的授权码流程、自动token刷新和会话管理。
- **智能密钥管理 (`src/auth/smart_api_key_provider.rs`)**: 提供多种动态密钥选择策略和自动故障恢复。
- **配置管理系统 (`src/config/`)**: 处理多环境配置，支持热重载。
- **缓存抽象层 (`src/cache/`)**: 提供统一缓存接口，支持内存和Redis后端，并具备故障降级能力。
- **健康监控系统 (`src/key_pool/api_key_health.rs`)**: 定期检查API密钥状态，收集性能指标并实现自动恢复。

## 常用开发命令

### 项目构建和运行
```bash
# 开发模式运行双端口服务 (首次运行会自动初始化数据库)
cargo run

# 发布模式构建
cargo build --release

# 运行所有测试
cargo test

# 运行特定测试
cargo test auth::tests::test_auth_middleware -- --nocapture
```

### 其他维护命令
```bash
# 安全审计
cargo audit

# 检查未使用的依赖
cargo machete

# 部署管理
./deploy.sh install             # 完整安装部署
./deploy.sh start               # 启动所有服务
./deploy.sh stop                # 停止所有服务
./deploy.sh status              # 查看服务运行状态
```

## 测试与部署

### 测试策略
- **分类**: 单元测试 (`cargo test`)、集成测试 (`tests/`)、性能测试 (`criterion`)、安全测试。
- **环境**: 使用 `tempfile` 创建临时数据库，`wiremock` 模拟外部服务，`serial_test` 保证测试隔离。

### 部署指南
- **开发环境**: 运行 `cargo run` 启动双端口服务 (代理: 8080, 管理: 9090)。
- **生产环境**: 使用 `./deploy.sh install` 一键部署，包含Docker容器化和Caddy反向代理。
- **数据库配置路径优先级**: 命令行参数 > 环境变量 > 配置文件 > 默认值。

## 安全与性能

### 安全设计要点
- **源信息隐藏**: AI服务商完全无法看到真实客户端信息。
- **数据加密**: 敏感配置使用AES-GCM加密存储。
- **权限控制**: RBAC系统支持17种细粒度权限类型。
- **审计日志**: 所有用户操作都有详细的审计记录。

### 性能优化策略
- Pingora原生性能处理AI代理请求。
- Redis缓存热点数据减少数据库压力。
- 连接池复用减少连接开销。
- 异步处理提高并发能力。

## 故障诊断

### 常见问题
- **追踪数据丢失**: 检查 `UnifiedTraceSystem` 是否正确初始化。
- **数据库连接失败**: 确认数据库路径配置和权限。
- **Redis连接问题**: 检查Redis服务状态和连接参数。
- **编译错误**: 确保Rust版本为1.75+ (`rustup update`)。

### 日志与配置
- **日志分析**: 使用结构化日志，关键字段包括 `request_id`, `user_id`, `provider`。
- **配置验证**: 运行 `cargo run -- --check` 验证配置文件语法。
- **查看配置**: `curl http://127.0.0.1:9090/api/system/info`