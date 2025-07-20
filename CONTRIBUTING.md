# 贡献指南

欢迎为 AI Proxy 项目做出贡献！

## 开发工作流

### 分支策略

- `main`: 主分支，保存生产就绪的代码
- `develop`: 开发分支，用于集成新功能
- `feature/*`: 功能分支，用于开发新功能
- `bugfix/*`: 错误修复分支
- `hotfix/*`: 紧急修复分支

### 提交规范

使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

#### 类型说明

- `feat`: 新功能
- `fix`: 错误修复
- `docs`: 文档更新
- `style`: 代码格式化（不影响功能）
- `refactor`: 代码重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建工具、辅助工具等
- `ci`: CI/CD 相关

#### 示例

```bash
feat(auth): add JWT token validation
fix(proxy): resolve memory leak in request forwarding
docs: update API documentation
test: add integration tests for Redis cache
```

### 开发流程

1. **Fork 项目并克隆**
   ```bash
   git clone https://github.com/your-username/api-proxy.git
   cd api-proxy
   ```

2. **创建功能分支**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **安装开发依赖**
   ```bash
   # 安装 Rust（如果还未安装）
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # 安装开发工具
   cargo install cargo-audit cargo-tarpaulin
   
   # 安装 pre-commit
   pip install pre-commit
   pre-commit install
   ```

4. **开发和测试**
   ```bash
   # 运行格式化
   cargo fmt
   
   # 运行 Clippy 检查
   cargo clippy --all-targets --all-features
   
   # 运行测试
   cargo test --all-features
   
   # 运行安全审计
   cargo audit
   ```

5. **提交代码**
   ```bash
   git add .
   git commit -m "feat: add your new feature"
   ```

6. **推送并创建 Pull Request**
   ```bash
   git push origin feature/your-feature-name
   ```

## 代码质量要求

### Rust 代码规范

- 使用 `cargo fmt` 进行代码格式化
- 通过 `cargo clippy` 的所有检查
- 添加必要的文档注释
- 为新功能编写测试
- 遵循 Rust 命名约定

### 测试要求

- 单元测试覆盖率 > 80%
- 为新功能添加集成测试
- 确保所有测试通过
- 测试数据使用 fixtures

### 文档要求

- 公共 API 必须有文档注释
- 复杂逻辑需要添加内联注释
- 更新相关的 README 和设计文档

## Pull Request 检查清单

提交 PR 前请确保：

- [ ] 代码通过 `cargo fmt --check`
- [ ] 代码通过 `cargo clippy --all-targets --all-features`
- [ ] 所有测试通过 `cargo test --all-features`
- [ ] 通过安全审计 `cargo audit`
- [ ] 添加了适当的测试
- [ ] 更新了相关文档
- [ ] 提交消息符合规范
- [ ] PR 描述清晰明确

## 性能要求

- 新功能不应显著影响现有性能
- 数据库查询应该优化
- 添加适当的缓存策略
- 使用 criterion 进行性能测试

## 安全要求

- 不得硬编码敏感信息
- 输入验证必须充分
- 使用安全的密码学库
- 定期运行安全审计

## 获得帮助

如有问题，请通过以下方式寻求帮助：

- 查看项目文档
- 搜索现有 Issues
- 创建新的 Issue
- 联系项目维护者

感谢您的贡献！🎉