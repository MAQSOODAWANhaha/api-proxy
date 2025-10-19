# 用户服务 API 文档

## 概述

本文档描述了用户服务API相关的接口，包括API密钥管理、使用统计、状态控制等功能。所有统计数据均从 `proxy_tracing` 表实时获取。

## 认证

所有接口都需要用户认证，会根据当前用户进行数据筛选。

---

## 1. 用户API Keys卡片展示

### 接口信息
- **请求路由**: `GET /api/user-service/cards`
- **请求方法**: GET
- **作用**: 获取用户API Keys的概览统计数据

### 筛选参数
- 自动按当前用户筛选数据

### 返回值
```json
{
    "success": true,
    "data": {
        "total_api_keys": 5,                    // 总API Key数量
        "active_api_keys": 4,                   // 活跃API Key数量
        "requests": 15420                       // 总请求数
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 2. 用户API Keys列表

### 接口信息
- **请求路由**: `GET /api/user-service/keys`
- **请求方法**: GET
- **作用**: 获取当前用户的API Keys列表，支持筛选和分页

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 | 默认值 |
|--------|------|------|------|--------|
| page | int | 否 | 页码 | 1 |
| limit | int | 否 | 每页数量 | 10 |
| name | string | 否 | 密钥名称筛选 | - |
| description | string | 否 | 描述筛选 | - |
| provider_type_id | int | 否 | 服务类型筛选 | - |
| is_active | bool | 否 | 状态筛选 | - |

### 返回值
```json
{
    "success": true,
    "data": {
        "service_api_keys": [
            {
                "id": 1,
                "name": "API Key 1",
                "description": "Description for API Key 1",
                "provider": "Claude",
                "provider_type_id": 1,
                "api_key": "sk-123-****123123",
                "usage": {
                    "successful_requests": 1200,
                    "failed_requests": 50,
                    "total_requests": 1250,
                    "success_rate": 96.0,
                    "avg_response_time": 850,
                    "total_cost": 125.50,
                    "total_tokens": 450000,
                    "last_used_at": "2023-10-01T12:34:56Z"
                },
                "is_active": true,
                "last_used_at": "2023-10-01T12:34:56Z",
                "created_at": "2023-09-15T10:20:30Z",
                "expires_at": "2025-12-31T23:59:59Z"
            }
        ],
        "pagination": {
            "page": 1,
            "limit": 10,
            "total": 5,
            "pages": 1
        }
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 3. 新增API Key

### 接口信息
- **请求路由**: `POST /api/user-service/keys`
- **请求方法**: POST
- **作用**: 创建新的用户API Key

### 请求参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| name | string | 是 | API Key名称 |
| description | string | 否 | 描述信息 |
| provider_type_id | int | 是 | 服务商类型ID |
| user_provider_keys_ids | array[int] | 是 | 关联的提供商密钥ID列表 |
| scheduling_strategy | string | 否 | 调度策略 |
| retry_count | int | 否 | 重试次数 |
| timeout_seconds | int | 否 | 超时时间(秒) |
| max_request_per_min | int | 否 | 每分钟最大请求数 |
| max_requests_per_day | int | 否 | 每日最大请求数 |
| max_tokens_per_day | i64 | 否 | 每日最大Token数 |
| max_cost_per_day | decimal | 否 | 每日最大费用 |
| expires_at | string | 否 | 过期时间(ISO 8601格式) |

### 请求体示例
```json
{
    "name": "API Key 3",
    "description": "Description for API Key 3",
    "provider_type_id": 1,
    "scheduling_strategy": "round_robin",
    "user_provider_keys_ids": [1, 2],
    "retry_count": 3,
    "timeout_seconds": 30,
    "max_request_per_min": 60,
    "max_requests_per_day": 50000,
    "max_tokens_per_day": 10000,
    "max_cost_per_day": 100.00,
    "expires_at": "2025-08-18T06:47:12.364806516Z",
    "is_active": true
}
```

### 返回值
```json
{
    "success": true,
    "data": {
        "id": 3,
        "api_key": "sk-usr-abcd1234567890abcdef",
        "name": "API Key 3",
        "description": "Description for API Key 3",
        "provider_type_id": 1,
        "is_active": true,
        "created_at": "2025-08-18T06:47:12.364806516Z"
    },
    "message": "API Key创建成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 4. 获取API Key详情

### 接口信息
- **请求路由**: `GET /api/user-service/keys/{id}`
- **请求方法**: GET
- **作用**: 获取指定API Key的详细信息

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | API Key ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": 1,
        "name": "API Key 1",
        "description": "Description for API Key 1",
        "provider_type_id": 1,
        "provider": "Claude",
        "api_key": "sk-usr-****abcdef",
        "user_provider_keys_ids": [1, 2],
        "scheduling_strategy": "round_robin",
        "retry_count": 3,
        "timeout_seconds": 30,
        "max_request_per_min": 60,
        "max_requests_per_day": 50000,
        "max_tokens_per_day": 10000,
        "max_cost_per_day": 100.00,
        "expires_at": "2025-12-31T23:59:59Z",
        "is_active": true,
        "created_at": "2023-09-15T10:20:30Z",
        "updated_at": "2023-10-01T15:30:45Z"
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 5. 编辑API Key

### 接口信息
- **请求路由**: `PUT /api/user-service/keys/{id}`
- **请求方法**: PUT
- **作用**: 更新指定API Key的配置信息

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | API Key ID |

### 请求参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| name | string | 否 | API Key名称 |
| description | string | 否 | 描述信息 |
| user_provider_keys_ids | array[int] | 否 | 关联的提供商密钥ID列表 |
| scheduling_strategy | string | 否 | 调度策略 |
| retry_count | int | 否 | 重试次数 |
| timeout_seconds | int | 否 | 超时时间(秒) |
| max_request_per_min | int | 否 | 每分钟最大请求数 |
| max_requests_per_day | int | 否 | 每日最大请求数 |
| max_tokens_per_day | int | 否 | 每日最大Token数 |
| max_cost_per_day | decimal | 否 | 每日最大费用 |
| expires_at | string | 否 | 过期时间(ISO 8601格式) |

### 请求体示例
```json
{
    "name": "Updated API Key",
    "description": "Updated description",
    "max_request_per_min": 120,
    "max_tokens_per_day": 20000
}
```

### 返回值
```json
{
    "success": true,
    "data": {
        "id": 1,
        "name": "Updated API Key",
        "description": "Updated description",
        "updated_at": "2025-08-18T06:47:12.364806516Z"
    },
    "message": "API Key更新成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 6. 删除API Key

### 接口信息
- **请求路由**: `DELETE /api/user-service/keys/{id}`
- **请求方法**: DELETE
- **作用**: 删除指定的API Key

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | API Key ID |

### 返回值
```json
{
    "success": true,
    "data": null,
    "message": "API Key删除成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 7. API Key使用统计

### 接口信息
- **请求路由**: `GET /api/user-service/keys/{id}/usage`
- **请求方法**: GET
- **作用**: 获取指定API Key的详细使用统计数据（来自proxy_tracing表）

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | API Key ID |

### 查询参数
| 参数名 | 类型 | 必填 | 描述 | 可选值 |
|--------|------|------|------|--------|
| time_range | string | 否 | 时间范围 | today, 7days, 30days |
| start_date | string | 否 | 自定义开始日期 | YYYY-MM-DD |
| end_date | string | 否 | 自定义结束日期 | YYYY-MM-DD |

### 返回值
```json
{
    "success": true,
    "data": {
        "total_requests": 1250,
        "successful_requests": 1200,
        "failed_requests": 50,
        "success_rate": 96.0,
        "total_tokens": 450000,
        "tokens_prompt": 200000,
        "tokens_completion": 250000,
        "cache_create_tokens": 15000,
        "cache_read_tokens": 8000,
        "total_cost": 125.50,
        "cost_currency": "USD",
        "avg_response_time": 850,
        "last_used": "2025-08-18T12:30:00Z",
        "usage_trend": [
            {
                "date": "2025-08-18",
                "requests": 150,
                "successful_requests": 145,
                "failed_requests": 5,
                "tokens": 50000,
                "cost": 12.50
            },
            {
                "date": "2025-08-17",
                "requests": 200,
                "successful_requests": 190,
                "failed_requests": 10,
                "tokens": 65000,
                "cost": 16.25
            }
        ]
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 8. 重新生成API Key

### 接口信息
- **请求路由**: `POST /api/user-service/keys/{id}/regenerate`
- **请求方法**: POST
- **作用**: 重新生成指定API Key的密钥值

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | API Key ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": 1,
        "api_key": "sk-usr-new1234567890abcdef",
        "regenerated_at": "2025-08-18T06:47:12.364806516Z"
    },
    "message": "API Key重新生成成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

### 注意事项
- 重新生成后，原API Key立即失效
- 所有使用旧API Key的客户端需要更新

---

## 9. 启用/禁用API Key

### 接口信息
- **请求路由**: `PUT /api/user-service/keys/{id}/status`
- **请求方法**: PUT
- **作用**: 切换指定API Key的启用/禁用状态

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | API Key ID |

### 请求参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| is_active | bool | 是 | 启用状态（true=启用，false=禁用） |

### 请求体示例
```json
{
    "is_active": false
}
```

### 返回值
```json
{
    "success": true,
    "data": {
        "id": 1,
        "is_active": false,
        "updated_at": "2025-08-18T06:47:12.364806516Z"
    },
    "message": "API Key状态更新成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

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
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

### 常见错误码
- **401 Unauthorized**: 用户未认证
- **403 Forbidden**: 用户无权限访问
- **404 Not Found**: API Key不存在
- **400 Bad Request**: 请求参数错误
- **500 Internal Server Error**: 服务器内部错误

## 数据来源说明

### 统计数据来源（proxy_tracing表）
所有使用统计数据均来自 `proxy_tracing` 表：
- **请求统计**: 根据 `is_success` 字段统计成功/失败请求
- **Token统计**: 聚合 `tokens_prompt`, `tokens_completion`, `tokens_total` 字段
- **缓存Token**: 聚合 `cache_create_tokens`, `cache_read_tokens` 字段
- **费用统计**: 聚合 `cost` 字段，按 `cost_currency` 分组
- **时间范围**: 使用 `created_at` 字段进行时间筛选
- **用户隔离**: 通过 `user_id` 和 `user_service_api_id` 确保数据安全

### 查询优化
利用现有索引提升查询性能：
- `idx_proxy_tracing_user_service_time`: 用户服务API + 时间索引
- `idx_proxy_tracing_user_time`: 用户 + 时间索引
- `idx_proxy_tracing_cost_time`: 费用 + 时间索引

## 注意事项

1. 所有接口都需要用户认证
2. 时间参数使用 ISO 8601 格式
3. 费用字段统一使用数值类型，货币单位在 `cost_currency` 字段中标识
4. API Key在响应中会进行脱敏处理（显示前4位和后4位）
5. 所有统计数据都按当前用户进行过滤
6. 删除API Key会同时清理相关的追踪数据
7. 重新生成API Key会保留历史统计数据