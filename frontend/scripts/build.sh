#!/bin/bash

# 前端构建脚本
# 用于生产环境构建和部署准备

set -e

echo "🚀 开始构建前端项目..."

# 检查Node.js环境
if ! command -v node &> /dev/null; then
    echo "❌ 错误: 未找到Node.js，请先安装Node.js"
    exit 1
fi

if ! command -v npm &> /dev/null; then
    echo "❌ 错误: 未找到npm，请先安装npm"
    exit 1
fi

# 获取当前目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# 切换到项目目录
cd "$PROJECT_DIR"

echo "📁 项目目录: $PROJECT_DIR"

# 清理旧的构建文件
echo "🧹 清理旧的构建文件..."
rm -rf dist
rm -rf node_modules/.vite

# 检查并安装依赖
echo "📦 检查项目依赖..."
if [ ! -d "node_modules" ]; then
    echo "📥 安装项目依赖..."
    npm ci --production=false
else
    echo "✅ 依赖已存在，跳过安装"
fi

# 运行类型检查（可选）
echo "🔍 跳过TypeScript类型检查（可选步骤）..."
echo "💡 如需类型检查，请运行: npm run type-check"

# 运行代码检查
echo "📝 运行代码检查..."
if command -v eslint &> /dev/null; then
    if npm run lint:check; then
        echo "✅ 代码检查通过"
    else
        echo "⚠️  代码检查失败，但继续构建..."
    fi
else
    echo "⚠️  未找到ESLint，跳过代码检查"
fi

# 生产环境构建
echo "🏭 开始生产环境构建..."
NODE_ENV=production npm run build:prod

# 检查构建结果
if [ -d "dist" ]; then
    echo "✅ 构建成功完成！"
    
    # 计算构建文件大小
    BUILD_SIZE=$(du -sh dist | cut -f1)
    echo "📊 构建文件大小: $BUILD_SIZE"
    
    # 显示主要文件
    echo "📄 主要构建文件:"
    find dist -name "*.js" -o -name "*.css" | head -10 | while read file; do
        size=$(du -h "$file" | cut -f1)
        echo "  - $(basename "$file"): $size"
    done
    
else
    echo "❌ 构建失败！"
    exit 1
fi

# 生成构建信息
echo "📋 生成构建信息..."
cat > dist/build-info.json << EOF
{
  "buildTime": "$(date -Iseconds)",
  "version": "$(node -p "require('./package.json').version")",
  "gitCommit": "$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')",
  "gitBranch": "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'unknown')",
  "nodeVersion": "$(node --version)",
  "npmVersion": "$(npm --version)"
}
EOF

# 创建部署说明
echo "📝 创建部署说明..."
cat > dist/README.md << EOF
# AI代理平台 - 前端构建

## 构建信息
- 构建时间: $(date)
- 版本: $(node -p "require('./package.json').version")
- Node.js版本: $(node --version)

## 部署说明

### 静态文件服务器部署
将 \`dist\` 目录中的所有文件部署到Web服务器的根目录。

### Nginx配置示例
\`\`\`nginx
server {
    listen 80;
    server_name your-domain.com;
    root /path/to/dist;
    index index.html;

    # 处理Vue Router的单页应用路由
    location / {
        try_files \$uri \$uri/ /index.html;
    }

    # 静态资源缓存
    location /assets/ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # API代理 (可选)
    location /api/ {
        proxy_pass http://localhost:9090;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
\`\`\`

### Apache配置示例
\`\`\`apache
<VirtualHost *:80>
    ServerName your-domain.com
    DocumentRoot /path/to/dist
    
    # 处理Vue Router的单页应用路由
    <Directory "/path/to/dist">
        RewriteEngine On
        RewriteBase /
        RewriteRule ^index\.html$ - [L]
        RewriteCond %{REQUEST_FILENAME} !-f
        RewriteCond %{REQUEST_FILENAME} !-d
        RewriteRule . /index.html [L]
    </Directory>
    
    # 静态资源缓存
    <LocationMatch "^/assets/">
        ExpiresActive On
        ExpiresDefault "access plus 1 year"
        Header append Cache-Control "public, immutable"
    </LocationMatch>
</VirtualHost>
\`\`\`

## 环境变量配置
确保在生产环境中正确配置以下环境变量：
- VITE_API_BASE_URL: 后端API地址
- VITE_WS_URL: WebSocket地址

## 浏览器支持
- Chrome/Edge ≥88
- Firefox ≥78  
- Safari ≥14
EOF

echo ""
echo "🎉 构建完成！"
echo "📁 构建文件位于: $PROJECT_DIR/dist"
echo "📋 部署说明: $PROJECT_DIR/dist/README.md"
echo ""
echo "💡 接下来可以："
echo "   1. 将 dist 目录部署到Web服务器"
echo "   2. 配置反向代理到后端API (localhost:9090)"
echo "   3. 启动Web服务器并访问应用"
echo ""