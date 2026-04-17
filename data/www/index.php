<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>PHP-Stack V2.0 测试页面</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
            border-radius: 10px;
            margin-bottom: 30px;
            text-align: center;
        }
        .section {
            background: white;
            padding: 20px;
            margin-bottom: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .success {
            color: #10b981;
            font-weight: bold;
        }
        .error {
            color: #ef4444;
            font-weight: bold;
        }
        .info {
            background: #e0f2fe;
            padding: 10px;
            border-left: 4px solid #0ea5e9;
            margin: 10px 0;
        }
        table {
            width: 100%;
            border-collapse: collapse;
        }
        th, td {
            padding: 10px;
            text-align: left;
            border-bottom: 1px solid #e5e7eb;
        }
        th {
            background: #f9fafb;
            font-weight: 600;
        }
        code {
            background: #f3f4f6;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: 'Consolas', monospace;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>🚀 PHP-Stack V2.0 测试页面</h1>
        <p>向导式环境搭建验证</p>
    </div>

    <?php
    // 测试 MySQL 连接
    $mysql_status = '未测试';
    $mysql_error = '';
    try {
        $pdo = new PDO('mysql:host=mysql;port=3306;dbname=app', 'root', 'root123');
        $pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
        $stmt = $pdo->query('SELECT VERSION() as version');
        $version = $stmt->fetch(PDO::FETCH_ASSOC)['version'];
        $mysql_status = "✅ 连接成功 (MySQL $version)";
    } catch (PDOException $e) {
        $mysql_status = '❌ 连接失败';
        $mysql_error = $e->getMessage();
    }

    // 测试 Redis 连接
    $redis_status = '未测试';
    try {
        if (class_exists('Redis')) {
            $redis = new Redis();
            $connected = @$redis->connect('redis', 6379);
            if ($connected) {
                $redis->set('test_key', 'Hello from PHP-Stack V2.0!');
                $value = $redis->get('test_key');
                $redis_status = "✅ 连接成功 (测试值: $value)";
            } else {
                $redis_status = '❌ 连接失败';
            }
        } else {
            $redis_status = '❌ Redis 扩展未安装';
        }
    } catch (Exception $e) {
        $redis_status = '❌ 连接失败: ' . $e->getMessage();
    }

    // 获取 PHP 信息
    $php_version = phpversion();
    $loaded_extensions = get_loaded_extensions();
    
    // 检查关键扩展
    $required_extensions = ['mysqli', 'pdo_mysql', 'redis', 'gd', 'mbstring', 'curl'];
    $extension_status = [];
    foreach ($required_extensions as $ext) {
        $extension_status[$ext] = extension_loaded($ext) ? '✅' : '❌';
    }
    ?>

    <!-- PHP 信息 -->
    <div class="section">
        <h2>📋 PHP 环境信息</h2>
        <table>
            <tr>
                <th>项目</th>
                <th>状态</th>
            </tr>
            <tr>
                <td>PHP 版本</td>
                <td><code><?= htmlspecialchars($php_version) ?></code></td>
            </tr>
            <tr>
                <td>服务器软件</td>
                <td><code><?= htmlspecialchars($_SERVER['SERVER_SOFTWARE'] ?? 'Unknown') ?></code></td>
            </tr>
            <tr>
                <td>文档根目录</td>
                <td><code><?= htmlspecialchars($_SERVER['DOCUMENT_ROOT'] ?? 'Unknown') ?></code></td>
            </tr>
        </table>
    </div>

    <!-- 扩展检查 -->
    <div class="section">
        <h2>🔧 PHP 扩展检查</h2>
        <table>
            <tr>
                <th>扩展名称</th>
                <th>状态</th>
            </tr>
            <?php foreach ($extension_status as $ext => $status): ?>
            <tr>
                <td><?= htmlspecialchars($ext) ?></td>
                <td class="<?= $status === '✅' ? 'success' : 'error' ?>">
                    <?= $status ?>
                </td>
            </tr>
            <?php endforeach; ?>
        </table>
        <div class="info">
            💡 提示：如果看到 ❌，请检查 Dockerfile 中的扩展安装配置
        </div>
    </div>

    <!-- 数据库连接 -->
    <div class="section">
        <h2>🗄️ 数据库连接测试</h2>
        <table>
            <tr>
                <th>服务</th>
                <th>状态</th>
            </tr>
            <tr>
                <td>MySQL (mysql:3306)</td>
                <td class="<?= strpos($mysql_status, '✅') !== false ? 'success' : 'error' ?>">
                    <?= htmlspecialchars($mysql_status) ?>
                </td>
            </tr>
            <tr>
                <td>Redis (redis:6379)</td>
                <td class="<?= strpos($redis_status, '✅') !== false ? 'success' : 'error' ?>">
                    <?= htmlspecialchars($redis_status) ?>
                </td>
            </tr>
        </table>
        
        <?php if ($mysql_error): ?>
        <div class="info" style="background: #fee2e2; border-color: #ef4444;">
            <strong>错误详情：</strong><br>
            <code><?= htmlspecialchars($mysql_error) ?></code>
        </div>
        <?php endif; ?>
    </div>

    <!-- 已加载的扩展列表 -->
    <div class="section">
        <h2>📦 已加载的 PHP 扩展 (共 <?= count($loaded_extensions) ?> 个)</h2>
        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 10px;">
            <?php foreach ($loaded_extensions as $ext): ?>
            <div style="background: #f3f4f6; padding: 8px; border-radius: 4px; font-size: 14px;">
                <?= htmlspecialchars($ext) ?>
            </div>
            <?php endforeach; ?>
        </div>
    </div>

    <!-- 下一步建议 -->
    <div class="section">
        <h2>🎯 下一步</h2>
        <ul>
            <li>将您的项目文件放到 <code>data/www/</code> 目录</li>
            <li>访问 <a href="http://localhost">http://localhost</a> 查看您的应用</li>
            <li>使用数据库工具连接 MySQL：<code>localhost:3306</code>（密码：<code>root123</code>）</li>
            <li>查看完整 PHP 信息：<a href="?phpinfo=1">phpinfo()</a></li>
        </ul>
    </div>

    <?php if (isset($_GET['phpinfo'])): ?>
    <div class="section">
        <h2>📊 PHP Info</h2>
        <?php phpinfo(); ?>
    </div>
    <?php endif; ?>

    <footer style="text-align: center; padding: 20px; color: #6b7280;">
        <p>PHP-Stack V2.0 - 向导式开发环境搭建</p>
        <p>部署时间: <?= date('Y-m-d H:i:s') ?></p>
    </footer>
</body>
</html>
