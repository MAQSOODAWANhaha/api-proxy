# 服务商类型 API 文档

## 概述

本文档描述了服务商类型（`provider_types`）相关接口。

当前模型按 `auth_type` 拆分为“按行粒度”：

- 同一个 `name` 可以存在多条记录（例如 `openai + api_key`、`openai + oauth`）
- `base_url`、认证配置等均以“行”为单位配置，便于不同认证方式使用不同上游地址
- 约束：`(name, auth_type)` 必须唯一

接口不再返回 `supported_auth_types`，每条记录仅返回 `auth_type`。

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
| is_active | bool | 否 | 仅返回启用/禁用的服务商类型（未传默认仅启用） | true |
| include_inactive | bool | 否 | 为 true 时返回全部服务商类型（包含禁用），忽略 is_active | false |

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
                "auth_type": "api_key",
                "base_url": "api.openai.com",
                "is_active": true,
                "supported_models": [],
                "auth_configs_json": {},
                "config_json": {},
                "token_mappings_json": {},
                "model_extraction_json": {},
                "created_at": "2024-01-01 00:00:00",
                "updated_at": "2024-01-01 00:00:00"
            },
            {
                "id": 2,
                "name": "gemini",
                "display_name": "Google Gemini",
                "auth_type": "oauth",
                "base_url": "cloudcode-pa.googleapis.com",
                "is_active": true,
                "supported_models": [],
                "auth_configs_json": {
                    "client_id": "xxx",
                    "client_secret": "yyy",
                    "redirect_uri": "https://example.com/oauth/callback",
                    "scopes": "https://www.googleapis.com/auth/generative-language",
                    "pkce_required": true,
                    "authorize": {
                        "url": "https://accounts.google.com/o/oauth2/auth",
                        "method": "GET",
                        "headers": {},
                        "query": {
                            "client_id": "{{client_id}}",
                            "redirect_uri": "{{redirect_uri}}",
                            "state": "{{session.state}}",
                            "scope": "{{scopes}}",
                            "response_type": "code",
                            "code_challenge": "{{session.code_challenge}}",
                            "code_challenge_method": "S256"
                        }
                    },
                    "exchange": {
                        "url": "https://oauth2.googleapis.com/token",
                        "method": "POST",
                        "headers": {},
                        "body": {
                            "grant_type": "authorization_code",
                            "code": "{{request.authorization_code}}",
                            "client_id": "{{client_id}}",
                            "client_secret": "{{client_secret}}",
                            "redirect_uri": "{{redirect_uri}}",
                            "code_verifier": "{{session.code_verifier}}"
                        }
                    },
                    "refresh": {
                        "url": "https://oauth2.googleapis.com/token",
                        "method": "POST",
                        "headers": {},
                        "body": {
                            "grant_type": "refresh_token",
                            "refresh_token": "{{session.refresh_token}}",
                            "client_id": "{{client_id}}",
                            "client_secret": "{{client_secret}}"
                        }
                    }
                },
                "config_json": {},
                "token_mappings_json": {},
                "model_extraction_json": {},
                "created_at": "2024-01-01 00:00:00",
                "updated_at": "2024-01-01 00:00:00"
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
| auth_type | string | 本行认证类型（`api_key` / `oauth`） |
| base_url | string | 服务商基础URL |
| is_active | bool | 是否启用 |
| supported_models | array[string] | 支持的模型列表（目前返回空数组，后续可扩展） |
| auth_configs_json | object | 本行认证配置（数据库原始字段；提交什么就存什么并回显） |
| config_json | object \| null | 本行通用配置（JSON，可编辑） |
| token_mappings_json | object \| null | 本行 token 映射配置（JSON，可编辑） |
| model_extraction_json | object \| null | 本行模型提取配置（JSON，可编辑） |
| created_at | string | 创建时间（ISO 8601格式） |
| updated_at | string | 更新时间（ISO 8601格式） |

### 支持的认证类型
| 类型 | 描述 |
|------|------|
| api_key | API密钥认证 |
| oauth | OAuth 2.0认证 |

---

## 获取单个服务商类型

### 接口信息
- **请求路由**: `GET /api/provider-types/providers/{id}`
- **请求方法**: GET
- **作用**: 按 ID 获取单条 `provider_types`

### 返回值
返回 `ProviderTypeItem`（结构同“列表接口”中的单条元素）。

---

## 创建服务商类型（管理员）

### 接口信息
- **请求路由**: `POST /api/provider-types/providers`
- **请求方法**: POST
- **作用**: 创建一条 `provider_types` 记录（按 `auth_type` 分行）

### 请求体
```json
{
  "name": "openai",
  "display_name": "OpenAI",
  "auth_type": "api_key",
  "base_url": "api.openai.com",
  "is_active": true,
  "config_json": {},
  "token_mappings_json": {},
  "model_extraction_json": {},
  "auth_configs_json": {}
}
```

### 返回值
```json
{
  "success": true,
  "data": {
    "provider_type": {
      "id": 1,
      "name": "openai",
      "display_name": "OpenAI",
      "auth_type": "api_key",
      "base_url": "api.openai.com",
      "is_active": true,
      "supported_models": [],
      "auth_configs_json": {},
      "config_json": {},
      "token_mappings_json": {},
      "model_extraction_json": {},
      "created_at": "2024-01-01 00:00:00",
      "updated_at": "2024-01-01 00:00:00"
    }
  },
  "message": "操作成功",
  "timestamp": "2025-08-20T06:47:12.364806516Z"
}
```

---

## 更新服务商类型（管理员）

### 接口信息
- **请求路由**: `PUT /api/provider-types/providers/{id}`
- **请求方法**: PUT
- **作用**: 更新一条 `provider_types` 记录

### 请求体（全部可选）
```json
{
  "display_name": "OpenAI (Updated)",
  "base_url": "api.openai.com",
  "is_active": true,
  "config_json": {},
  "token_mappings_json": {},
  "model_extraction_json": {},
  "auth_configs_json": {}
}
```

### 返回值
返回结构同“创建服务商类型”。

---

## 删除服务商类型（管理员）

### 接口信息
- **请求路由**: `DELETE /api/provider-types/providers/{id}`
- **请求方法**: DELETE
- **作用**: 删除一条 `provider_types` 记录

### 返回值
```json
{
  "success": true,
  "data": {
    "deleted": true
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
- **403 Forbidden**: 非管理员访问管理员接口
- **500 Internal Server Error**: 服务器内部错误

## 使用场景

该接口主要用于：
1. 前端下拉选择器的数据源
2. 创建API Key时选择服务商类型
3. 编辑API Key时显示当前服务商类型
4. 系统管理界面增删改查服务商配置（含 JSON 配置编辑）

## 注意事项

1. 该接口需要用户认证
2. 默认只返回启用状态的服务商类型
3. 返回的服务商类型按 ID 升序排序
4. display_name字段用于前端显示，name字段用于系统内部标识
5. 同一 `name` 下会有多条记录（按 `auth_type` 分行），管理端创建/更新时需明确 `auth_type`
