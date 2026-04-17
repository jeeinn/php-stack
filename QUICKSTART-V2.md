# V2.0 向导式环境搭建 - 快速启动

## 🚀 一键启动开发服务器

```bash
npm run tauri dev
```

## 📋 使用步骤

### 1️⃣ 选择技术栈
- ✅ PHP 8.2（默认）
  - 扩展：mysqli, pdo_mysql, redis, gd, mbstring（默认勾选）
- ✅ MySQL 8.0（默认）
- ✅ Redis 7.0（默认）
- ✅ Nginx 1.24（建议勾选）

### 2️⃣ 配置镜像源
- APT: 阿里云 ⭐
- Composer: 阿里云 ⭐
- 点击"保存配置"

### 3️⃣ 预览并部署
- 查看生成的配置
- 点击"🚀 一键启动"
- 等待部署完成（首次约 10-15 分钟）

## ✅ 验证部署

部署完成后，访问以下地址：

### Web 测试页面
```
http://localhost
```

应该看到 PHP-Stack V2.0 测试页面，显示：
- ✅ PHP 版本信息
- ✅ 扩展加载状态
- ✅ MySQL 连接测试
- ✅ Redis 连接测试

### 数据库连接
- **主机**: localhost
- **端口**: 3306
- **用户**: root
- **密码**: root123
- **数据库**: app

### Redis 连接
- **主机**: localhost
- **端口**: 6379

## 🔍 命令行验证

```bash
# 查看运行的容器
docker ps --filter "name=ps-"

# 测试 MySQL
docker exec -it ps-mysql-8-0 mysql -uroot -proot123 -e "SELECT 'MySQL OK!' as status;"

# 测试 Redis
docker exec -it ps-redis-7-0 redis-cli ping

# 测试 PHP 扩展
docker exec -it ps-php-8-2 php -r "echo extension_loaded('mysqli') ? 'MySQLi OK' : 'MySQLi FAIL';"

# 查看日志
docker logs ps-php-8-2
docker logs ps-nginx-1-24
```

## 📁 重要目录

```
php-stack/
├── data/
│   └── www/              # 👈 将您的项目文件放在这里
│       └── index.php     # 测试页面
├── nginx/
│   └── conf.d/           # Nginx 站点配置
│       └── default.conf
├── build/
│   └── php-8.2-custom/   # PHP 自定义镜像
├── .env                  # 镜像源配置
└── docker-compose.yml    # 自动生成的配置
```

## 🐛 遇到问题？

### 1. 镜像构建失败
**解决**: 确保在步骤 1 中选择了阿里云镜像源

### 2. 端口冲突
**解决**: 停止占用 80/3306/6379/9000 端口的服务

### 3. 访问 localhost 显示 404
**解决**: 确认 `data/www/index.php` 文件存在

### 4. MySQL 连接失败
**解决**: 等待 MySQL 完全启动（约 30 秒），密码是 `root123`

## 📖 更多文档

- [完整测试指南](./V2.0-TESTING-GUIDE.md)
- [实施总结](./IMPLEMENTATION-SUMMARY.md)
- [设计文档](./v2.0-wizard-environment-builder-design.md)

## 💡 提示

- 首次部署较慢（需要构建 PHP 镜像），请耐心等待
- 二次部署会使用缓存，速度很快（1-2 分钟）
- 所有数据保存在 `data/` 目录，删除容器不会丢失数据
- 可以随时修改 `.env` 文件切换镜像源

---

**祝您使用愉快！** 🎉
