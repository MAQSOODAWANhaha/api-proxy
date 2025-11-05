# OAuth v2 认证接口文档

## 概述

本文档描述了新版OAuth 2.0认证相关的接口，用于实现Claude Max、Google等服务的认证流程。新版API以会话为中心，提供了更安全的授权码交换和令牌管理功能。

## 认证

所有接口都需要用户认证。

---

## 1. 获取支持的OAuth提供商列表

### 接口信息
- **请求路由**: `GET /api/oauth/providers`
- **请求方法**: GET
- **作用**: 获取系统当前支持进行OAuth认证的服务商列表。

### 响应格式
```json
{
    "success": true,
    "data": [
        {
            "provider_name": "google",
            "display_name": "Google"
        },
        {
            "provider_name": "claude",
            "display_name": "Anthropic Claude"
        }
    ],
    "message": "操作成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 2. 启动OAuth授权流程

### 接口信息
- **请求路由**: `POST /api/oauth/authorize`
- **请求方法**: POST
- **作用**: 为指定的服务商启动OAuth授权流程，生成授权URL和会话ID。

### 请求参数
```json
{
    "provider_name": "claude"
}
```

| 参数名 | 类型 | 必填 | 描述 |
|---|---|---|---|
| provider_name | string | 是 | 服务商名称，从 `GET /api/oauth/providers` 获取 |

### 响应格式
```json
{
    "success": true,
    "data": {
        "authorization_url": "https://claude.ai/oauth/authorize?client_id=...&state=...&code_challenge=...",
        "session_id": "uuid-session-id"
    },
    "message": "OAuth授权会话创建成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 3. 交换授权码获取令牌

### 接口信息
- **请求路由**: `POST /api/oauth/exchange`
- **请求方法**: POST
- **作用**: 在OAuth回调后，使用授权码和会话ID从后端安全地交换令牌。

### 请求参数
```json
{
    "session_id": "uuid-session-id",
    "authorization_code": "auth-code-from-provider"
}
```

| 参数名 | 类型 | 必填 | 描述 |
|---|---|---|---|
| session_id | string | 是 | `authorize` 接口返回的会话ID |
| authorization_code | string | 是 | OAuth提供商回调时返回的授权码 |

### 响应格式
```json
{
    "success": true,
    "data": {
        "session_id": "uuid-session-id",
        "status": "authorized",
        "expires_at": "2024-12-31T23:59:59Z"
    },
    "message": "OAuth授权完成",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 4. 获取用户OAuth会话列表

### 接口信息
- **请求路由**: `GET /api/oauth/sessions`
- **请求方法**: GET
- **作用**: 获取当前用户的所有OAuth会话列表。

### 响应格式
```json
{
    "success": true,
    "data": [
        {
            "session_id": "uuid-session-id-1",
            "provider_name": "claude",
            "status": "authorized",
            "created_at": "2024-01-01T00:00:00Z",
            "expires_at": "2024-12-31T23:59:59Z"
        }
    ],
    "message": "操作成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 5. 刷新令牌

### 接口信息
- **请求路由**: `POST /api/oauth/sessions/{session_id}/refresh`
- **请求方法**: POST
- **作用**: 手动触发指定会话的访问令牌刷新。

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|---|---|---|---|
| session_id | string | 是 | OAuth会话ID |

### 响应格式
```json
{
    "success": true,
    "data": {
        "session_id": "uuid-session-id",
        "status": "authorized",
        "new_expires_at": "2025-01-31T23:59:59Z"
    },
    "message": "Token刷新成功",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 6. 删除会话 (撤销授权)

### 接口信息
- **请求路由**: `DELETE /api/oauth/sessions/{session_id}`
- **请求方法**: DELETE
- **作用**: 删除一个OAuth会话，并尝试撤销其关联的令牌。

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|---|---|---|---|
| session_id | string | 是 | 要删除的OAuth会话ID |

### 响应格式
```json
{
    "success": true,
    "data": null,
    "message": "OAuth会话已删除",
    "timestamp": "2024-01-01T00:00:00Z"
}
```

---

## 7. 清理过期会话 (管理员)

### 接口信息
- **请求路由**: `POST /api/oauth/cleanup`
- **请求方法**: POST
- **作用**: 清理系统中所有过期的和孤立的OAuth会话。
- **权限**: 仅限管理员

### 响应格式
```json
{
    "success": true,
    "data": {
        "removed_expired": 10,
        "removed_orphaned": 5
    },
    "message": "清理任务完成",
    "timestamp": "2024-01-01T00:00:00Z"
}
```
