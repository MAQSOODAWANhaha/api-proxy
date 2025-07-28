# SSL证书配置指南

本指南介绍如何为域名 `domain.com` 配置SSL证书和自动续期。

## 🎯 概述

已创建以下文件用于SSL配置：

- `nginx-ssl.conf` - 专用SSL Nginx配置
- `ssl-manager.sh` - SSL证书管理脚本
- `docker-compose.ssl.yaml` - SSL服务扩展配置

## 🚀 快速开始

### 1. 初始化SSL环境

```bash
sudo ./ssl-manager.sh init
```

这将：
- 安装必要的依赖包
- 安装acme.sh证书管理工具
- 创建所需目录
- 生成Docker配置文件

### 2. 申请SSL证书

```bash
sudo ./ssl-manager.sh request
```

这将：
- 使用Let's Encrypt申请免费SSL证书
- 配置自动续期（每日检查）
- 安装证书到指定目录

### 3. 部署SSL服务

```bash
sudo ./ssl-manager.sh deploy
```

这将：
- 停止原有HTTP代理服务
- 启动新的HTTPS代理服务
- 显示证书和服务状态

## 📋 完整命令列表

```bash
# 查看所有可用命令
./ssl-manager.sh

# 查看证书和服务状态
./ssl-manager.sh status

# 手动续期证书
sudo ./ssl-manager.sh renew

# 移除SSL配置
sudo ./ssl-manager.sh remove
```

## 🔧 配置说明

### 域名配置
脚本默认配置域名为 `domain.com`，如需修改请编辑 `ssl-manager.sh` 中的配置变量：

```bash
DOMAIN="domain.com"
EMAIL="admin@zhanglei.vip"  # 请修改为实际邮箱
```

### 证书存储位置
- 系统证书目录: `/opt/ssl/live/domain.com/`
- Docker卷目录: `/var/lib/docker/volumes/api-proxy_ssl_certs/_data/`

### 自动续期
脚本会自动设置cron任务：
- 每日2点检查证书是否需要续期
- 每月1号和15号强制执行续期检查

## 🌐 服务访问

SSL部署完成后：
- HTTPS: `https://domain.com`
- HTTP自动重定向到HTTPS
- Let's Encrypt验证路径: `http://domain.com/.well-known/acme-challenge/`

## 🛠️ Docker服务管理

### 启动SSL服务
```bash
docker-compose -f docker-compose.yaml -f docker-compose.ssl.yaml up -d
```

### 查看SSL服务状态
```bash
docker-compose -f docker-compose.yaml -f docker-compose.ssl.yaml ps
```

### 查看SSL代理日志
```bash
docker logs api-proxy-ssl-gateway
```

### 停止SSL服务
```bash
docker-compose -f docker-compose.yaml -f docker-compose.ssl.yaml down
```

## 🔍 故障排除

### 1. 证书申请失败
- 检查域名DNS解析是否正确指向服务器IP
- 确保80端口可访问（用于ACME验证）
- 查看日志：`tail -f /var/log/ssl-manager.log`

### 2. Nginx启动失败
- 检查证书文件是否存在
- 验证nginx配置语法：`docker exec api-proxy-ssl-gateway nginx -t`
- 查看nginx日志：`docker logs api-proxy-ssl-gateway`

### 3. 自动续期失败
- 检查cron服务状态：`systemctl status cron`
- 查看续期日志：`tail -f /var/log/ssl-renewal.log`
- 手动测试续期：`sudo ./ssl-manager.sh renew`

## ⚠️ 注意事项

1. **首次运行需要root权限**：安装依赖和设置cron任务
2. **域名解析**：确保域名正确解析到服务器IP
3. **防火墙**：确保开放80和443端口
4. **邮箱配置**：修改脚本中的邮箱地址以接收续期通知
5. **生产环境**：首次测试建议使用Let's Encrypt的staging环境

## 📞 技术支持

如遇问题，请检查：
1. 日志文件：`/var/log/ssl-manager.log`
2. 证书状态：`./ssl-manager.sh status`
3. 服务状态：`docker-compose ps`
4. 网络连通性：`curl -I http://domain.com/health`