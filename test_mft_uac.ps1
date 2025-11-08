# MFT UAC 提权测试脚本
# 测试 MFT Service 的管理员权限启动流程

param(
    [switch]$AsAdmin,
    [switch]$EnableMFT,
    [switch]$DisableMFT,
    [switch]$CheckStatus
)

$ErrorActionPreference = "Stop"
$ExePath = ".\target\release\ilauncher.exe"
$ConfigPath = "$env:APPDATA\iLauncher\config\config.json"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  MFT UAC 提权测试" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 检查 exe 是否存在
if (-not (Test-Path $ExePath)) {
    Write-Host "❌ 错误: 未找到 ilauncher.exe" -ForegroundColor Red
    Write-Host "   请先编译: cargo build --release" -ForegroundColor Yellow
    exit 1
}

# 测试 1: 启用 MFT
if ($EnableMFT) {
    Write-Host "📝 测试 1: 启用 MFT" -ForegroundColor Green
    Write-Host "-----------------------------------"
    
    # 创建配置文件
    $configDir = Split-Path $ConfigPath -Parent
    if (-not (Test-Path $configDir)) {
        New-Item -ItemType Directory -Path $configDir -Force | Out-Null
    }
    
    $config = @{
        general = @{
            hotkey = "Alt+Space"
            search_delay = 100
            max_results = 10
            language = "zh-CN"
            clear_on_hide = $true
        }
        appearance = @{
            theme = "dark"
            language = "zh-CN"
            window_width = 800
            window_height = 600
            font_size = 14
            transparency = 95
            show_preview = $true
        }
        plugins = @{
            enabled_plugins = @("file_search")
            disabled_plugins = @()
        }
        advanced = @{
            start_on_boot = $false
            show_tray_icon = $true
            enable_analytics = $false
            cache_enabled = $true
            enable_mft = $true
        }
    } | ConvertTo-Json -Depth 10
    
    Set-Content -Path $ConfigPath -Value $config -Encoding UTF8
    Write-Host "✓ 配置文件已更新: enable_mft = true" -ForegroundColor Green
    Write-Host "   位置: $ConfigPath" -ForegroundColor Gray
    Write-Host ""
    
    Write-Host "⚠️  接下来会启动 UI，请注意 UAC 提示！" -ForegroundColor Yellow
    Write-Host "   1. UI 启动后会立即弹出 UAC 对话框" -ForegroundColor Gray
    Write-Host "   2. 点击 '是' 允许 MFT Service 以管理员权限运行" -ForegroundColor Gray
    Write-Host "   3. 查看任务管理器应该有两个 ilauncher.exe 进程" -ForegroundColor Gray
    Write-Host ""
    
    Read-Host "按 Enter 启动 UI..."
    
    # 启动 UI（不以管理员运行）
    Start-Process -FilePath $ExePath -WorkingDirectory (Get-Location)
    
    Write-Host ""
    Write-Host "✓ UI 已启动（普通权限）" -ForegroundColor Green
    Write-Host "  应该会弹出 UAC 对话框请求管理员权限启动 MFT Service" -ForegroundColor Gray
    Write-Host ""
    
    Start-Sleep -Seconds 3
    
    Write-Host "检查进程..." -ForegroundColor Cyan
    $processes = Get-Process ilauncher -ErrorAction SilentlyContinue
    if ($processes) {
        Write-Host "✓ 找到 $($processes.Count) 个 ilauncher 进程:" -ForegroundColor Green
        foreach ($proc in $processes) {
            Write-Host "  - PID: $($proc.Id), 内存: $([math]::Round($proc.WorkingSet64/1MB, 2)) MB" -ForegroundColor Gray
        }
    } else {
        Write-Host "⚠️  未找到 ilauncher 进程" -ForegroundColor Yellow
    }
    
    exit 0
}

# 测试 2: 禁用 MFT
if ($DisableMFT) {
    Write-Host "📝 测试 2: 禁用 MFT" -ForegroundColor Green
    Write-Host "-----------------------------------"
    
    if (Test-Path $ConfigPath) {
        $config = Get-Content $ConfigPath | ConvertFrom-Json
        $config.advanced.enable_mft = $false
        $config | ConvertTo-Json -Depth 10 | Set-Content $ConfigPath -Encoding UTF8
        
        Write-Host "✓ 配置文件已更新: enable_mft = false" -ForegroundColor Green
        Write-Host "   位置: $ConfigPath" -ForegroundColor Gray
    } else {
        Write-Host "⚠️  配置文件不存在，跳过" -ForegroundColor Yellow
    }
    
    # 停止所有 ilauncher 进程
    Write-Host ""
    Write-Host "停止所有 ilauncher 进程..." -ForegroundColor Cyan
    Get-Process ilauncher -ErrorAction SilentlyContinue | Stop-Process -Force
    Write-Host "✓ 已停止" -ForegroundColor Green
    
    exit 0
}

# 测试 3: 检查状态
if ($CheckStatus) {
    Write-Host "📊 当前状态检查" -ForegroundColor Green
    Write-Host "-----------------------------------"
    
    # 检查配置
    if (Test-Path $ConfigPath) {
        $config = Get-Content $ConfigPath | ConvertFrom-Json
        Write-Host "配置文件:" -ForegroundColor Cyan
        Write-Host "  enable_mft: $($config.advanced.enable_mft)" -ForegroundColor White
    } else {
        Write-Host "⚠️  未找到配置文件" -ForegroundColor Yellow
    }
    
    Write-Host ""
    
    # 检查进程
    Write-Host "运行中的进程:" -ForegroundColor Cyan
    $processes = Get-Process ilauncher -ErrorAction SilentlyContinue
    if ($processes) {
        foreach ($proc in $processes) {
            Write-Host "  PID: $($proc.Id)" -ForegroundColor White
            Write-Host "    内存: $([math]::Round($proc.WorkingSet64/1MB, 2)) MB" -ForegroundColor Gray
            Write-Host "    启动时间: $($proc.StartTime)" -ForegroundColor Gray
        }
    } else {
        Write-Host "  无运行中的进程" -ForegroundColor Gray
    }
    
    Write-Host ""
    
    # 检查数据库
    Write-Host "MFT 数据库:" -ForegroundColor Cyan
    $dbPath = "$env:TEMP\ilauncher_mft"
    if (Test-Path $dbPath) {
        $dbs = Get-ChildItem -Path $dbPath -Filter "*.db"
        if ($dbs) {
            foreach ($db in $dbs) {
                Write-Host "  $($db.Name): $([math]::Round($db.Length/1MB, 2)) MB" -ForegroundColor White
            }
        } else {
            Write-Host "  无数据库文件" -ForegroundColor Gray
        }
    } else {
        Write-Host "  目录不存在" -ForegroundColor Gray
    }
    
    Write-Host ""
    
    # 检查日志
    Write-Host "日志文件:" -ForegroundColor Cyan
    $logPath = "$env:TEMP\ilauncher_mft_scanner.log"
    if (Test-Path $logPath) {
        $logSize = (Get-Item $logPath).Length
        Write-Host "  $logPath" -ForegroundColor White
        Write-Host "  大小: $([math]::Round($logSize/1KB, 2)) KB" -ForegroundColor Gray
        Write-Host ""
        Write-Host "  最后 10 行:" -ForegroundColor Gray
        Get-Content $logPath -Tail 10 | ForEach-Object {
            Write-Host "    $_" -ForegroundColor DarkGray
        }
    } else {
        Write-Host "  不存在" -ForegroundColor Gray
    }
    
    exit 0
}

# 测试 4: 手动测试 MFT Service（以管理员运行）
if ($AsAdmin) {
    Write-Host "🔐 测试 4: 手动启动 MFT Service (管理员模式)" -ForegroundColor Green
    Write-Host "-----------------------------------"
    
    # 检查是否已经是管理员
    $currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    $isAdmin = $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    
    if (-not $isAdmin) {
        Write-Host "⚠️  当前不是管理员权限，正在请求提权..." -ForegroundColor Yellow
        Start-Process powershell -Verb RunAs -ArgumentList "-NoExit", "-File", $MyInvocation.MyCommand.Path, "-AsAdmin"
        exit 0
    }
    
    Write-Host "✓ 当前以管理员身份运行" -ForegroundColor Green
    Write-Host ""
    
    Write-Host "启动 MFT Service..." -ForegroundColor Cyan
    Write-Host "命令: $ExePath --mft-service" -ForegroundColor Gray
    Write-Host ""
    
    # 启动 MFT Service
    & $ExePath --mft-service
    
    exit 0
}

# 默认：显示帮助
Write-Host "用法:" -ForegroundColor Cyan
Write-Host "  .\test_mft_uac.ps1 -EnableMFT     # 启用 MFT 并启动 UI（会弹 UAC）" -ForegroundColor White
Write-Host "  .\test_mft_uac.ps1 -DisableMFT    # 禁用 MFT 并停止进程" -ForegroundColor White
Write-Host "  .\test_mft_uac.ps1 -CheckStatus   # 检查当前状态" -ForegroundColor White
Write-Host "  .\test_mft_uac.ps1 -AsAdmin       # 手动以管理员启动 MFT Service" -ForegroundColor White
Write-Host ""
Write-Host "示例测试流程:" -ForegroundColor Yellow
Write-Host "  1. .\test_mft_uac.ps1 -EnableMFT   # 启用并测试 UAC" -ForegroundColor Gray
Write-Host "  2. .\test_mft_uac.ps1 -CheckStatus # 查看状态" -ForegroundColor Gray
Write-Host "  3. .\test_mft_uac.ps1 -DisableMFT  # 清理" -ForegroundColor Gray
