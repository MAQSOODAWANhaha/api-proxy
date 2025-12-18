# 用户管理 API 文档

## 概述

用户管理 API 提供完整的用户增删改查功能，包括用户注册、更新、查询、删除以及状态管理。

## 基础信息

- **基础路径**: `/api/users`
- **认证方式**: JWT Token
- **权限要求**: 部分接口需要管理员权限
- **响应格式**: JSON

## 数据模型

### 用户实体

```typescript
interface User {
  id: number;              // 用户ID (自动生成)
  username: string;        // 用户名 (唯一，最大50字符)
  email: string;           // 邮箱 (唯一，最大100字符)
  is_active: boolean;      // 是否激活 (默认: true)
  is_admin: boolean;       // 是否管理员 (默认: false)
  last_login?: string;     // 最后登录时间 (ISO 8601格式)
  created_at: string;      // 创建时间 (ISO 8601格式)
  updated_at: string;      // 更新时间 (ISO 8601格式)
  // 统计数据 (从proxy_tracing表聚合)
  total_requests: number;  // 总请求数
  total_cost: number;      // 总花费 (USD)
  total_tokens: number;    // 总token消耗
}
```

### 请求数据模型

#### 创建用户请求
```typescript
interface CreateUserRequest {
  username: string;        // 用户名 (必填，3-50字符，唯一)
  email: string;           // 邮箱 (必填，有效邮箱格式，唯一)
  password: string;        // 密码 (必填，最少8字符)
  is_admin?: boolean;      // 是否管理员 (可选，默认false)
}
```

#### 更新用户请求
```typescript
interface UpdateUserRequest {
  username?: string;       // 用户名 (可选，3-50字符，唯一)
  email?: string;          // 邮箱 (可选，有效邮箱格式，唯一)
  password?: string;       // 密码 (可选，最少8字符)
  is_active?: boolean;     // 是否激活 (可选)
  is_admin?: boolean;      // 是否管理员 (可选，需要管理员权限)
}
```

#### 用户查询参数
```typescript
interface UserQueryParams {
  page?: number;           // 页码 (默认: 1)
  limit?: number;          // 每页数量 (默认: 10, 最大: 100)
  search?: string;         // 搜索关键词 (用户名或邮箱)
  is_active?: boolean;     // 按激活状态筛选
  is_admin?: boolean;      // 按管理员状态筛选
  sort?: 'created_at' | 'updated_at' | 'username' | 'email'; // 排序字段 (默认: created_at)
  order?: 'asc' | 'desc';  // 排序方向 (默认: desc)
}
```

## API 接口

### 0. 获取用户统计

**GET** `/api/users/stats`

获取用户统计数据，用于管理端「用户管理」页面顶部统计卡片展示。

#### 响应示例
```json
{
  "success": true,
  "data": {
    "total": 25,
    "active": 20,
    "admin": 2,
    "inactive": 5
  },
  "message": "操作成功",
  "timestamp": "2025-12-18T01:12:27.555971976Z"
}
```

### 1. 获取用户列表

**GET** `/api/users`

获取分页的用户列表，支持搜索和筛选。

#### 查询参数
参考 `UserQueryParams` 数据模型

#### 响应示例
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "username": "admin",
      "email": "admin@example.com",
      "is_active": true,
      "is_admin": true,
      "last_login": "2024-01-15T10:30:00Z",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-15T10:30:00Z",
      "total_requests": 1250,
      "total_cost": 45.67,
      "total_tokens": 125000
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 10,
    "total": 25,
    "pages": 3
  },
  "message": "获取成功",
  "timestamp": "2024-01-15T12:00:00Z"
}
```

### 2. 获取单个用户

**GET** `/api/users/{id}`

根据用户ID获取用户详细信息。

#### 路径参数
- `id` (number): 用户ID

#### 响应示例
```json
{
  "success": true,
  "data": {
    "id": 1,
    "username": "admin",
    "email": "admin@example.com",
    "is_active": true,
    "is_admin": true,
    "last_login": "2024-01-15T10:30:00Z",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-15T10:30:00Z",
    "total_requests": 1250,
    "total_cost": 45.67,
    "total_tokens": 125000
  },
  "message": "获取成功",
  "timestamp": "2024-01-15T12:00:00Z"
}
```

### 3. 创建用户

**POST** `/api/users`

创建新用户账户。

#### 权限要求
- 管理员权限

#### 请求体
参考 `CreateUserRequest` 数据模型

#### 请求示例
```json
{
  "username": "newuser",
  "email": "newuser@example.com",
  "password": "securepassword123",
  "is_admin": false
}
```

#### 响应示例
```json
{
  "success": true,
  "data": {
    "id": 2,
    "username": "newuser",
    "email": "newuser@example.com",
    "is_active": true,
    "is_admin": false,
    "last_login": null,
    "created_at": "2024-01-15T12:00:00Z",
    "updated_at": "2024-01-15T12:00:00Z",
    "total_requests": 0,
    "total_cost": 0.0,
    "total_tokens": 0
  },
  "message": "用户创建成功",
  "timestamp": "2024-01-15T12:00:00Z"
}
```

### 4. 更新用户

**PUT** `/api/users/{id}`

更新指定用户的信息。

#### 权限要求
- **管理员权限**: 只有管理员可以更新用户信息。

#### 路径参数
- `id` (number): 用户ID

#### 请求体
参考 `UpdateUserRequest` 数据模型

#### 请求示例
```json
{
  "username": "updateduser",
  "email": "updated@example.com",
  "is_active": true
}
```

#### 响应示例
```json
{
  "success": true,
  "data": {
    "id": 2,
    "username": "updateduser",
    "email": "updated@example.com",
    "is_active": true,
    "is_admin": false,
    "last_login": null,
    "created_at": "2024-01-15T12:00:00Z",
    "updated_at": "2024-01-15T12:05:00Z",
    "total_requests": 25,
    "total_cost": 2.34,
    "total_tokens": 3500
  },
  "message": "用户更新成功",
  "timestamp": "2024-01-15T12:05:00Z"
}
```

### 5. 删除用户

**DELETE** `/api/users/{id}`

删除指定的用户。

#### 权限要求
- 管理员权限

#### 路径参数
- `id` (number): 用户ID

#### 响应示例
```json
{
  "success": true,
  "data": null,
  "message": "用户删除成功",
  "timestamp": "2024-01-15T12:10:00Z"
}
```

### 6. 批量删除用户

**DELETE** `/api/users`

批量删除多个用户。

#### 权限要求
- 管理员权限

#### 请求体
```json
{
  "ids": [2, 3, 4]
}
```

#### 响应示例
```json
{
  "success": true,
  "data": null,
  "message": "成功删除 3 个用户",
  "timestamp": "2024-01-15T12:15:00Z"
}
```

### 7. 用户状态切换

**PATCH** `/api/users/{id}/toggle-status`

切换用户的激活状态。

#### 权限要求
- 管理员权限

#### 路径参数
- `id` (number): 用户ID

#### 响应示例
```json
{
  "success": true,
  "data": {
    "id": 2,
    "username": "testuser",
    "email": "test@example.com",
    "is_active": false,
    "is_admin": false,
    "last_login": null,
    "created_at": "2024-01-15T12:00:00Z",
    "updated_at": "2024-01-15T12:20:00Z",
    "total_requests": 15,
    "total_cost": 1.23,
    "total_tokens": 2100
  },
  "message": "用户状态更新成功",
  "timestamp": "2024-01-15T12:20:00Z"
}
```

### 8. 重置用户密码

**PATCH** `/api/users/{id}/reset-password`

重置用户密码。

#### 权限要求
- 管理员权限

#### 路径参数
- `id` (number): 用户ID

#### 请求体
```json
{
  "new_password": "newpassword123"
}
```

#### 响应示例
```json
{
  "success": true,
  "data": null,
  "message": "密码重置成功",
  "timestamp": "2024-01-15T12:25:00Z"
}
```

---

## 用户个人资料 API

以下接口用于管理当前登录用户的个人资料。

### 9. 获取当前用户个人资料

**GET** `/api/users/profile`

获取当前登录用户的详细个人资料，包括统计信息和头像。

#### 权限要求
- 当前登录用户

#### 响应示例
```json
{
  "success": true,
  "data": {
    "name": "currentuser",
    "email": "currentuser@example.com",
    "avatar": "https://www.gravatar.com/avatar/...",
    "role": "普通用户",
    "created_at": "2024-01-10T08:00:00Z",
    "last_login": "2024-01-18T14:00:00Z",
    "total_requests": 580,
    "monthly_requests": 120
  },
  "message": "获取成功",
  "timestamp": "2024-01-18T15:00:00Z"
}
```

### 10. 更新当前用户个人资料

**PUT** `/api/users/profile`

更新当前登录用户的个人信息（例如邮箱）。

#### 权限要求
- 当前登录用户

#### 请求体
```json
{
  "email": "new-email@example.com"
}
```

#### 响应示例
```json
{
  "success": true,
  "data": {
    "name": "currentuser",
    "email": "new-email@example.com",
    "avatar": "https://www.gravatar.com/avatar/...",
    "role": "普通用户",
    "created_at": "2024-01-10T08:00:00Z",
    "last_login": "2024-01-18T14:00:00Z",
    "total_requests": 581,
    "monthly_requests": 121
  },
  "message": "Profile updated successfully",
  "timestamp": "2024-01-18T15:05:00Z"
}
```

### 11. 修改当前用户密码

**POST** `/api/users/password`

修改当前登录用户的密码。

#### 权限要求
- 当前登录用户

#### 请求体
```json
{
  "current_password": "oldpassword123",
  "new_password": "newsecurepassword456"
}
```

#### 响应示例
```json
{
  "success": true,
  "data": null,
  "message": "Password changed successfully",
  "timestamp": "2024-01-18T15:10:00Z"
}
```



## 错误响应

所有API接口在出错时都会返回统一的错误格式：

```json
{
  "success": false,
  "error": {
    "code": "USER_NOT_FOUND",
    "message": "用户不存在"
  },
  "timestamp": "2024-01-15T12:00:00Z"
}
```

### 常见错误码

| 错误码 | HTTP状态码 | 描述 |
|--------|------------|------|
| `VALIDATION_ERROR` | 400 | 请求参数验证失败 |
| `USER_NOT_FOUND` | 404 | 用户不存在 |
| `USERNAME_EXISTS` | 409 | 用户名已存在 |
| `EMAIL_EXISTS` | 409 | 邮箱已存在 |
| `UNAUTHORIZED` | 401 | 未授权 |
| `FORBIDDEN` | 403 | 权限不足 |
| `INTERNAL_ERROR` | 500 | 服务器内部错误 |

## 统计数据说明

用户列表接口返回的统计数据来源于 `proxy_tracing` 表，通过以下方式计算：

- **total_requests**: 统计该用户在 `proxy_tracing` 表中的记录总数
- **total_cost**: 统计该用户所有请求的总花费（`cost` 字段求和）
- **total_tokens**: 统计该用户所有请求的总token消耗（`tokens_total` 字段求和）

统计数据通过 `LEFT JOIN` 查询实时计算，确保数据的准确性和实时性。

## 数据验证规则

### 用户名验证
- 长度: 3-50 字符
- 格式: 字母、数字、下划线、连字符
- 唯一性: 不能重复

### 邮箱验证
- 格式: 有效的邮箱格式
- 长度: 最大100字符
- 唯一性: 不能重复

### 密码验证
- 长度: 最少8字符
- 强度: 建议包含大小写字母、数字和特殊字符

## 使用示例

### JavaScript/TypeScript 示例

```typescript
// 获取用户列表
const getUserList = async (params: UserQueryParams) => {
  const searchParams = new URLSearchParams();
  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined) {
      searchParams.append(key, String(value));
    }
  });
  
  const response = await fetch(`/api/users?${searchParams}`, {
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    }
  });
  
  return response.json();
};

// 创建用户
const createUser = async (userData: CreateUserRequest) => {
  const response = await fetch('/api/users', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(userData)
  });
  
  return response.json();
};

// 更新用户
const updateUser = async (id: number, userData: UpdateUserRequest) => {
  const response = await fetch(`/api/users/${id}`, {
    method: 'PUT',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(userData)
  });
  
  return response.json();
};

// 删除用户
const deleteUser = async (id: number) => {
  const response = await fetch(`/api/users/${id}`, {
    method: 'DELETE',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    }
  });
  
  return response.json();
};
```

## 安全考虑

1. **密码安全**: 密码使用盐值加密存储，不会明文存储
2. **权限控制**: 严格的权限验证机制
3. **输入验证**: 所有输入数据都会进行严格验证
4. **SQL注入防护**: 使用参数化查询防止SQL注入
5. **跨域安全**: 适当的CORS配置

## 版本历史

- v1.0.0 - 初始版本，提供基础的用户CRUD功能
