# 公开统计 API 文档

## 概述

本文档描述了系统公开的统计信息接口，无需认证即可访问，用于展示系统的总体运营情况。

## 基础信息

- **基础路径**: `/api/stats`
- **认证方式**: 无

---

## 1. 获取统计概览

**GET** `/api/stats/overview`

获取系统运营的核心统计数据概览。

#### 响应示例

```json
{
  "success": true,
  "data": {
    "total_requests": 1234567,
    "total_tokens": 987654321,
    "total_cost_usd": 12345.67,
    "active_users": 150
  },
  "message": "操作成功",
  "timestamp": "2025-08-22T10:00:00Z"
}
```

---

## 2. 获取使用趋势

**GET** `/api/stats/trend`

获取系统最近30天的每日使用趋势（请求数、Token数）。

#### 响应示例

```json
{
  "success": true,
  "data": [
    {
      "date": "2025-08-22",
      "requests": 5000,
      "tokens": 12345678
    },
    {
      "date": "2025-08-21",
      "requests": 4800,
      "tokens": 11000000
    }
  ],
  "message": "操作成功",
  "timestamp": "2025-08-22T10:05:00Z"
}
```

---

## 3. 获取模型使用分布

**GET** `/api/stats/model-share`

获取不同模型的使用次数分布情况。

#### 响应示例

```json
{
  "success": true,
  "data": [
    {
      "model": "gpt-4",
      "requests": 500000
    },
    {
      "model": "claude-3-opus",
      "requests": 350000
    },
    {
      "model": "gemini-1.5-pro",
      "requests": 250000
    }
  ],
  "message": "操作成功",
  "timestamp": "2025-08-22T10:10:00Z"
}
```

---

## 4. 获取最新日志

**GET** `/api/stats/logs`

获取最近的公开日志（通常是成功请求的简要信息）。

#### 响应示例

```json
{
  "success": true,
  "data": [
    {
      "model_used": "gpt-4",
      "duration_ms": 1500,
      "status_code": 200,
      "created_at": "2025-08-22T10:14:00Z"
    },
    {
      "model_used": "claude-3-opus",
      "duration_ms": 1200,
      "status_code": 200,
      "created_at": "2025-08-22T10:13:55Z"
    }
  ],
  "message": "操作成功",
  "timestamp": "2025-08-22T10:15:00Z"
}
```
