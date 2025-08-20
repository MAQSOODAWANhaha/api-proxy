# 仪表盘统计 API 文档

## 概述

本文档描述了仪表盘统计相关的 API 接口，包括数据卡片展示、模型使用统计、Token 使用趋势等功能。

## 认证

所有接口都需要用户认证，会根据当前用户进行数据筛选。

---

## 1. 仪表盘数据卡片展示

### 接口信息
- **请求路由**: `GET /api/statistics/today/cards`
- **请求方法**: GET
- **作用**: 获取今天的统计数据和与昨天数据的增长率对比

### 筛选参数
- 自动按当前用户筛选数据
- 固定展示今天的数据

### 返回值
```json
{
    "success": true,
    "data": {
        "requests_today": 0,                    // 今天请求数
        "rate_requests_today": "+10%",          // 今天请求数增长率
        "successes_today": 0.0,                 // 今天成功数
        "rate_successes_today": 0,              // 今天成功率增长率
        "tokens_today": 20123123,               // 今天 Token 使用量
        "rate_tokens_today": 0,                 // 今天 Token 使用量增长率
        "avg_response_time_today": 0,           // 今天平均响应时间
        "rate_avg_response_time_today": 0       // 今天平均响应时间增长率
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 2. 模型使用请求数占比

### 接口信息
- **请求路由**: `GET /api/statistics/models/rate`
- **请求方法**: GET
- **作用**: 获取模型使用请求数占比统计

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 | 可选值 |
|--------|------|------|------|--------|
| user_id | int | 是 | 当前用户ID | 自动获取 |
| time_range | string | 否 | 时间范围 | today, 7days, 30days |
| start | string | 否 | 自定义开始时间 | ISO 8601 格式 |
| end | string | 否 | 自定义结束时间 | ISO 8601 格式 |

### 特殊规则
- 最多展示 6 个模型
- 如果超过 6 个模型，第 6 个显示为"其他"，包含剩余所有模型的总和

### 返回值
```json
{
    "success": true,
    "data": {
        "model_usage": [
            {
                "model": "gpt-3.5-turbo",
                "usage": 1234567
            },
            {
                "model": "gpt-4",
                "usage": 1234567
            },
            {
                "model": "gemini-2.5-pro",
                "usage": 1234567
            },
            {
                "model": "gemini-2.5-flash",
                "usage": 1234567
            },
            {
                "model": "claude-3",
                "usage": 1234567
            },
            {
                "model": "其他",
                "usage": 1234567
            }
        ]
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 3. 模型使用详细统计

### 接口信息
- **请求路由**: `GET /api/statistics/models/statistics`
- **请求方法**: GET
- **作用**: 获取所有模型的详细使用统计数据

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 | 可选值 |
|--------|------|------|------|--------|
| user_id | int | 是 | 当前用户ID | 自动获取 |
| time_range | string | 否 | 时间范围 | today, 7days, 30days |
| start | string | 否 | 自定义开始时间 | ISO 8601 格式 |
| end | string | 否 | 自定义结束时间 | ISO 8601 格式 |

### 返回值
```json
{
    "success": true,
    "data": {
        "model_usage": [
            {
                "model": "gpt-3.5-turbo",
                "usage": 1234567,               // 使用次数
                "percentage": 50.0,             // 使用占比
                "cost": "100.0$"                // 费用
            },
            {
                "model": "gpt-4",
                "usage": 1234567,
                "percentage": 30.0,
                "cost": "200.0$"
            }
            // ... 更多模型数据
        ]
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 4. Token 使用趋势

### 接口信息
- **请求路由**: `GET /api/statistics/tokens/trend`
- **请求方法**: GET
- **作用**: 获取最近 30 天的 Token 使用趋势数据

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| user_id | int | 是 | 当前用户ID（自动获取） |

### 返回值
```json
{
    "success": true,
    "data": {
        "token_usage": [
            {
                "timestamp": "2025-08-18T06:47:12.364806516Z",
                "cache_create_tokens": 123456,     // 缓存创建 Token
                "cache_read_tokens": 654321,       // 缓存读取 Token
                "tokens_prompt": 777777,           // 提示 Token
                "tokens_completion": 888888,       // 完成 Token
                "cost": "$23.0"                    // 当日费用
            }
            // ... 最近 30 天的数据
        ],
        "current_token_usage": 123,                // 今天使用 Token 总数
        "average_token_usage": 456,                // 平均每天使用量
        "max_token_usage": 789                     // 最大单日使用量
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 5. 用户 API Keys 请求趋势

### 接口信息
- **请求路由**: `GET /api/statistics/user-service-api-keys/request`
- **请求方法**: GET
- **作用**: 获取用户 API Keys 最近 30 天的请求次数趋势

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| user_id | int | 是 | 当前用户ID（自动获取） |

### 返回值
```json
{
    "success": true,
    "data": {
        "request_usage": [
            {
                "timestamp": "2025-08-18T06:47:12.364806516Z",
                "request": 123456                  // 当日请求数
            },
            {
                "timestamp": "2025-08-17T06:47:12.364806516Z",
                "request": 654321
            }
            // ... 最近 30 天的数据
        ],
        "current_request_usage": 123,              // 今天请求总数
        "average_request_usage": 456,              // 平均每天请求数
        "max_request_usage": 789                   // 最大单日请求数
    },
    "message": "操作成功",
    "timestamp": "2025-08-18T06:47:12.364806516Z"
}
```

---

## 6. 用户 API Keys Token 使用趋势

### 接口信息
- **请求路由**: `GET /api/statistics/user-service-api-keys/token`
- **请求方法**: GET
- **作用**: 获取用户 API Keys 最近 30 天的 Token 使用量趋势

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 |
|--------|------|------|------|
| user_id | int | 是 | 当前用户ID（自动获取） |

### 返回值
```json
{
    "success": true,
    "data": {
        "token_usage": [
            {
                "timestamp": "2025-08-18T06:47:12.364806516Z",
                "total_token": 123456              // 当日 Token 使用总量
            },
            {
                "timestamp": "2025-08-17T06:47:12.364806516Z",
                "total_token": 654321
            }
            // ... 最近 30 天的数据
        ],
        "current_token_usage": 123,                // 今天 Token 数量总和
        "average_token_usage": 456,                // 平均每天 Token 使用量
        "max_token_usage": 789                     // 最大单日 Token 使用量
    },
    "message": "操作成功",
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

## 注意事项

1. 所有接口都需要用户认证
2. 时间参数使用 ISO 8601 格式
3. 费用字段统一使用美元符号($)
4. 增长率以百分比形式显示，包含正负号
5. 所有统计数据都按当前用户进行过滤