# 系统监控 API 文档

## 概述

本文档描述了系统监控相关的接口，用于获取系统当前的健康状况和资源使用情况。

## 认证

所有接口都需要管理员权限认证。

---

## 1. 获取系统详细信息

### 接口信息
- **请求路由**: `GET /api/system/info`
- **请求方法**: GET
- **作用**: 获取服务的详细信息，包括版本、构建时间、Git 提交、运行时和配置等。

### 返回值
```json
{
    "success": true,
    "data": {
        "service": {
            "name": "AI Proxy",
            "version": "0.1.0",
            "build_time": "2024-01-01T12:00:00Z",
            "git_commit": "abcdef123"
        },
        "runtime": {
            "uptime_seconds": 3600,
            "rust_version": "1.75.0",
            "target": "x86_64-unknown-linux-gnu"
        },
        "configuration": {
            "management_port": 8081,
            "proxy_port": 8080,
            "workers": 8,
            "database_url": "postgres://***:***@localhost/ai_proxy"
        }
    },
    "message": "操作成功",
    "timestamp": "2025-08-21T10:00:00.000Z"
}
```

---

## 2. 获取系统实时监控指标

### 接口信息
- **请求路由**: `GET /api/system/metrics`
- **请求方法**: GET
- **作用**: 获取系统核心监控指标，包括CPU、内存、磁盘使用率和系统运行时间。

### 返回值
```json
{
    "success": true,
    "data": {
        "cpu_usage": 25.5,                    // CPU 使用率 (%)
        "memory": {
            "total_mb": 16384,                // 总内存 (MB)
            "used_mb": 8192,                  // 已用内存 (MB)
            "usage_percentage": 50.0          // 内存使用率 (%)
        },
        "disk": {
            "total_gb": 500,                  // 总磁盘空间 (GB)
            "used_gb": 250,                   // 已用磁盘空间 (GB)
            "usage_percentage": 50.0          // 磁盘使用率 (%)
        },
        "uptime": "12d 4h 32m"                // 系统正常运行时间
    },
    "message": "操作成功",
    "timestamp": "2025-08-21T10:00:00.000Z"
}
```

### 字段说明
| 字段名 | 类型 | 描述 |
|--------|------|------|
| cpu_usage | float | 当前CPU使用率，百分比 |
| memory | object | 内存使用情况 |
| memory.total_mb | int | 总物理内存，单位MB |
| memory.used_mb | int | 已使用内存，单位MB |
| memory.usage_percentage | float | 内存使用率，百分比 |
| disk | object | 磁盘使用情况 |
| disk.total_gb | int | 总磁盘空间，单位GB |
| disk.used_gb | int | 已使用磁盘空间，单位GB |
| disk.usage_percentage | float | 磁盘使用率，百分比 |
| uptime | string | 系统自上次启动以来的运行时间 |

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
    "timestamp": "2025-08-21T10:00:00.000Z"
}
```
