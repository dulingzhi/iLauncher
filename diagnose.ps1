# iLauncher 诊断脚本

Write-Host "=== iLauncher 诊断 ===" -ForegroundColor Cyan

# 1. 检查进程
Write-Host "`n1. 检查进程状态:" -ForegroundColor Yellow
$processes = Get-Process ilauncher -ErrorAction SilentlyContinue
if ($processes) {
    foreach ($p in $processes) {
        Write-Host "  ✓ PID: $($p.Id), 内存: $([math]::Round($p.WorkingSet64/1MB, 2))MB" -ForegroundColor Green
        Write-Host "    启动时间: $($p.StartTime)" -ForegroundColor Gray
        Write-Host "    命令行: " -NoNewline -ForegroundColor Gray
        
        # 尝试获取命令行参数
        try {
            $cmdLine = (Get-CimInstance Win32_Process -Filter "ProcessId = $($p.Id)").CommandLine
            if ($cmdLine -like "*--mft-service*") {
                Write-Host "$cmdLine" -ForegroundColor Magenta
            } else {
                Write-Host "$cmdLine" -ForegroundColor Cyan
            }
        } catch {
            Write-Host "无法获取" -ForegroundColor Red
        }
    }
} else {
    Write-Host "  ✗ 没有运行的 ilauncher 进程" -ForegroundColor Red
}

# 2. 检查 MFT 数据库
Write-Host "`n2. 检查 MFT 数据库:" -ForegroundColor Yellow
$mftDbDir = "$env:LOCALAPPDATA\iLauncher\mft_databases"
if (Test-Path $mftDbDir) {
    $dbFiles = Get-ChildItem "$mftDbDir\*.db" -ErrorAction SilentlyContinue
    if ($dbFiles) {
        Write-Host "  ✓ 数据库目录: $mftDbDir" -ForegroundColor Green
        foreach ($db in $dbFiles) {
            $sizeMB = [math]::Round($db.Length/1MB, 2)
            Write-Host "    - $($db.Name): ${sizeMB}MB (修改于 $($db.LastWriteTime))" -ForegroundColor Gray
        }
    } else {
        Write-Host "  ⚠ 数据库目录存在但无数据库文件" -ForegroundColor Yellow
    }
} else {
    Write-Host "  ✗ 数据库目录不存在: $mftDbDir" -ForegroundColor Red
}

# 3. 检查日志
Write-Host "`n3. 检查日志文件:" -ForegroundColor Yellow
$logDir = "$env:LOCALAPPDATA\iLauncher\logs"
if (Test-Path $logDir) {
    $logFiles = Get-ChildItem "$logDir\*.log" -ErrorAction SilentlyContinue
    if ($logFiles) {
        Write-Host "  ✓ 日志目录: $logDir" -ForegroundColor Green
        foreach ($log in $logFiles) {
            Write-Host "    - $($log.Name) ($([math]::Round($log.Length/1KB, 2))KB)" -ForegroundColor Gray
            Write-Host "      最后10行:" -ForegroundColor Gray
            Get-Content $log.FullName -Tail 10 | ForEach-Object {
                Write-Host "      $_" -ForegroundColor DarkGray
            }
        }
    } else {
        Write-Host "  ⚠ 日志目录存在但无日志文件" -ForegroundColor Yellow
    }
} else {
    Write-Host "  ✗ 日志目录不存在: $logDir" -ForegroundColor Red
}

# 4. 检查配置
Write-Host "`n4. 检查配置:" -ForegroundColor Yellow
$configFile = "$env:LOCALAPPDATA\iLauncher\config\config.json"
if (Test-Path $configFile) {
    Write-Host "  ✓ 配置文件: $configFile" -ForegroundColor Green
    $config = Get-Content $configFile -Raw | ConvertFrom-Json
    
    # 查找 file_search 插件配置
    if ($config.plugins) {
        $fileSearchPlugin = $config.plugins | Where-Object { $_.id -eq "file_search" }
        if ($fileSearchPlugin) {
            Write-Host "    file_search 插件状态:" -ForegroundColor Gray
            Write-Host "      enabled: $($fileSearchPlugin.enabled)" -ForegroundColor $(if ($fileSearchPlugin.enabled) { "Green" } else { "Red" })
            if ($fileSearchPlugin.config -and $fileSearchPlugin.config.use_mft) {
                Write-Host "      use_mft: $($fileSearchPlugin.config.use_mft)" -ForegroundColor $(if ($fileSearchPlugin.config.use_mft) { "Green" } else { "Red" })
            }
        } else {
            Write-Host "  ⚠ 未找到 file_search 插件配置" -ForegroundColor Yellow
        }
    }
} else {
    Write-Host "  ✗ 配置文件不存在: $configFile" -ForegroundColor Red
}

# 5. 性能建议
Write-Host "`n5. 性能建议:" -ForegroundColor Yellow
if ($processes -and $dbFiles) {
    Write-Host "  ✓ Service 运行中且数据库存在" -ForegroundColor Green
    Write-Host "  - 搜索应该使用 MFT 模式（快速）" -ForegroundColor Gray
} else {
    Write-Host "  ⚠ Service 未运行或数据库不存在" -ForegroundColor Yellow
    Write-Host "  - 搜索将降级到 BFS 模式（慢速）" -ForegroundColor Gray
}

Write-Host "`n=== 诊断完成 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "建议操作:" -ForegroundColor Yellow
if (-not $processes) {
    Write-Host "  1. 启动 UI: .\target\release\ilauncher.exe" -ForegroundColor White
}
if (-not $dbFiles) {
    Write-Host "  2. 等待 MFT Service 完成初始扫描（可能需要几分钟）" -ForegroundColor White
}
Write-Host "  3. 查看实时日志: Get-Content '$env:LOCALAPPDATA\iLauncher\logs\mft_scanner.log' -Wait" -ForegroundColor White
