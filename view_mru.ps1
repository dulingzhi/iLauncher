# 查看 MRU 数据库内容
$dbPath = "$env:LOCALAPPDATA\iLauncher\data\statistics.db"

if (!(Test-Path $dbPath)) {
    Write-Host "数据库不存在: $dbPath" -ForegroundColor Red
    exit 1
}

Write-Host "数据库路径: $dbPath" -ForegroundColor Cyan
Write-Host ""

# 使用 .NET SQLite 库
Add-Type -Path "C:\Windows\Microsoft.NET\assembly\GAC_MSIL\System.Data.SQLite\v4.0_1.0.118.0__db937bc2d44ff139\System.Data.SQLite.dll" -ErrorAction SilentlyContinue

try {
    $conn = New-Object System.Data.SQLite.SQLiteConnection
    $conn.ConnectionString = "Data Source=$dbPath"
    $conn.Open()
    
    # 查询热门结果
    $sql = "SELECT result_id, plugin_id, title, count, last_used FROM result_clicks ORDER BY count DESC, last_used DESC LIMIT 10"
    $cmd = $conn.CreateCommand()
    $cmd.CommandText = $sql
    $reader = $cmd.ExecuteReader()
    
    Write-Host "🔥 热门结果 (Top 10):" -ForegroundColor Yellow
    Write-Host "=" * 100
    
    $index = 1
    while ($reader.Read()) {
        $id = $reader["result_id"]
        $plugin = $reader["plugin_id"]
        $title = $reader["title"]
        $count = $reader["count"]
        $lastUsed = $reader["last_used"]
        
        Write-Host "$index. [$count 次] $title" -ForegroundColor Green
        Write-Host "   ID: $id" -ForegroundColor Gray
        Write-Host "   Plugin: $plugin | Last: $lastUsed" -ForegroundColor Gray
        Write-Host ""
        $index++
    }
    
    $reader.Close()
    $conn.Close()
    
} catch {
    Write-Host "错误: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "尝试使用替代方法..." -ForegroundColor Yellow
    
    # 如果 SQLite 库不可用，提示安装
    Write-Host "请先安装 SQLite:" -ForegroundColor Cyan
    Write-Host "  winget install sqlite.sqlite" -ForegroundColor White
}
