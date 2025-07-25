# AI Proxy API Reference

完整的API接口参考文档，包含curl请求示例和响应格式。

## 基础信息

- **Base URL**: `http://127.0.0.1:9090/api`
- **Content-Type**: `application/json`
- **认证**: JWT Token (暂时放开，后续实现)

## 1. 认证接口

### 1.1 用户登录
**简化版本，暂时放开认证验证**

```bash
curl -X POST http://127.0.0.1:9090/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "any_password"
  }'
```

**响应**:
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "user": {
    "id": 1,
    "username": "admin",
    "email": "admin@example.com",
    "is_admin": true
  }
}
```

## 2. 健康检查接口

### 2.1 基础健康检查

```bash
curl http://127.0.0.1:9090/api/health
```

**响应**:
```json
{
  "status": "healthy",
  "timestamp": "2025-07-24T11:30:00.000Z",
  "details": {
    "healthy_servers": 2,
    "total_servers": 3,
    "avg_response_time_ms": 150
  }
}
```

### 2.2 详细健康检查

```bash
curl http://127.0.0.1:9090/api/health/detailed
```

**响应**:
```json
{
  "status": "healthy",
  "timestamp": "2025-07-24T11:30:00.000Z",
  "system": {
    "total_servers": 3,
    "healthy_servers": 2,
    "unhealthy_servers": 1,
    "active_tasks": 5,
    "avg_response_time": "150ms",
    "is_running": true
  },
  "adapters": {},
  "load_balancers": "TODO: 添加负载均衡器状态"
}
```

## 3. 系统信息接口

### 3.1 获取系统信息

```bash
curl http://127.0.0.1:9090/api/system/info
```

**响应**:
```json
{
  "service": {
    "name": "AI Proxy",
    "version": "0.1.0",
    "build_time": "2025-07-24T10:00:00Z",
    "git_commit": "abc123"
  },
  "runtime": {
    "uptime_seconds": 3600,
    "rust_version": "1.75.0",
    "target": "x86_64"
  },
  "configuration": {
    "server_port": 8080,
    "https_port": 0,
    "workers": 4,
    "database_url": "sqlite::memory:"
  }
}
```

### 3.2 获取系统指标

```bash
curl http://127.0.0.1:9090/api/system/metrics
```

**响应**:
```json
{
  "memory": {
    "total": 8589934592,
    "used": 2147483648,
    "available": 6442450944,
    "usage_percent": 25.0
  },
  "cpu": {
    "load_average": [0.5, 0.6, 0.7],
    "cores": 8,
    "usage_percent": 0.0
  },
  "network": {
    "bytes_sent": 1048576,
    "bytes_received": 2097152,
    "connections_active": 10
  },
  "process": {
    "pid": 12345,
    "threads": 16,
    "file_descriptors": 64,
    "uptime_seconds": 3600
  },
  "timestamp": "2025-07-24T11:30:00.000Z"
}
```

## 4. 负载均衡器接口

### 4.1 获取负载均衡器状态

```bash
curl http://127.0.0.1:9090/api/loadbalancer/status
```

**响应**:
```json
{
  "status": "active",
  "algorithms": ["round_robin", "weighted", "health_based"],
  "current_algorithm": "health_based",
  "load_balancers": {
    "OpenAI": {
      "total_servers": 2,
      "healthy_servers": 2,
      "current_requests": 1500
    },
    "Claude": {
      "total_servers": 1,
      "healthy_servers": 1,
      "current_requests": 800
    }
  }
}
```

### 4.2 列出所有服务器

```bash
# 基础查询
curl http://127.0.0.1:9090/api/loadbalancer/servers

# 带过滤条件
curl "http://127.0.0.1:9090/api/loadbalancer/servers?upstream_type=OpenAI&healthy=true"
```

**查询参数**:
- `upstream_type`: 上游类型过滤 (OpenAI, Anthropic, GoogleGemini)
- `healthy`: 健康状态过滤 (true/false)

**响应**:
```json
{
  "servers": [
    {
      "id": "openai-api-openai-com-443",
      "api_id": 1,
      "upstream_type": "OpenAI",
      "display_name": "OpenAI Official",
      "host": "api.openai.com",
      "port": 443,
      "use_tls": true,
      "weight": 100,
      "is_healthy": true,
      "is_active": true,
      "response_time_ms": 120,
      "requests_total": 1500,
      "requests_successful": 1485,
      "requests_failed": 15,
      "rate_limit": 60,
      "timeout_seconds": 30,
      "created_at": "2025-07-24T10:00:00.000Z",
      "last_used": "2025-07-24T11:25:00.000Z"
    }
  ],
  "total": 1,
  "filters": {
    "upstream_type": "OpenAI",
    "healthy": true
  }
}
```

### 4.3 添加新服务器

```bash
curl -X POST http://127.0.0.1:9090/api/loadbalancer/servers \
  -H "Content-Type: application/json" \
  -d '{
    "upstream_type": "OpenAI",
    "host": "api.openai.com",
    "port": 443,
    "use_tls": true,
    "weight": 100,
    "max_connections": 1000,
    "timeout_ms": 5000
  }'
```

**请求参数**:
- `upstream_type`: 上游类型 (必需)
- `host`: 主机地址 (必需)
- `port`: 端口号 (必需)
- `use_tls`: 是否使用TLS (可选，默认false)
- `weight`: 权重 (可选，默认100)
- `max_connections`: 最大连接数 (可选)
- `timeout_ms`: 超时时间毫秒 (可选，默认5000)

**响应**:
```json
{
  "id": "openai-api-openai-com-443",
  "success": true,
  "message": "Server added successfully"
}
```

### 4.4 更改调度策略

```bash
curl -X PATCH http://127.0.0.1:9090/api/loadbalancer/strategy \
  -H "Content-Type: application/json" \
  -d '{
    "upstream_type": "openai",
    "strategy": "weighted"
  }'
```

**请求参数**:
- `upstream_type`: 上游类型 (必需)
- `strategy`: 调度策略 (必需): "round_robin", "weighted", "health_based"

**响应**:
```json
{
  "success": true,
  "message": "Strategy changed successfully for openai",
  "old_strategy": "RoundRobin",
  "new_strategy": "Weighted"
}
```

### 4.5 服务器操作（启用/禁用/移除）

```bash
curl -X POST http://127.0.0.1:9090/api/loadbalancer/servers/action \
  -H "Content-Type: application/json" \
  -d '{
    "server_id": "openai-1",
    "action": "disable"
  }'
```

**请求参数**:
- `server_id`: 服务器ID (必需)
- `action`: 操作类型 (必需): "enable", "disable", "remove"

**响应**:
```json
{
  "success": true,
  "message": "Server disabled successfully",
  "server_id": "openai-1",
  "action": "disable"
}
```

### 4.6 获取负载均衡器详细指标

```bash
curl http://127.0.0.1:9090/api/loadbalancer/metrics
```

**响应**:
```json
{
  "metrics": {
    "OpenAI": {
      "total_servers": 2,
      "healthy_servers": 2,
      "unhealthy_servers": 0,
      "success_rate": 100.0,
      "servers": [
        {
          "address": "api.openai.com:443",
          "weight": 100,
          "is_healthy": true,
          "success_requests": 1485,
          "failed_requests": 15,
          "avg_response_time_ms": 120.5,
          "last_health_check": "2025-07-24T11:30:00.000Z",
          "use_tls": true
        }
      ]
    },
    "Anthropic": {
      "total_servers": 1,
      "healthy_servers": 1,
      "unhealthy_servers": 0,
      "success_rate": 100.0,
      "servers": [
        {
          "address": "api.anthropic.com:443",
          "weight": 100,
          "is_healthy": true,
          "success_requests": 800,
          "failed_requests": 5,
          "avg_response_time_ms": 180.2,
          "last_health_check": "2025-07-24T11:30:00.000Z",
          "use_tls": true
        }
      ]
    }
  },
  "timestamp": "2025-07-24T11:30:00.000Z"
}
```

## 5. 适配器接口

### 5.1 列出所有适配器

```bash
curl http://127.0.0.1:9090/api/adapters
```

**响应**:
```json
{
  "adapters": [
    {
      "id": 1,
      "name": "OpenAI",
      "display_name": "OpenAI Official",
      "upstream_type": "openai_chat",
      "base_url": "https://api.openai.com",
      "default_model": "gpt-4",
      "max_tokens": 4096,
      "rate_limit": 60,
      "timeout_seconds": 30,
      "health_check_path": "/v1/models",
      "auth_header_format": "Bearer {token}",
      "status": "active",
      "supported_endpoints": 3,
      "endpoints": ["/v1/chat/completions", "/v1/models", "/v1/embeddings"],
      "version": "1.0.0",
      "created_at": "2025-07-24T10:00:00.000Z",
      "updated_at": "2025-07-24T10:00:00.000Z"
    }
  ],
  "total": 1,
  "timestamp": "2025-07-24T11:30:00.000Z"
}
```

### 5.2 获取适配器统计信息

```bash
curl http://127.0.0.1:9090/api/adapters/stats
```

**响应**:
```json
{
  "summary": {
    "total_adapters": 3,
    "total_endpoints": 9,
    "adapter_types": 3,
    "total_active_configs": 5
  },
  "by_type": {
    "openai_chat": {
      "adapters": 1,
      "endpoints": 3,
      "active_configs": 2,
      "names": ["OpenAI"]
    },
    "anthropic_chat": {
      "adapters": 1,
      "endpoints": 2,
      "active_configs": 1,
      "names": ["Claude"]
    }
  },
  "detailed_stats": {
    "OpenAI": {
      "id": 1,
      "display_name": "OpenAI Official",
      "api_format": "openai_chat",
      "base_url": "https://api.openai.com",
      "supported_endpoints": 3,
      "active_configurations": 2,
      "runtime_info": {
        "upstream_type": "OpenAI",
        "endpoints": ["/v1/chat/completions", "/v1/models"]
      },
      "health_status": {
        "status": "healthy",
        "last_check": "2025-07-24T11:25:00.000Z",
        "response_time_ms": 120,
        "success_rate": 99.0,
        "healthy_servers": 1,
        "total_servers": 1,
        "is_healthy": true
      },
      "rate_limit": 60,
      "timeout_seconds": 30,
      "last_updated": "2025-07-24T10:00:00.000Z"
    }
  },
  "timestamp": "2025-07-24T11:30:00.000Z"
}
```

## 6. 统计接口

### 6.1 获取统计概览

```bash
# 基础查询
curl http://127.0.0.1:9090/api/statistics/overview

# 带参数查询
curl "http://127.0.0.1:9090/api/statistics/overview?hours=24&upstream_type=OpenAI"
```

**查询参数**:
- `hours`: 时间范围(小时) (可选，默认24)
- `group_by`: 分组方式 (可选，hour/day)
- `upstream_type`: 上游类型过滤 (可选)

**响应**:
```json
{
  "time_range": {
    "hours": 24,
    "start_time": "2025-07-23T11:30:00.000Z",
    "end_time": "2025-07-24T11:30:00.000Z"
  },
  "requests": {
    "total": 15000,
    "successful": 14850,
    "failed": 150,
    "success_rate": 99.0
  },
  "response_times": {
    "avg_ms": 145,
    "p50_ms": 120,
    "p95_ms": 280,
    "p99_ms": 450
  },
  "traffic": {
    "requests_per_second": 0.625,
    "bytes_sent": 52428800,
    "bytes_received": 104857600
  },
  "by_provider": {
    "OpenAI": {
      "requests": 10000,
      "success_rate": 99.2,
      "avg_response_ms": 120
    },
    "Claude": {
      "requests": 5000,
      "success_rate": 98.5,
      "avg_response_ms": 180
    }
  },
  "top_endpoints": [
    {
      "path": "/v1/chat/completions",
      "requests": 12000,
      "percentage": 80.0
    },
    {
      "path": "/v1/models",
      "requests": 2000,
      "percentage": 13.3
    }
  ]
}
```

### 6.2 获取请求统计

```bash
# 基础查询
curl http://127.0.0.1:9090/api/statistics/requests

# 带参数查询  
curl "http://127.0.0.1:9090/api/statistics/requests?hours=168&group_by=day"
```

**查询参数**:
- `hours`: 时间范围(小时) (可选，默认24)
- `group_by`: 分组方式 (可选，hour/day，默认hour)
- `upstream_type`: 上游类型过滤 (可选)

**响应**:
```json
{
  "time_range": {
    "hours": 24,
    "group_by": "hour",
    "start_time": "2025-07-23T11:30:00.000Z",
    "end_time": "2025-07-24T11:30:00.000Z",
    "points": 24
  },
  "data": [
    {
      "timestamp": "2025-07-23T11:00:00.000Z",
      "requests": 500,
      "successful": 495,
      "failed": 5,
      "avg_response_ms": 145,
      "success_rate": 99.0
    },
    {
      "timestamp": "2025-07-23T12:00:00.000Z",
      "requests": 650,
      "successful": 645,
      "failed": 5,
      "avg_response_ms": 138,
      "success_rate": 99.2
    }
  ],
  "aggregated": {
    "total_requests": 15000,
    "total_successful": 14850,
    "total_failed": 150,
    "avg_response_ms": 145
  }
}
```

## 7. 用户管理接口

### 7.1 列出用户

```bash
# 基础查询
curl http://127.0.0.1:9090/api/users

# 带分页和过滤
curl "http://127.0.0.1:9090/api/users?page=1&limit=10&status=active"
```

**查询参数**:
- `page`: 页码 (可选，默认1)
- `limit`: 每页大小 (可选，默认20)
- `status`: 状态过滤 (可选，active/inactive)

**响应**:
```json
{
  "users": [
    {
      "id": 1,
      "username": "admin",
      "email": "admin@example.com",
      "role": "admin",
      "status": "active",
      "created_at": "2025-07-24T10:00:00.000Z",
      "last_login": "2025-07-24T11:00:00.000Z"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 10,
    "total": 1,
    "pages": 1
  }
}
```

### 7.2 创建用户

```bash
curl -X POST http://127.0.0.1:9090/api/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "newuser",
    "email": "newuser@example.com",
    "password": "password123",
    "role": "user"
  }'
```

**请求参数**:
- `username`: 用户名 (必需)
- `email`: 邮箱 (必需)
- `password`: 密码 (必需，最少6位)
- `role`: 角色 (可选，user/admin，默认user)

**响应**:
```json
{
  "success": true,
  "user": {
    "id": 2,
    "username": "newuser",
    "email": "newuser@example.com",
    "role": "user",
    "status": "active",
    "created_at": "2025-07-24T11:30:00.000Z",
    "last_login": null
  },
  "message": "User created successfully"
}
```

### 7.3 获取单个用户

```bash
curl http://127.0.0.1:9090/api/users/1
```

**响应**:
```json
{
  "id": 1,
  "username": "admin",
  "email": "admin@example.com",
  "role": "admin",
  "status": "active",
  "created_at": "2025-07-24T10:00:00.000Z",
  "last_login": "2025-07-24T11:00:00.000Z"
}
```

## 8. API密钥管理接口

### 8.1 列出API密钥

```bash
# 基础查询
curl http://127.0.0.1:9090/api/api-keys

# 带过滤条件
curl "http://127.0.0.1:9090/api/api-keys?page=1&limit=10&user_id=1&status=active"
```

**查询参数**:
- `page`: 页码 (可选，默认1)
- `limit`: 每页大小 (可选，默认20)
- `user_id`: 用户ID过滤 (可选)
- `status`: 状态过滤 (可选，active/inactive)

**响应**:
```json
{
  "api_keys": [
    {
      "id": 1,
      "name": "OpenAI API Key",
      "key_prefix": "sk-proj-abc123...",
      "user_id": 1,
      "description": "My OpenAI API key for testing",
      "status": "active",
      "scopes": ["api:access", "api:high_rate", "tokens:professional"],
      "usage_count": 1500,
      "created_at": "2025-07-24T10:00:00.000Z",
      "expires_at": "2025-08-24T10:00:00.000Z",
      "last_used_at": "2025-07-24T11:25:00.000Z"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 10,
    "total": 1,
    "pages": 1
  }
}
```

### 8.2 创建API密钥

```bash
curl -X POST http://127.0.0.1:9090/api/api-keys \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": 1,
    "name": "My New API Key",
    "description": "API key for production use",
    "expires_in_days": 30,
    "scopes": ["chat:read", "chat:write"]
  }'
```

**请求参数**:
- `user_id`: 用户ID (必需)
- `name`: 密钥名称 (必需)
- `description`: 描述 (可选)
- `expires_in_days`: 过期天数 (可选)
- `scopes`: 权限范围 (可选)

**响应**:
```json
{
  "success": true,
  "api_key": {
    "id": 2,
    "name": "My New API Key",
    "key": "sk-proj-xyz789abcdef123456789...",
    "key_prefix": "sk-proj-xyz789...",
    "user_id": 1,
    "description": "API key for production use",
    "status": "active",
    "scopes": ["chat:read", "chat:write"],
    "usage_count": 0,
    "created_at": "2025-07-24T11:30:00.000Z",
    "expires_at": "2025-08-24T11:30:00.000Z",
    "last_used_at": null
  },
  "message": "API key created successfully. Please save it now as it won't be shown again."
}
```

### 8.3 获取单个API密钥

```bash
curl http://127.0.0.1:9090/api/api-keys/1
```

**响应**:
```json
{
  "id": 1,
  "name": "OpenAI API Key",
  "key_prefix": "sk-proj-abc123...",
  "user_id": 1,
  "description": "My OpenAI API key for testing",
  "status": "active",
  "scopes": ["api:access", "api:high_rate", "tokens:professional"],
  "usage_count": 1500,
  "created_at": "2025-07-24T10:00:00.000Z",
  "expires_at": "2025-08-24T10:00:00.000Z",
  "last_used_at": "2025-07-24T11:25:00.000Z"
}
```

### 8.4 撤销API密钥

```bash
curl -X POST http://127.0.0.1:9090/api/api-keys/1/revoke
```

**响应**:
```json
{
  "success": true,
  "message": "API key 1 has been revoked",
  "revoked_at": "2025-07-24T11:30:00.000Z"
}
```

## 9. 通用响应格式

### 成功响应
所有成功的API调用都会返回HTTP状态码200，并包含相应的JSON数据。

### 错误响应

**400 Bad Request**:
```json
{
  "error": "Bad Request",
  "message": "Invalid request parameters"
}
```

**401 Unauthorized**:
```json
{
  "error": "Unauthorized",
  "message": "Authentication required"
}
```

**404 Not Found**:
```json
{
  "error": "Not Found",
  "message": "Resource not found"
}
```

**500 Internal Server Error**:
```json
{
  "error": "Internal Server Error",
  "message": "An internal error occurred"
}
```

## 10. 认证说明

当前版本暂时放开了认证验证，任何用户名和密码都可以登录成功。后续版本将实现完整的JWT认证机制。

对于需要认证的接口，请在请求头中包含JWT token：
```bash
curl -H "Authorization: Bearer <token>" http://127.0.0.1:9090/api/endpoint
```

## 11. 限制说明

- 当前使用内存数据库，服务重启后数据会丢失
- 部分接口可能返回模拟数据
- 健康检查和监控功能可能需要实际的上游服务器配置才能返回真实数据