# 前端配置说明

## 环境配置文件

### 配置文件优先级
1. `.env.local` - 本地开发配置（优先级最高，不提交到版本控制）
2. `.env.development` - 开发环境配置
3. `.env.production` - 生产环境配置

### 配置项说明

| 配置项 | 说明 | 示例 |
|--------|------|------|
| `VITE_APP_TITLE` | 应用标题 | `AI Proxy Admin` |
| `VITE_API_BASE_URL` | 后端API基础URL | `http://192.168.1.100:9090/api` |
| `VITE_WS_BASE_URL` | WebSocket基础URL | `ws://192.168.1.100:9090` |
| `VITE_APP_ENV` | 环境标识 | `development`, `production`, `local` |
| `VITE_ENABLE_MOCK` | 是否启用Mock数据 | `true`, `false` |

## 部署配置

### 开发环境
```bash
# 使用默认配置
npm run dev

# 或者创建 .env.local 文件自定义配置
cp .env.local.example .env.local
# 编辑 .env.local 文件
```

### 生产环境
```bash
# 构建生产版本
npm run build

# 或者指定自定义API地址
VITE_API_BASE_URL=https://api.example.com/api npm run build
```

### 前后端分离部署

#### 场景1：前后端同域部署
```env
VITE_API_BASE_URL=/api
```

#### 场景2：前后端不同服务器
```env
VITE_API_BASE_URL=http://backend-server:9090/api
```

#### 场景3：使用反向代理
```env
VITE_API_BASE_URL=/api
```
然后在Nginx中配置：
```nginx
location /api {
    proxy_pass http://backend-server:9090/api;
}
```

## 运行时配置检查

应用启动时会自动验证配置并在开发环境下输出配置信息到控制台。

如果发现配置问题，请检查：
1. 环境变量是否正确设置
2. 后端服务是否正常运行
3. 网络连接是否正常
4. CORS配置是否正确（如果前后端分离部署）