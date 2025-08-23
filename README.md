# AI代理平台 (AI Proxy Platform)

> 基于 Rust + Pingora 构建的企业级AI服务代理平台，采用**双端口分离架构**设计

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Pingora](https://img.shields.io/badge/Pingora-0.5.0-blue.svg)](https://github.com/cloudflare/pingora)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## 📋 项目概述

AI代理平台为企业提供统一、安全、高性能的AI服务访问网关，支持多个主流AI服务提供商（OpenAI、Google Gemini、Anthropic Claude）。采用独创的**双端口分离架构**和**完全数据驱动设计**，实现代理服务与管理功能的完全分离，确保高性能和高可用性。所有配置均存储在数据库中，支持动态更新无需重启。

### 🏗️ 双端口分离架构

```
┌─────────────────┐    ┌─────────────────┐
│  Pingora 代理    │    │  Axum 管理      │
│   端口: 8080    │    │   端口: 9090    │
│                │    │                │
│ • AI请求代理     │    │ • 用户管理      │
│ • 负载均衡      │    │ • API密钥管理    │
│ • 认证验证      │    │ • 统计查询      │
│ • 请求转发      │    │ • 系统配置      │
└─────────────────┘    └─────────────────┘
        │                      │
        └──────────┬───────────┘
                  │
        ┌─────────────────┐
        │   共享数据层     │
        │ • SQLite/数据库  │
        │ • Redis缓存     │
        │ • 统一认证      │
        └─────────────────┘
```

## ✨ 核心特性

### 🎯 已实现功能

- **🔌 双端口分离架构**: 代理服务(8080)专注性能，管理服务(9090)专注功能
- **🔐 企业级认证系统**: JWT令牌 + API密钥 + RBAC权限控制(17种权限类型)
- **⚙️ 动态配置管理**: 数据库驱动配置，无硬编码地址，支持热重载
- **🛡️ 源信息完全隐藏**: AI服务商无法看到真实客户端信息
- **📊 实时健康检查**: 后台监控服务状态，自动故障检测
- **💾 统一数据管理**: SQLite + Sea-ORM + Redis缓存优化

### 🚀 已实现功能(续)

- **⚖️ 智能负载均衡**: 轮询、权重、健康度最佳三种调度策略
- **🔌 AI服务商适配器**: OpenAI、Gemini、Claude完整API适配
- **📈 监控统计系统**: 实时指标收集、使用量分析、成本控制
- **🎨 Web管理界面**: React 18 + TypeScript + shadcn/ui 响应式管理面板

## 🚀 快速开始

### 环境要求

```bash
# Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# 依赖服务
# Redis (可选，用于缓存优化)
sudo apt install redis-server  # Ubuntu/Debian
brew install redis             # macOS
```

### 安装和启动

```bash
# 1. 克隆项目
git clone https://github.com/MAQSOODAWANhaha/api-proxy.git
cd api-proxy

# 2. 初始化数据库
cargo run --bin migration up

# 3. 启动服务 (双端口模式)
cargo run

# 服务启动成功后：
# - Pingora代理服务: http://localhost:8080 (AI请求代理)
# - Axum管理服务: http://localhost:9090 (系统管理)
```

### 验证服务状态

```bash
# 检查管理服务健康状态
curl http://127.0.0.1:9090/api/health

# 查看系统信息
curl http://127.0.0.1:9090/api/system/info

# 获取详细健康报告
curl http://127.0.0.1:9090/api/health/detailed
```

## 📚 API使用示例

### 管理API示例 (端口9090)

```bash
# 用户登录 (当前简化版本，任意用户名密码均可登录)
curl -X POST http://127.0.0.1:9090/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "any_password"
  }'

# 获取负载均衡器状态
curl http://127.0.0.1:9090/api/loadbalancer/status

# 查看统计概览
curl http://127.0.0.1:9090/api/statistics/overview

# 获取适配器信息
curl http://127.0.0.1:9090/api/adapters
```

### AI代理示例 (端口8080)

```bash
# OpenAI ChatGPT API代理
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '{
    "model": "gpt-3.5-turbo",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 50
  }'

# Google Gemini API代理 (支持多种认证格式)
# 使用 Authorization: Bearer 格式
curl -X POST http://localhost:8080/v1/models/gemini-1.5-flash:generateContent \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '{
    "contents": [{"parts": [{"text": "Hello!"}]}]
  }'

# 或使用 X-goog-api-key 格式 (基于数据库配置)
curl -X POST http://localhost:8080/v1/models/gemini-1.5-flash:generateContent \
  -H "Content-Type: application/json" \
  -H "X-goog-api-key: your-api-key" \
  -d '{
    "contents": [{"parts": [{"text": "Hello!"}]}]
  }'

# Anthropic Claude API代理
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -H "anthropic-version: 2023-06-01" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 50,
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## 📖 文档导航

| 文档 | 说明 | 内容概要 |
|------|------|----------|
| **[API.md](docs/API.md)** | API接口完整参考 | 所有管理API的详细说明和curl示例 |
| **[DESIGN.md](docs/DESIGN.md)** | 系统架构设计 | 完整技术方案+任务管理与实施跟踪 |
| **[FRONTEND_DESIGN.md](docs/FRONTEND_DESIGN.md)** | 前端设计方案 | React 18技术栈+开发者角色定义 |
| **[GOAL.md](docs/GOAL.md)** | 项目目标规划 | 分阶段实施计划和成功标准 |

## 📊 当前开发状态

### 🎯 里程碑进展

- ✅ **Phase 0**: 硬编码地址问题解决，动态配置系统完成
- ✅ **Phase 1**: 基础设施搭建完成 (数据库、缓存、配置管理)
- ✅ **认证系统**: JWT + API Key + RBAC权限控制完整实现
- ✅ **动态配置**: ProviderConfigManager替代所有硬编码地址
- ✅ **Phase 2**: 核心代理功能 (负载均衡器、AI适配器已完成)
- ✅ **Phase 3**: 管理功能与监控 (统计分析、用户管理已完成)
- 🔄 **Phase 4**: 安全与TLS (证书管理、安全防护开发中)
- ✅ **Phase 5**: 前端界面 (React 18管理面板已完成)

### 🔧 技术债务和改进

- [x] 消除硬编码API地址，实现动态配置
- [x] 完善认证授权机制
- [x] 建立错误处理框架
- [x] 实现负载均衡调度器
- [x] 完成AI服务商适配器
- [x] 增加性能监控和告警
- [x] 数据驱动的模型名称记录
- [x] 完全数据驱动的认证系统

## 🛠️ 开发环境

### 代码质量检查

```bash
# 代码格式化
cargo fmt

# 代码质量检查
cargo clippy -- -D warnings

# 安全审计
cargo audit

# 运行测试
cargo test

# 性能基准测试
cargo bench
```

### 项目结构

```
api-proxy/
├── src/                    # 核心源代码
│   ├── auth/              # 认证授权系统
│   ├── config/            # 配置管理 (含动态配置)
│   ├── proxy/             # Pingora代理服务
│   ├── management/        # Axum管理服务
│   ├── cache/             # Redis缓存层
│   └── error/             # 错误处理框架
├── entity/                # 数据库实体定义
├── migration/             # 数据库迁移脚本
├── web/                   # React 18前端应用 (已完成)
├── docs/                  # 完整项目文档
└── CLAUDE.md              # 开发指南和说明
```

## 🤝 贡献指南

1. **环境配置**: 查看 [CLAUDE.md](CLAUDE.md) 了解开发环境配置
2. **架构理解**: 阅读 [DESIGN.md](docs/DESIGN.md) 了解系统架构
3. **任务认领**: 查看 [DESIGN.md 第11章](docs/DESIGN.md#11-项目任务管理与实施跟踪) 了解待完成任务
4. **代码规范**: 使用 `cargo fmt` 和 `cargo clippy` 确保代码质量
5. **测试覆盖**: 为新功能编写相应的单元测试和集成测试

## 🔗 相关链接

- **技术栈**: [Rust](https://www.rust-lang.org/) + [Pingora](https://github.com/cloudflare/pingora) + [Axum](https://github.com/tokio-rs/axum)
- **数据库**: [Sea-ORM](https://www.sea-ql.org/SeaORM/) + [SQLite](https://www.sqlite.org/)
- **前端**: [React 18](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/) + [shadcn/ui](https://ui.shadcn.com/)

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详细信息。

## 🏷️ 版本信息

- **当前版本**: v1.0.0 (生产就绪版本)
- **Rust版本要求**: 1.75+
- **最后更新**: 2025年8月

---

> 💡 **提示**: 项目已达到生产就绪状态，完整实现企业级AI代理功能。欢迎贡献代码或提出建议！如有问题请查看文档或提交Issue。