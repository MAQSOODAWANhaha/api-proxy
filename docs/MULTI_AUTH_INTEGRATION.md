# 多认证方式集成技术方案（优化版）

## 项目概述

本方案旨在扩展现有API代理平台，支持Claude Max订阅的OAuth2.0认证以及Gemini CLI的多种认证方式，提供统一的多provider认证管理系统。基于简洁设计原则，采用最小修改策略，避免数据库字段膨胀和架构过度复杂。

## 认证方式支持矩阵

| Provider | OAuth2.0 | API Key | Service Account | ADC | 说明 |
|----------|----------|---------|------------------|-----|------|
| Claude Max | ✅ | ✅ | ❌ | ❌ | 订阅用户主要使用OAuth |
| Gemini CLI | ✅ | ✅ | ✅ | ✅ | 支持Google全套认证方式 |
| OpenAI | ❌ | ✅ | ❌ | ❌ | 仅支持API Key |
| Anthropic | ❌ | ✅ | ❌ | ❌ | 仅支持API Key |

## 技术架构设计

### 1. 认证类型枚举

```rust
#[derive(Debug, Clone, Serialize, Deserialize, EnumString)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// 传统API密钥认证
    ApiKey,
    /// OAuth2.0认证 (Claude Max)
    OAuth2,
    /// Google OAuth认证 (Gemini CLI个人账户)
    GoogleOAuth,
    /// Google服务账户认证
    ServiceAccount,
    /// Google应用默认凭据 (ADC)
    ADC,
}
```

### 2. 数据库结构设计

> **JSONB类型说明**: 使用JSONB而非TEXT类型存储JSON数据，提供更好的语义清晰度和未来数据库兼容性。在SQLite中实际存储为TEXT，但支持完整的JSON查询功能。

#### 2.1 provider_types表扩展
```sql
-- 添加支持的认证类型配置
ALTER TABLE provider_types ADD COLUMN supported_auth_types JSONB;
ALTER TABLE provider_types ADD COLUMN auth_config_json JSONB;

-- 移除冗余字段（合并到auth_config_json中）
-- 注意：auth_header_format功能已整合到auth_config_json.auth_configs中

-- 索引优化（支持JSON查询）
CREATE INDEX idx_provider_types_supported_auth ON provider_types(supported_auth_types);
```

#### 2.2 user_provider_keys表扩展
```sql
-- 添加认证类型和统一配置字段
ALTER TABLE user_provider_keys ADD COLUMN auth_type VARCHAR(30) NOT NULL DEFAULT 'api_key';
ALTER TABLE user_provider_keys ADD COLUMN auth_config_json JSONB;
ALTER TABLE user_provider_keys ADD COLUMN auth_status VARCHAR(20) DEFAULT 'pending';
ALTER TABLE user_provider_keys ADD COLUMN expires_at DATETIME;
ALTER TABLE user_provider_keys ADD COLUMN last_auth_check DATETIME;

-- 索引优化
CREATE INDEX idx_user_provider_keys_auth_type ON user_provider_keys(auth_type);
CREATE INDEX idx_user_provider_keys_auth_status ON user_provider_keys(auth_status);
CREATE INDEX idx_user_provider_keys_expires_at ON user_provider_keys(expires_at);
```

#### 2.3 新增oauth_sessions表
```sql
CREATE TABLE oauth_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id VARCHAR(64) UNIQUE NOT NULL,
    user_id INTEGER NOT NULL,
    provider_type_id INTEGER NOT NULL,
    auth_type VARCHAR(30) NOT NULL,
    state VARCHAR(64) NOT NULL,
    code_verifier VARCHAR(128),
    code_challenge VARCHAR(128),
    redirect_uri TEXT NOT NULL,
    scopes TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    completed_at DATETIME,
    error_message TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (provider_type_id) REFERENCES provider_types(id) ON DELETE CASCADE
);

CREATE INDEX idx_oauth_sessions_session_id ON oauth_sessions(session_id);
CREATE INDEX idx_oauth_sessions_user_provider ON oauth_sessions(user_id, provider_type_id);
CREATE INDEX idx_oauth_sessions_expires_at ON oauth_sessions(expires_at);
```

### 3. 核心组件架构

#### 3.1 统一认证配置设计

**配置架构优化**：移除冗余的`auth_header_format`字段，统一使用`auth_config_json`管理所有认证配置。这样可以：
- 避免数据库字段膨胀
- 提供更灵活的配置扩展
- 简化认证策略实现
- 统一配置管理接口

#### 3.2 统一认证管理器

```rust
/// 统一认证管理器 - 基于策略模式的简洁设计
pub struct AuthManager {
    db: Arc<DatabaseConnection>,
    cache: Arc<dyn AbstractCache>,
    crypto: Arc<CryptoService>,
    
    // 认证策略注册表
    strategies: HashMap<AuthType, Box<dyn AuthStrategy>>,
}

impl AuthManager {
    /// 根据provider配置自动选择认证方式并处理请求
    pub async fn authenticate_request(&self, ctx: &mut ProxyContext) -> Result<()>;
    
    /// 启动OAuth认证流程
    pub async fn initiate_oauth_flow(&self, 
        user_id: i32, 
        provider_id: i32, 
        auth_type: AuthType
    ) -> Result<String>; // 返回授权URL
    
    /// 处理OAuth回调
    pub async fn handle_oauth_callback(&self, 
        session_id: &str, 
        code: &str
    ) -> Result<i32>; // 返回provider_key_id
    
    /// 刷新认证凭据
    pub async fn refresh_credentials(&self, provider_key_id: i32) -> Result<()>;
    
    /// 验证认证状态
    pub async fn validate_auth_status(&self, provider_key_id: i32) -> Result<AuthStatus>;
}
```

#### 3.3 认证策略接口

```rust
/// 认证策略抽象接口 - 简化设计
#[async_trait]
pub trait AuthStrategy: Send + Sync {
    /// 认证类型标识
    fn auth_type(&self) -> AuthType;
    
    /// 准备请求认证信息
    async fn prepare_request(&self, ctx: &mut ProxyContext) -> Result<()>;
    
    /// 验证认证凭据有效性
    async fn validate_credentials(&self, config: &serde_json::Value) -> Result<bool>;
    
    /// 刷新认证凭据（如果支持）
    async fn refresh_credentials(&self, config: &mut serde_json::Value) -> Result<()>;
}

/// API Key认证策略
pub struct ApiKeyStrategy;

/// OAuth2认证策略（Claude Max）
pub struct OAuth2Strategy {
    crypto: Arc<CryptoService>,
}

/// Google OAuth认证策略
pub struct GoogleOAuthStrategy {
    crypto: Arc<CryptoService>,
}

/// Google服务账户认证策略
pub struct ServiceAccountStrategy {
    crypto: Arc<CryptoService>,
}

/// Google ADC认证策略
pub struct ADCStrategy;
```

### 4. Provider配置方案

**配置统一说明**：所有认证相关配置都统一存储在`auth_config_json`字段中，包括原来`auth_header_format`的功能。每个认证类型都有对应的配置结构，确保配置的完整性和一致性。

#### 4.1 Claude Max Provider配置
```json
{
  "name": "claude-max",
  "display_name": "Claude Max订阅",
  "base_url": "api.anthropic.com",
  "api_format": "anthropic",
  "supported_auth_types": ["oauth2", "api_key"],
  "auth_config_json": {
    "oauth2": {
      "authorization_url": "https://auth.anthropic.com/oauth2/authorize",
      "token_url": "https://auth.anthropic.com/oauth2/token",
      "scopes": ["claude.read", "claude.chat"],
      "pkce_required": true
    },
    "api_key": {
      "header_format": "Authorization: Bearer {key}"
    }
  }
}
```

#### 4.2 Gemini CLI Provider配置
```json
{
  "name": "gemini-cli",
  "display_name": "Google Gemini CLI",
  "base_url": "generativelanguage.googleapis.com",
  "api_format": "gemini_rest",
  "supported_auth_types": ["google_oauth", "service_account", "adc", "api_key"],
  "auth_config_json": {
    "google_oauth": {
      "authorization_url": "https://accounts.google.com/o/oauth2/auth",
      "token_url": "https://oauth2.googleapis.com/token",
      "scopes": ["https://www.googleapis.com/auth/generative-language"]
    },
    "service_account": {
      "token_url": "https://oauth2.googleapis.com/token",
      "scopes": ["https://www.googleapis.com/auth/generative-language"]
    },
    "adc": {
      "scopes": ["https://www.googleapis.com/auth/generative-language"]
    },
    "api_key": {
      "header_format": "X-goog-api-key: {key}"
    }
  }
}
```

### 5. API接口设计

#### 5.1 OAuth认证端点

```yaml
# 启动OAuth认证流程
POST /api/oauth/authorize:
  body:
    provider_type_id: integer
    auth_type: string # oauth2, google_oauth
    name: string
  response:
    authorization_url: string
    session_id: string

# 处理OAuth回调
GET /api/oauth/callback/{provider_type}:
  parameters:
    - code: string
    - state: string
  response:
    success: boolean
    provider_key_id: integer
```

#### 5.2 Provider Keys管理扩展

```yaml
# 创建认证配置
POST /api/provider-keys:
  body:
    provider_type_id: integer
    auth_type: string
    name: string
    auth_config_json: object # 根据auth_type包含不同配置
  response:
    id: integer
    authorization_url: string (optional) # OAuth类型才有

# 获取用户的provider keys
GET /api/provider-keys:
  parameters:
    - provider_type_id: integer (optional)
  response:
    - id: integer
      name: string
      provider_type: object
      auth_type: string
      auth_status: string # pending, authorized, expired, error
      expires_at: datetime (optional)
      last_used_at: datetime

# 刷新认证凭据
POST /api/provider-keys/{id}/refresh:
  response:
    success: boolean
    new_expires_at: datetime (optional)
```

### 6. 前端界面设计

#### 6.1 认证类型选择器

```typescript
interface AuthTypeSelector {
  supportedAuthTypes: string[];
  selectedAuthType: string;
  onAuthTypeChange: (authType: string) => void;
}

const AuthTypeSelector: React.FC<AuthTypeSelector> = ({
  supportedAuthTypes,
  selectedAuthType,
  onAuthTypeChange
}) => {
  return (
    <Select
      label="认证方式"
      value={selectedAuthType}
      onChange={onAuthTypeChange}
      options={supportedAuthTypes.map(type => ({
        value: type,
        label: getAuthTypeDisplayName(type)
      }))}
    />
  );
};
```

#### 6.2 多认证配置表单

```typescript
interface AuthConfigForm {
  authType: string;
  config: Record<string, any>;
  onConfigChange: (config: Record<string, any>) => void;
}

const AuthConfigForm: React.FC<AuthConfigForm> = ({
  authType,
  config,
  onConfigChange
}) => {
  // 根据认证类型渲染不同的配置表单
  switch (authType) {
    case 'api_key':
      return <ApiKeyForm config={config} onChange={onConfigChange} />;
    case 'oauth2':
    case 'google_oauth':
      return <div>OAuth认证将跳转到授权页面</div>;
    case 'service_account':
      return <ServiceAccountForm config={config} onChange={onConfigChange} />;
    case 'adc':
      return <ADCStatusForm />;
    default:
      return null;
  }
};
```

## 实施计划（5周完成）

### Phase 1: 基础架构搭建 (Week 1)
- 数据库迁移：扩展provider_types和user_provider_keys表
- 创建oauth_sessions表
- 更新Entity模型和认证类型枚举
- 实现AuthManager基础架构和AuthStrategy接口

### Phase 2: 多认证策略实现 (Week 2)
- 实现所有认证策略：ApiKey, OAuth2, GoogleOAuth, ServiceAccount, ADC
- 集成OAuth2流程和PKCE支持
- 实现token加密存储和刷新机制

### Phase 3: API接口开发 (Week 3)
- 开发OAuth认证端点(/api/oauth/authorize, /api/oauth/callback)
- 扩展Provider Keys管理API支持多认证类型
- 集成认证中间件到代理层

### Phase 4: 前端界面开发 (Week 4)
- 实现认证类型选择器和配置表单
- 更新API Keys管理页面支持多认证
- 集成OAuth授权流程到前端

### Phase 5: 测试和部署 (Week 5)
- 单元测试和集成测试
- 安全性验证和性能优化
- 文档更新和部署上线

## 核心原则

### 1. 安全设计
- OAuth流程使用PKCE和State参数防护
- 所有敏感数据AES-GCM加密存储
- 强制HTTPS传输和证书验证

### 2. 向后兼容
- 现有API Key认证完全保持不变
- 新字段都有合理默认值
- 支持渐进式迁移

### 3. 简洁架构
- 基于策略模式的统一设计
- JSON配置避免数据库字段膨胀
- 最小化组件复杂度

该优化方案通过简洁的设计实现多认证方式支持，确保架构清晰、维护性强，同时为未来扩展奠定基础。