# 日志管理 API 文档

## 概述

本文档描述了代理跟踪日志管理相关的接口，基于 `proxy_tracing` 表的数据结构设计。该接口支持日志的查询、统计和详情查看等功能，主要用于监控和分析API代理请求的执行情况。

## 认证

所有接口都需要用户认证。

---

## 获取日志卡片统计数据

### 接口信息
- **请求路由**: `GET /api/logs/dashboard-stats`
- **请求方法**: GET
- **作用**: 获取日志管理页面的卡片统计数据

### 返回值
```json
{
    "success": true,
    "data": {
        "total_requests": 125420,
        "successful_requests": 123890,
        "failed_requests": 1530,
        "success_rate": 98.78,
        "total_tokens": 8547230,
        "total_cost": 2847.50,
        "avg_response_time": 1250
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 统计字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| total_requests | int | 总请求数量 |
| successful_requests | int | 成功请求数量 |
| failed_requests | int | 失败请求数量 |
| success_rate | float | 成功率（百分比） |
| total_tokens | int | 总Token消耗 |
| total_cost | float | 总费用（USD） |
| avg_response_time | int | 平均响应时间（毫秒） |

---

## 获取日志列表

### 接口信息
- **请求路由**: `GET /api/logs/traces`
- **请求方法**: GET
- **作用**: 获取代理跟踪日志列表，支持分页、搜索和过滤

### 查询参数
| 参数名 | 类型 | 必填 | 描述 | 默认值 |
|--------|------|------|------|--------|
| page | int | 否 | 页码（从1开始） | 1 |
| limit | int | 否 | 每页数量（10/20/50/100） | 20 |
| search | string | 否 | 搜索关键词（搜索请求ID、路径、模型） | - |
| method | string | 否 | 筛选HTTP方法（GET/POST/PUT/DELETE） | - |
| status_code | int | 否 | 筛选状态码 | - |
| is_success | boolean | 否 | 筛选成功状态（true/false） | - |
| model_used | string | 否 | 筛选使用的模型 | - |
| provider_type_id | int | 否 | 筛选服务商类型ID | - |
| user_service_api_id | int | 否 | 筛选用户服务API ID | - |
| user_service_api_name | string | 否 | 筛选用户服务API名称（模糊） | - |
| user_provider_key_name | string | 否 | 筛选提供商密钥名称（模糊） | - |
| start_time | string | 否 | 开始时间（ISO 8601格式） | - |
| end_time | string | 否 | 结束时间（ISO 8601格式） | - |

### 返回值
```json
{
    "success": true,
    "data": {
        "traces": [
            {
                "id": 12345,
                "request_id": "req-1234567890abcdef",
                "user_service_api_id": 1,
                "user_provider_key_id": 5,
                "method": "POST",
                "path": "/v1/chat/completions",
                "status_code": 200,
                "tokens_prompt": 150,
                "tokens_completion": 300,
                "tokens_total": 450,
                "cost": 0.025,
                "cost_currency": "USD",
                "model_used": "gpt-4",
                "client_ip": "192.168.1.100",
                "user_agent": "OpenAI/Python 1.0.0",
                "error_type": null,
                "error_message": null,
                "retry_count": 0,
                "provider_type_id": 1,
                "start_time": "2025-08-20T06:47:10.123456789Z",
                "end_time": "2025-08-20T06:47:11.234567890Z",
                "duration_ms": 1111,
                "is_success": true,
                "created_at": "2025-08-20T06:47:11.234567890Z",
                "provider_name": "OpenAI",
                "service_name": "ChatGPT Service"
            }
        ],
        "pagination": {
            "page": 1,
            "limit": 20,
            "total": 125420,
            "pages": 6271
        }
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| id | int | 跟踪记录唯一标识 |
| request_id | string | 请求唯一标识 |
| user_service_api_id | int | 用户服务API ID |
| user_provider_key_id | int | 用户提供商密钥ID（可为null） |
| method | string | HTTP请求方法 |
| path | string | 请求路径 |
| status_code | int | HTTP状态码 |
| tokens_prompt | int | 提示Token数量 |
| tokens_completion | int | 完成Token数量 |
| tokens_total | int | 总Token数量 |
| cost | float | 请求费用 |
| cost_currency | string | 费用货币单位 |
| model_used | string | 使用的模型 |
| client_ip | string | 客户端IP地址 |
| user_agent | string | 用户代理字符串 |
| error_type | string | 错误类型（可为null） |
| error_message | string | 错误消息（可为null） |
| retry_count | int | 重试次数 |
| provider_type_id | int | 服务商类型ID |
| start_time | string | 请求开始时间（ISO 8601格式） |
| end_time | string | 请求结束时间（ISO 8601格式） |
| duration_ms | int | 请求持续时间（毫秒） |
| is_success | boolean | 是否成功 |
| created_at | string | 记录创建时间（ISO 8601格式） |
| provider_name | string | 服务商名称（关联查询） |
| user_service_api_name | string | 用户服务API名称（关联查询） |
| user_provider_key_name | string | 提供商密钥名称（关联查询） |

---

## 获取日志详情

### 接口信息
- **请求路由**: `GET /api/logs/traces/{id}`
- **请求方法**: GET
- **作用**: 获取指定代理跟踪日志的详细信息

### 路径参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| id | int | 是 | 跟踪记录ID |

### 返回值
```json
{
    "success": true,
    "data": {
        "id": 12345,
        "request_id": "req-1234567890abcdef",
        "user_service_api_id": 1,
        "user_provider_key_id": 5,
        "user_id": 101,
        "method": "POST",
        "path": "/v1/chat/completions",
        "status_code": 200,
        "tokens_prompt": 150,
        "tokens_completion": 300,
        "tokens_total": 450,
        "token_efficiency_ratio": 2.0,
        "cache_create_tokens": 0,
        "cache_read_tokens": 0,
        "cost": 0.025,
        "cost_currency": "USD",
        "model_used": "gpt-4",
        "client_ip": "192.168.1.100",
        "user_agent": "OpenAI/Python 1.0.0",
        "error_type": null,
        "error_message": null,
        "retry_count": 0,
        "provider_type_id": 1,
        "start_time": "2025-08-20T06:47:10.123456789Z",
        "end_time": "2025-08-20T06:47:11.234567890Z",
        "duration_ms": 1111,
        "is_success": true,
        "created_at": "2025-08-20T06:47:11.234567890Z",
        "provider_name": "OpenAI",
        "user_service_api_name": "ChatGPT Service",
        "user_provider_key_name": "Primary GPT Key"
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 详情字段说明
除了列表接口中的字段外，详情接口还包含：
| 字段名 | 类型 | 描述 |
|--------|------|------|
| user_id | int | 用户ID |
| token_efficiency_ratio | float | Token效率比率 |
| cache_create_tokens | int | 缓存创建Token数量 |
| cache_read_tokens | int | 缓存读取Token数量 |
| provider_key_name | string | 提供商密钥名称（关联查询） |

---

## 获取日志统计分析

### 接口信息
- **请求路由**: `GET /api/logs/analytics`
- **请求方法**: GET
- **作用**: 获取日志的统计分析数据，用于图表展示

### 查询参数
| 参数名 | 类型 | 必填 | 描述 | 默认值 |
|--------|------|------|------|--------|
| time_range | string | 否 | 时间范围（1h/6h/24h/7d/30d） | 24h |
| group_by | string | 否 | 分组方式（hour/day/model/provider/status） | hour |

### 返回值
```json
{
    "success": true,
    "data": {
        "time_series": [
            {
                "timestamp": "2025-08-20T06:00:00Z",
                "total_requests": 1250,
                "successful_requests": 1220,
                "failed_requests": 30,
                "total_tokens": 45000,
                "total_cost": 12.50,
                "avg_response_time": 1200
            }
        ],
        "model_distribution": [
            {
                "model": "gpt-4",
                "request_count": 8500,
                "token_count": 450000,
                "cost": 125.50,
                "percentage": 68.0
            },
            {
                "model": "gpt-3.5-turbo",
                "request_count": 4000,
                "token_count": 180000,
                "cost": 45.20,
                "percentage": 32.0
            }
        ],
        "provider_distribution": [
            {
                "provider_name": "OpenAI",
                "request_count": 10500,
                "success_rate": 99.2,
                "avg_response_time": 1100
            },
            {
                "provider_name": "Anthropic",
                "request_count": 2000,
                "success_rate": 98.5,
                "avg_response_time": 1300
            }
        ],
        "status_distribution": [
            {
                "status_code": 200,
                "count": 12300,
                "percentage": 98.4
            },
            {
                "status_code": 400,
                "count": 150,
                "percentage": 1.2
            },
            {
                "status_code": 500,
                "count": 50,
                "percentage": 0.4
            }
        ]
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
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
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 常见错误码
- **401 Unauthorized**: 用户未认证
- **403 Forbidden**: 用户无权限访问
- **400 Bad Request**: 请求参数错误
- **404 Not Found**: 日志记录不存在
- **422 Unprocessable Entity**: 请求数据验证失败
- **500 Internal Server Error**: 服务器内部错误

## 使用场景

该接口主要用于：
1. 日志管理页面的统计数据展示
2. 代理请求的监控和分析
3. 性能指标的跟踪和优化
4. 错误日志的查询和排查
5. 费用和Token使用情况的统计
6. 服务商性能对比分析

## 注意事项

1. 该接口需要用户认证
2. 日志数据量可能很大，建议合理使用分页参数
3. 时间查询建议使用索引优化的字段
4. 统计数据可能存在缓存，更新可能有延迟
5. 详情查询会关联多个表，响应时间可能较长
6. 分析接口的数据聚合可能需要较长时间，建议异步处理
7. 大时间范围的查询可能会有性能影响，建议限制查询范围