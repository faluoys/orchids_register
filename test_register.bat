@echo off
echo ========================================
echo Orchids Auto Register - 快速测试
echo ========================================
echo.

echo [1/3] 检查编译...
cd /d D:\Code\olareg\Orchids-auto-register-main\Orchids-auto-register-main
cargo build --release
if %errorlevel% neq 0 (
    echo 编译失败！请检查错误信息。
    pause
    exit /b 1
)

echo.
echo [2/3] 启动打码 API...
start "打码API" cmd /k "cd /d D:\Code\grokzhuce-main\打码 && python api_solver.py --port 5000 --thread 2"
echo 等待打码 API 启动...
timeout /t 10

echo.
echo [3/3] 运行测试注册...
echo.
echo 请确保已配置以下环境变量或修改此脚本：
echo - FREEMAIL_BASE_URL: freemail API 地址
echo - FREEMAIL_ADMIN_TOKEN: freemail 管理员令牌
echo.

REM 修改这里的配置
set FREEMAIL_BASE_URL=https://your-freemail.com
set FREEMAIL_ADMIN_TOKEN=your-jwt-token-here

cargo run --release -- ^
  --use-freemail ^
  --freemail-base-url "%FREEMAIL_BASE_URL%" ^
  --freemail-admin-token "%FREEMAIL_ADMIN_TOKEN%" ^
  --use-capmonster ^
  --captcha-api-url "http://127.0.0.1:5000" ^
  --use-proxy-pool ^
  --debug-email

echo.
echo ========================================
echo 测试完成！
echo ========================================
pause
