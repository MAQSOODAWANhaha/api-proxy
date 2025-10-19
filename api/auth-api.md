# 认证 API 文档

## 概述

本文档描述了用户认证相关的 API 接口，包括用户登录、登出、Token 验证和刷新。

## 基础信息

- **基础路径**: `/api/auth`
- **认证方式**: 部分接口需要 JWT Token
- **响应格式**: JSON

---

## 1. 用户登录

**POST** `/api/auth/login`

使用用户名和密码进行登录，成功后返回 JWT Token。

#### 请求体

```json
{
  "username": "your_username",
  "password": "your_password"
}
```

| 参数名 | 类型 | 必填 | 描述 |
|---|---|---|---|
| username | string | 是 | 用户名 |
| password | string | 是 | 密码 |

#### 响应示例

```json
{
  "success": true,
  "data": {
    "token": "ey...",
    "refresh_token": "ey...",
    "user": {
      "id": 1,
      "username": "admin",
      "email": "admin@example.com",
      "is_admin": true
    }
  },
  "message": "Login successful",
  "timestamp": "2024-01-18T16:00:00Z"
}
```

---

## 2. 用户登出

**POST** `/api/auth/logout`

用户登出，需要有效的 JWT Token。

#### 权限要求
- 当前登录用户

#### 响应示例

```json
{
  "success": true,
  "data": null,
  "message": "Logout successful",
  "timestamp": "2024-01-18T16:05:00Z"
}
```

---

## 3. 验证 Token

**GET** `/api/auth/validate`

验证当前请求头中的 JWT Token 是否有效。

#### 权限要求
- 需要提供 JWT Token

#### 响应示例 (Token 有效)

```json
{
  "success": true,
  "data": {
    "valid": true,
    "user": {
      "id": 1,
      "username": "admin",
      "email": "admin@example.com",
      "is_admin": true
    }
  },
  "message": "操作成功",
  "timestamp": "2024-01-18T16:10:00Z"
}
```

#### 响应示例 (Token 无效)

```json
{
  "success": true,
  "data": {
    "valid": false,
    "user": null
  },
  "message": "操作成功",
  "timestamp": "2024-01-18T16:11:00Z"
}
```

---

## 4. 刷新 Token

**POST** `/api/auth/refresh`

使用 `refresh_token` 获取一个新的 `access_token`。

#### 请求体

```json
{
  "refresh_token": "your_refresh_token"
}
```

#### 响应示例

```json
{
  "success": true,
  "data": {
    "access_token": "ey...",
    "token_type": "Bearer",
    "expires_in": 3600
  },
  "message": "Token refreshed successfully",
  "timestamp": "2024-01-18T16:15:00Z"
}
```
