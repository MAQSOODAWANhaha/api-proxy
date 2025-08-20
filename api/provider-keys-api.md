# 提供商密钥管理 API 文档

## 概述

本文档描述了提供商密钥管理相关的接口，用于管理上游AI服务商的API密钥。该接口支持完整的增删改查和统计功能，包括密钥的创建、编辑、删除、查看统计信息和健康检查等。

## 认证

所有接口都需要用户认证。

---

## 获取提供商密钥列表

### 接口信息
- **请求路由**: `GET /api/provider-keys/keys`
- **请求方法**: GET
- **作用**: 获取提供商密钥列表，支持分页、搜索和过滤

### 查询参数
| 参数名 | 类型 | 必填 | 描述 | 默认值 |
|--------|------|------|------|--------|
| page | int | 否 | 页码（从1开始） | 1 |
| limit | int | 否 | 每页数量（10/20/50/100） | 10 |
| search | string | 否 | 搜索关键词（搜索账号、密钥名称） | - |
| provider | string | 否 | 筛选指定服务商 | - |
| status | string | 否 | 筛选状态（active/disabled/error） | - |

### 返回值
```json
{
    "success": true,
    "data": {
        "provider_keys": [
            {
                "id": "1",
                "provider": "OpenAI",
                "name": "Primary GPT Key",
                "api_key": "sk-1234567890abcdef1234567890abcdef",
                "weight": 1,
                "max_requests_per_minute": 60,
                "max_tokens_prompt_per_minute": 1000,
                "max_requests_per_day": 10000,
                "is_active": true,
                "usage": 8520,
                "cost": 125.50,
                "created_at": "2024-01-10T00:00:00Z",
                "health_status": "healthy"
            }
        ],
        "pagination": {
            "page": 1,
            "limit": 10,
            "total": 25,
            "pages": 3
        }
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| id | int | 密钥唯一标识 |
| user_id | int | 用户ID |
| provider_type_id | int | 服务商类型ID |
| provider | string | 服务商名称（通过provider_type_id关联查询） |
| name | string | 密钥名称 |
| api_key | string | API密钥值 |
| weight | int | 权重 |
| max_requests_per_minute | int | 请求限制/分钟（RPM） |
| max_tokens_prompt_per_minute | int | Token限制/分钟（TPM） |
| max_requests_per_day | int | 请求限制/天（RPD） |
| is_active | boolean | 是否启用 |
| health_status | string | 健康状态（healthy/warning/error） |
| usage | int | 使用次数（统计字段） |
| cost | float | 本月花费（统计字段） |
| created_at | string | 创建时间（ISO 8601格式） |
| updated_at | string | 更新时间（ISO 8601格式） |

---

## 创建提供商密钥

### 接口信息
- **请求路由**: `POST /api/provider-keys/keys`
- **请求方法**: POST
- **作用**: 创建新的提供商密钥

### 请求体
```json
{
    "provider_type_id": 1,
    "name": "Primary GPT Key",
    "api_key": "sk-1234567890abcdef1234567890abcdef",
    "weight": 1,
    "max_requests_per_minute": 60,
    "max_tokens_prompt_per_minute": 1000,
    "max_requests_per_day": 10000,
    "is_active": true
}
```

### 请求字段说明
| 字段名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| provider_type_id | int | 是 | 服务商类型id |
| name | string | 是 | 密钥名称 |
| api_key | string | 是 | API密钥值 |
| weight | int | 否 | 权重，默认1 |
| max_requests_per_minute | int | 否 | 请求限制/分钟，默认0 |
| max_tokens_prompt_per_minute | int | 否 | Token限制/分钟，默认0 |
| max_requests_per_day | int | 否 | 请求限制/天，默认0 |
| is_active | boolean | 否 | 状态，默认true |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": "12345",
        "provider": "OpenAI",
        "name": "Primary GPT Key",
        "created_at": "2025-08-20T06:47:12.364806516Z"
    },
    "message": "创建成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

---

## 获取提供商密钥详情

### 接口信息
- **请求路由**: `GET /api/provider-keys/keys/{id}`
- **请求方法**: GET
- **作用**: 获取指定提供商密钥的详细信息

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | string | 是 | 密钥ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": "1",
        "provider": "OpenAI",
        "name": "Primary GPT Key",
        "api_key": "sk-1234567890abcdef1234567890abcdef",
        "weight": 1,
        "max_requests_per_minute": 60,
        "max_tokens_prompt_per_minute": 1000,
        "max_requests_per_day": 10000,
        "is_active": true,
        "usage": 8520,
        "cost": 125.50,
        "created_at": "2024-01-10T00:00:00Z",
        "updated_at": "2024-01-16T15:20:00Z",
        "health_status": "healthy"
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

---

## 更新提供商密钥

### 接口信息
- **请求路由**: `PUT /api/provider-keys/keys/{id}`
- **请求方法**: PUT
- **作用**: 更新指定的提供商密钥信息

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | string | 是 | 密钥ID |

### 请求体
```json
{
    "provider_type_id": 1,
    "name": "Updated GPT Key",
    "api_key": "sk-new1234567890abcdef1234567890abcdef",
    "weight": 2,
    "max_requests_per_minute": 80,
    "max_tokens_prompt_per_minute": 1200,
    "max_requests_per_day": 12000,
    "is_active": true
}
```

### 返回值
```json
{
    "success": true,
    "data": {
        "id": "1",
        "name": "Updated GPT Key",
        "updated_at": "2025-08-20T06:47:12.364806516Z"
    },
    "message": "更新成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

---

## 删除提供商密钥

### 接口信息
- **请求路由**: `DELETE /api/provider-keys/keys/{id}`
- **请求方法**: DELETE
- **作用**: 删除指定的提供商密钥

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | string | 是 | 密钥ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": "1",
        "deleted_at": "2025-08-20T06:47:12.364806516Z"
    },
    "message": "删除成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

---

## 获取密钥统计信息

### 接口信息
- **请求路由**: `GET /api/provider-keys/keys/{id}/stats`
- **请求方法**: GET
- **作用**: 获取指定提供商密钥的统计信息和使用数据

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | string | 是 | 密钥ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "basic_info": {
            "provider": "OpenAI",
            "name": "Primary GPT Key",
            "weight": 1
        },
        "usage_stats": {
            "total_usage": 8520,
            "monthly_cost": 125.50,
            "success_rate": 99.2,
            "avg_response_time": 850
        },
        "daily_trends": {
            "usage": [320, 450, 289, 645, 378, 534, 489],
            "cost": [12.5, 18.2, 11.3, 25.8, 15.1, 21.4, 19.6]
        },
        "limits": {
            "max_requests_per_minute": 60,
            "max_tokens_prompt_per_minute": 1000,
            "max_requests_per_day": 10000
        }
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 统计字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| basic_info | object | 基本信息 |
| usage_stats | object | 使用统计 |
| daily_trends | object | 每日趋势数据（7天） |
| limits | object | 限制配置 |

---

## 执行健康检查

### 接口信息
- **请求路由**: `POST /api/provider-keys/keys/{id}/health-check`
- **请求方法**: POST
- **作用**: 对指定的提供商密钥执行健康检查

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | string | 是 | 密钥ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": "1",
        "health_status": "healthy",
        "check_time": "2025-08-20T06:47:12.364806516Z",
        "response_time": 245,
        "details": {
            "status_code": 200,
            "latency": 245,
            "error_message": null
        }
    },
    "message": "健康检查完成",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 健康检查字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| health_status | string | 健康状态（healthy/warning/error） |
| check_time | string | 检查时间 |
| response_time | int | 响应时间（毫秒） |
| details | object | 详细检查结果 |

---

## 通用响应格式

所有接口都遵循统一的响应格式：

```json
{
    "success": boolean,        // 操作是否成功
    "data": object,           // 具体数据内容
    "message": string,        // 操作结果消息
    "timestamp": string       // 响应时间戳（ISO 8601 格式）
}
```

## 错误处理

当请求失败时，响应格式如下：

```json
{
    "success": false,
    "data": null,
    "message": "错误描述信息",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 常见错误码
- **401 Unauthorized**: 用户未认证
- **403 Forbidden**: 用户无权限访问
- **400 Bad Request**: 请求参数错误
- **404 Not Found**: 密钥不存在
- **409 Conflict**: 密钥名称已存在
- **422 Unprocessable Entity**: 请求数据验证失败
- **500 Internal Server Error**: 服务器内部错误

## 使用场景

该接口主要用于：
1. 提供商密钥管理页面的增删改查操作
2. 密钥列表展示、搜索和过滤
3. 密钥的健康状态监控
4. 使用统计和费用分析
5. 密钥配置和限制管理

## 注意事项

1. 该接口需要用户认证
2. API密钥值在传输和存储时需要加密处理
3. 删除操作不可逆，需要确认后执行
4. 健康检查可能需要较长时间，建议异步处理
5. 统计数据可能存在缓存，更新可能有延迟
6. 密钥的权重用于负载均衡，数值越大优先级越高
7. 限制配置（RPM/TPM/RPD）用于流量控制，0表示无限制