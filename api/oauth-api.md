# OAuth认证接口文档

## 概述

本文档描述了OAuth认证相关的接口，用于实现Claude Max、Google等服务的OAuth 2.0认证流程。支持PKCE (Proof Key for Code Exchange)授权码流程，提供完整的OAuth认证生命周期管理。

## 认证

所有接口都需要用户认证。

---

## 启动OAuth授权流程

### 接口信息
- **请求路由**: `POST /api/oauth/authorize`
- **请求方法**: POST
- **作用**: 启动OAuth授权流程，生成授权URL和会话

### 请求参数
```json
{
    "provider_type_id": 3,
    "auth_type": "oauth2",
    "name": "My Claude OAuth Key",
    "description": "Claude API OAuth认证",
    "redirect_uri": "http://localhost:9090/oauth/callback"
}
```

| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| provider_type_id | int | 是 | 服务商类型ID |
| auth_type | string | 是 | 认证类型 (oauth) |
| name | string | 是 | API Key名称 |
| description | string | 否 | 描述信息 |
| redirect_uri | string | 否 | 重定向URI，默认使用系统配置 |

### 响应格式
```json
{
    "success": true,
    "data": {
        "authorization_url": "https://claude.ai/oauth/authorize?client_id=xxx&state=xxx&code_challenge=xxx",
        "session_id": "uuid-session-id",
        "state": "random-state-string",
        "expires_at": "2024-01-01T01:00:00Z"
    },
    "message": "OAuth授权会话创建成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

### 字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| authorization_url | string | 完整的OAuth授权URL，用于跳转 |
| session_id | string | OAuth会话ID，用于后续回调处理 |
| state | string | CSRF防护state参数 |
| expires_at | string | 会话过期时间 |

---

## 处理OAuth回调

### 接口信息
- **请求路由**: `GET /api/oauth/callback`
- **请求方法**: GET
- **作用**: 处理OAuth服务商的授权回调，完成token交换

### 查询参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| code | string | 是 | 授权码 |
| state | string | 是 | 状态参数，用于验证 |
| session_id | string | 否 | 会话ID（部分服务商通过此传递） |

### 响应格式
```json
{
    "success": true,
    "data": {
        "provider_key_id": 123,
        "key_name": "My Claude OAuth Key",
        "auth_status": "authorized",
        "expires_at": "2024-12-31T23:59:59Z",
        "created_at": "2024-01-01T00:00:00Z"
    },
    "message": "OAuth授权完成",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

### 字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| provider_key_id | int | 创建的用户提供商密钥ID |
| key_name | string | API Key名称 |
| auth_status | string | 认证状态 (authorized, expired, error) |
| expires_at | string | 访问令牌过期时间 |
| created_at | string | 创建时间 |

---

## 查询OAuth状态

### 接口信息
- **请求路由**: `GET /api/oauth/status/{session_id}`
- **请求方法**: GET
- **作用**: 查询OAuth会话的当前状态

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| session_id | string | 是 | OAuth会话ID |

### 响应格式
```json
{
    "success": true,
    "data": {
        "session_id": "uuid-session-id",
        "status": "pending",
        "provider_type_id": 3,
        "auth_type": "oauth2",
        "created_at": "2024-01-01T00:00:00Z",
        "expires_at": "2024-01-01T01:00:00Z"
    },
    "message": "获取OAuth状态成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

### 会话状态值
| 状态 | 描述 |
|------|------|
| pending | 等待用户授权 |
| authorized | 授权完成 |
| expired | 会话已过期 |
| error | 授权失败 |
| cancelled | 用户取消授权 |

---

## 刷新OAuth访问令牌

### 接口信息
- **请求路由**: `POST /api/oauth/refresh`
- **请求方法**: POST
- **作用**: 使用刷新令牌获取新的访问令牌

### 请求参数
```json
{
    "provider_key_id": 123
}
```

| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| provider_key_id | int | 是 | 用户提供商密钥ID |

### 响应格式
```json
{
    "success": true,
    "data": {
        "provider_key_id": 123,
        "new_expires_at": "2024-12-31T23:59:59Z",
        "refreshed_at": "2024-01-01T00:00:00Z"
    },
    "message": "Token刷新成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 撤销OAuth授权

### 接口信息
- **请求路由**: `DELETE /api/oauth/revoke/{key_id}`
- **请求方法**: DELETE
- **作用**: 撤销OAuth授权，删除访问令牌

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| key_id | int | 是 | 用户提供商密钥ID |

### 响应格式
```json
{
    "success": true,
    "data": {
        "provider_key_id": 123,
        "revoked_at": "2024-01-01T00:00:00Z"
    },
    "message": "OAuth授权已撤销",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## OAuth回调页面

### 特殊页面
- **页面路由**: `GET /oauth/callback`
- **作用**: OAuth回调处理页面，用于postMessage通信

此页面主要用于：
1. 接收OAuth服务商的回调
2. 调用后端API处理授权码
3. 通过postMessage将结果发送给父窗口
4. 自动关闭弹窗

### postMessage消息格式

#### 授权成功
```javascript
{
    type: 'OAUTH_SUCCESS',
    data: {
        provider_key_id: 123,
        key_name: "My Claude OAuth Key",
        auth_status: "authorized"
    }
}
```

#### 授权失败
```javascript
{
    type: 'OAUTH_ERROR',
    error: {
        code: 'AUTH_FAILED',
        message: '授权失败的具体原因'
    }
}
```

#### 用户取消
```javascript
{
    type: 'OAUTH_CANCEL'
}
```

---

## 错误处理

### 常见错误响应
```json
{
    "success": false,
    "data": null,
    "message": "错误描述信息",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

### 错误码说明
| 错误码 | 描述 | HTTP状态码 |
|--------|------|------------|
| INVALID_PROVIDER | 无效的服务商类型 | 400 |
| UNSUPPORTED_AUTH_TYPE | 不支持的认证类型 | 400 |
| INVALID_SESSION | 无效的OAuth会话 | 400 |
| SESSION_EXPIRED | OAuth会话已过期 | 400 |
| INVALID_STATE | 无效的state参数 | 400 |
| AUTH_CODE_EXPIRED | 授权码已过期 | 400 |
| TOKEN_EXCHANGE_FAILED | Token交换失败 | 500 |
| REFRESH_TOKEN_INVALID | 刷新令牌无效 | 400 |

---

## 安全考虑

### PKCE流程
- 使用SHA256生成code_challenge
- code_verifier随机生成，安全存储
- 防止授权码拦截攻击

### CSRF防护
- state参数随机生成
- 回调时验证state一致性
- 防止跨站请求伪造攻击

### 会话管理
- OAuth会话有效期限制（默认15分钟）
- 过期会话自动清理
- 防止会话重放攻击

### Token安全
- 访问令牌和刷新令牌加密存储
- 敏感信息不出现在日志中
- 支持令牌自动刷新机制

---

## 使用场景

### 典型OAuth流程
1. 前端调用 `/api/oauth/authorize` 获取授权URL
2. 弹出新窗口到授权URL进行用户授权
3. 用户完成授权后回调到 `/oauth/callback` 页面
4. 回调页面处理授权码并通过postMessage通知父窗口
5. 前端接收结果并更新UI状态

### 集成示例
```javascript
// 启动OAuth流程
const response = await fetch('/api/oauth/authorize', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        provider_type_id: 3,
        auth_type: 'oauth2',
        name: 'My OAuth Key'
    })
})

const { authorization_url } = await response.json()

// 打开弹窗
const popup = window.open(authorization_url, 'oauth', 'width=600,height=700')

// 监听结果
window.addEventListener('message', (event) => {
    if (event.data.type === 'OAUTH_SUCCESS') {
        // 处理成功结果
        console.log('OAuth成功:', event.data.data)
        popup.close()
    }
})
```

## 注意事项

1. OAuth会话具有时效性，建议在15分钟内完成授权
2. postMessage通信需要验证消息来源
3. 支持的OAuth服务商需要在provider_types表中正确配置
4. 刷新令牌操作是异步的，注意处理并发情况
5. 撤销授权会同时删除本地存储的令牌信息