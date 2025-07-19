# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个基于 Rust + Pingora 的企业级 AI 服务代理平台，为用户提供统一的 AI 服务访问接口，支持多个主流 AI 服务提供商（OpenAI、Google Gemini、Anthropic Claude），具备负载均衡、监控统计、安全防护等功能。

## 核心架构

### 技术栈
- **核心框架**: Rust + Pingora (统一入口)
- **管理API**: Axum (内嵌在Pingora中)
- **数据库**: SQLite + Sea-ORM
- **缓存**: Redis
- **TLS**: rustls + acme-lib

### 架构设计原理
- **单一入口**: Pingora 作为系统唯一入口点，处理所有 TLS 终止和证书管理
- **职责分离**: 管理功能通过内嵌 Axum 实现，代理功能通过 Pingora 原生能力实现
- **路由分发**: 
  - 管理API请求: `/api/*`, `/admin/*`, `/`
  - AI代理请求: `/v1/*`, `/proxy/*`

## 常用命令

### 构建和运行
```bash
# 编译项目
cargo build

# 开发模式运行
cargo run

# 发布模式构建
cargo build --release

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

### 开发工具
```bash
# 检查代码质量
cargo clippy -- -D warnings

# 安全审计
cargo audit

# 依赖更新
cargo update
```

## 核心模块结构

根据设计文档，项目主要包含以下核心模块：

1. **Pingora 统一入口层**
   - TLS 管理和证书自动续期
   - 智能路由分发
   - 监控统计

2. **管理服务层 (Axum)**
   - 用户管理系统
   - API 密钥管理
   - 统计查询接口

3. **AI 代理核心**
   - 负载均衡调度器
   - 健康检查机制
   - 请求转发和响应处理

4. **共享状态层**
   - SQLite 数据库 (用户数据、配置数据、统计数据)
   - Redis 缓存 (健康状态、统计数据、负载均衡状态)
   - 文件存储 (TLS证书、日志文件)

## 数据流模式

### 管理API请求流程
```
Client → Pingora → Router → Axum → Business Logic → Database/Redis → Response
```

### AI代理请求流程
```
Client → Pingora → Auth → LoadBalancer → UpstreamSelect → ProxyForward → AI Provider → Response → Stats
```

## 开发注意事项

- 项目采用 Rust 2024 Edition
- 当前处于早期开发阶段，main.rs 仅包含基础 Hello World
- 需要根据 docs/DESIGN.md 中的详细设计实现各个模块
- 所有外部 AI 服务商信息都会被隐藏，确保客户端信息安全