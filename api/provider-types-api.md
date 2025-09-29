# 服务商类型 API 文档

## 概述

本文档描述了服务商类型相关的接口，用于获取系统支持的AI服务商类型列表。该接口主要用于前端下拉选择器的数据源。

## 认证

所有接口都需要用户认证。

---

## 获取服务商类型列表

### 接口信息
- **请求路由**: `GET /api/provider-types/providers`
- **请求方法**: GET
- **作用**: 获取系统支持的所有AI服务商类型列表

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 | 默认值 |
|--------|------|------|------|--------|
| is_active | bool | 否 | 仅返回启用的服务商类型 | true |

### 返回值
```json
{
    "success": true,
    "data": {
        "provider_types": [
            {
                "id": 1,
                "name": "openai",
                "display_name": "OpenAI",
                "description": "OpenAI GPT系列模型",
                "is_active": true,
                "supported_models": ["gpt-4", "gpt-3.5-turbo"],
                "supported_auth_types": ["api_key"],
                "auth_configs": {
                    "api_key": {}
                },
                "created_at": "2024-01-01T00:00:00Z"
            },
            {
                "id": 2,
                "name": "gemini",
                "display_name": "Google Gemini",
                "description": "Google Gemini系列模型",
                "is_active": true,
                "supported_models": ["gemini-pro", "gemini-pro-vision"],
                "supported_auth_types": ["api_key", "oauth"],
                "auth_configs": {
                    "api_key": {},
                    "oauth": {
                        "authorize_url": "https://accounts.google.com/o/oauth2/auth",
                        "token_url": "https://oauth2.googleapis.com/token",
                        "scopes": "https://www.googleapis.com/auth/generative-language"
                    }
                },
                "created_at": "2024-01-01T00:00:00Z"
            },
            {
                "id": 3,
                "name": "claude",
                "display_name": "Anthropic Claude",
                "description": "Anthropic Claude系列模型",
                "is_active": true,
                "supported_models": ["claude-3-opus", "claude-3-sonnet"],
                "supported_auth_types": ["api_key", "oauth2"],
                "auth_configs": {
                    "api_key": {},
                    "oauth2": {
                        "authorize_url": "https://claude.ai/oauth/authorize",
                        "token_url": "https://console.anthropic.com/v1/oauth/token",
                        "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
                        "scopes": "org:create_api_key user:profile user:inference",
                        "pkce_required": true
                    }
                },
                "created_at": "2024-01-01T00:00:00Z"
            }
        ]
    },
    "message": "操作成功",
    "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

### 字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| id | int | 服务商类型ID |
| name | string | 服务商内部标识名称 |
| display_name | string | 服务商显示名称（用于前端显示） |
| description | string | 服务商描述信息 |
| is_active | bool | 是否启用 |
| supported_models | array[string] | 支持的模型列表 |
| supported_auth_types | array[string] | 支持的认证类型列表 |
| auth_configs | object | 各认证类型的配置信息 |
| created_at | string | 创建时间（ISO 8601格式） |

### 支持的认证类型
| 类型 | 描述 |
|------|------|
| api_key | API密钥认证 |
| oauth | OAuth 2.0认证 |

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
- **500 Internal Server Error**: 服务器内部错误

## 使用场景

该接口主要用于：
1. 前端下拉选择器的数据源
2. 创建API Key时选择服务商类型
3. 编辑API Key时显示当前服务商类型
4. 系统管理界面显示服务商配置

## 注意事项

1. 该接口需要用户认证
2. 默认只返回启用状态的服务商类型
3. 返回的服务商类型按创建时间排序
4. display_name字段用于前端显示，name字段用于系统内部标识