# AI Proxy System

Enterprise-grade AI service proxy platform built with Rust and Pingora.

## Overview

The AI Proxy System provides a unified, secure, and high-performance gateway to multiple AI service providers including OpenAI, Google Gemini, and Anthropic Claude. It features intelligent load balancing, comprehensive monitoring, and enterprise-grade security.

## Features

- **Unified API Gateway**: Single endpoint for multiple AI providers
- **Intelligent Load Balancing**: Round-robin, weighted, and health-based scheduling
- **Enterprise Security**: TLS encryption, certificate auto-renewal, request filtering
- **Comprehensive Monitoring**: Real-time metrics, health checks, usage analytics
- **High Performance**: Built with Rust and Pingora for maximum throughput
- **Easy Deployment**: Single service deployment with minimal operational overhead

## Architecture

- **Core Framework**: Rust + Pingora (unified entry point)
- **Management API**: Axum (embedded HTTP service)
- **Database**: SQLite + Sea-ORM
- **Cache**: Redis
- **Frontend**: Vue 3 + TypeScript + Element Plus

## Development Status

🚧 **Currently in active development**

- ✅ Phase 1: Infrastructure setup (in progress)
- 🔄 Phase 2: Pingora integration & proxy core
- 📋 Phase 3: Management features & monitoring
- 📋 Phase 4: Security & TLS
- 📋 Phase 5: Frontend development
- 📋 Phase 6: Testing & optimization

## Getting Started

### Prerequisites

- Rust 1.75+
- Redis (for caching)
- SQLite (for data storage)

### Development

```bash
# Clone the repository
git clone https://github.com/your-org/api-proxy.git
cd api-proxy

# Check code quality
cargo clippy
cargo fmt --check
cargo audit

# Run the development server
cargo run
```

## Documentation

- [Design Document](docs/DESIGN.md) - Complete system architecture and design
- [Goal Document](docs/GOAL.md) - Project goals and implementation roadmap
- [Development Guide](CLAUDE.md) - Development setup and guidelines

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Please read our development guidelines in [CLAUDE.md](CLAUDE.md) before contributing.