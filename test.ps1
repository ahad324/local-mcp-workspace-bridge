# Configuration (Update the port if you changed it in settings)
$port = 3001
$url = "http://localhost:$port/mcp"
$workspace = "G:\Ahad\WorkspaceOS\QUICK_MVP\local-mcp-workspace-bridge"
$testFile = Join-Path $workspace "mcp_manual_test.txt"

Write-Host "Testing MCP Server at $url" -ForegroundColor Cyan
Write-Host ""

# 1. Initialize the connection
Write-Host "1. Sending Initialize request..." -ForegroundColor Yellow
$init = @{ jsonrpc="2.0"; id=1; method="initialize"; params=@{} } | ConvertTo-Json
try {
    $response = Invoke-RestMethod -Uri $url -Method Post -ContentType "application/json" -Body $init
    Write-Host "   Success: $($response.result.serverInfo.name)" -ForegroundColor Green
} catch {
    Write-Host "   Failed: $_" -ForegroundColor Red
}

# 2. Write a file (Edit/Create)
Write-Host ""
Write-Host "2. Sending Write File request to: $testFile" -ForegroundColor Yellow
$write = @{
    jsonrpc="2.0"
    id=2
    method="tools/call"
    params=@{
        name="write_file"
        arguments=@{
            path=$testFile
            content="Hello! This file was created by a manual PowerShell test."
        }
    }
} | ConvertTo-Json -Depth 10

try {
    $response = Invoke-RestMethod -Uri $url -Method Post -ContentType "application/json" -Body $write
    Write-Host "   Success: $($response.result.content[0].text)" -ForegroundColor Green
} catch {
    Write-Host "   Failed: $_" -ForegroundColor Red
    Write-Host "   Error Details: $($_.ErrorDetails.Message)" -ForegroundColor Red
}

# 3. Read the file back
Write-Host ""
Write-Host "3. Sending Read File request..." -ForegroundColor Yellow
$read = @{
    jsonrpc="2.0"
    id=3
    method="tools/call"
    params=@{
        name="read_file"
        arguments=@{
            path=$testFile
        }
    }
} | ConvertTo-Json -Depth 10

try {
    $response = Invoke-RestMethod -Uri $url -Method Post -ContentType "application/json" -Body $read
    Write-Host "   Success! File content:" -ForegroundColor Green
    Write-Host "   === $($response.result.content[0].text) ===" -ForegroundColor White
} catch {
    Write-Host "   Failed: $_" -ForegroundColor Red
    Write-Host "   Error Details: $($_.ErrorDetails.Message)" -ForegroundColor Red
}

Write-Host ""
Write-Host "Manual test complete!" -ForegroundColor Cyan