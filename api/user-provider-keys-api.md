# 用户提供商密钥 API 文档

## 概述

本文档描述了用户提供商密钥相关的接口，用于获取当前用户配置的AI服务商密钥列表。该接口主要用于创建和编辑API Key时选择关联的提供商密钥。

## 认证

所有接口都需要用户认证，会根据当前用户进行数据筛选。

---

## 获取用户提供商密钥列表

### 接口信息
- **请求路由**: `GET /api/provider-keys/keys`
- **请求方法**: GET
- **作用**: 获取当前用户配置的AI服务商密钥列表

### 筛选参数
| 参数名 | 类型 | 必填 | 描述 | 默认值 |
|--------|------|------|------|--------|
| provider_type_id | int | 否 | 筛选指定服务商类型的密钥 | - |
| is_active | bool | 否 | 仅返回启用状态的密钥 | true |

### 返回值
```json
{
    "success": true,
    "data": {
        "user_provider_keys": [
            {
                "id": 1,
                "name": "openai-primary",
                "display_name": "OpenAI 主密钥"
            },
            {
                "id": 2,
                "name": "openai-backup",
                "display_name": "OpenAI 备用密钥"
            },
            {
                "id": 3,
                "name": "claude-primary",
                "display_name": "Claude 主密钥"
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
| id | int | 提供商密钥ID（用于选择） |
| name | string | 密钥内部标识名称 |
| display_name | string | 密钥显示名称（用于前端下拉框显示） |

---

## 按服务商类型获取密钥

### 接口信息
- **请求路由**: `GET /api/provider-keys/keys?provider_type_id={id}`
- **请求方法**: GET
- **作用**: 获取指定服务商类型的用户密钥列表

### 查询参数示例
```
GET /api/provider-keys/keys?provider_type_id=1&is_active=true
```

### 返回值
与上述格式相同，但只包含指定服务商类型的密钥。

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
- **500 Internal Server Error**: 服务器内部错误

## 使用场景

该接口主要用于：
1. 创建API Key时选择关联的提供商密钥（下拉选择）
2. 编辑API Key时显示和修改关联的提供商密钥（多选框）
3. 快速获取可用的提供商密钥列表

## 注意事项

1. 该接口需要用户认证，只返回当前用户的密钥
2. 专为下拉选择设计，返回简化的数据结构
3. 默认只返回启用状态的密钥
4. 支持按服务商类型筛选密钥
5. 不包含分页，直接返回所有符合条件的密钥
6. 返回的字段仅包含选择所需的基本信息