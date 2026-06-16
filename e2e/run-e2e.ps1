param(
  [string]$Port = "3001",
  [switch]$KeepOpen
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
$Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$TempDir = "$env:TEMP\atendemente-e2e-$Timestamp"

# Create temp directories
New-Item -ItemType Directory -Path "$TempDir\data" -Force | Out-Null
New-Item -ItemType Directory -Path "$TempDir\uploads" -Force | Out-Null

Write-Host "=== AtendeMente E2E Test Runner ===" -ForegroundColor Cyan
Write-Host "Temp dir: $TempDir" -ForegroundColor Gray

# Set environment variables for temp DBs
$env:DATABASE_URL = "sqlite:$TempDir\data\app.db?mode=rwc"
$env:AUTH_DATABASE_URL = "sqlite:$TempDir\data\auth.db?mode=rwc"
$env:SERVER_PORT = $Port
$env:STORAGE_DIR = "$TempDir\uploads"
$env:RUST_LOG = "info"

# Build the server binary (if needed)
Write-Host "`n[1/4] Building server binary..." -ForegroundColor Yellow
Set-Location -Path "$RootDir\src-tauri"
cargo build --bin server 2>&1 | Out-Host
if ($LASTEXITCODE -ne 0) {
  Write-Host "Failed to build server binary" -ForegroundColor Red
  exit 1
}

$ServerBin = "$RootDir\src-tauri\target\debug\server.exe"
if (-not (Test-Path $ServerBin)) {
  Write-Host "Server binary not found at $ServerBin" -ForegroundColor Red
  exit 1
}

# Start Vite dev server
Write-Host "`n[2/4] Starting Vite dev server..." -ForegroundColor Yellow
$ViteProcess = Start-Process -FilePath "npx.cmd" -ArgumentList "vite" -WorkingDirectory $RootDir -NoNewWindow -PassThru
Start-Sleep -Seconds 5

# Start Rust server (inherits env vars from current process)
Write-Host "`n[3/4] Starting API server on port $Port..." -ForegroundColor Yellow
$ServerProcess = Start-Process -FilePath $ServerBin -ArgumentList "--port", $Port -NoNewWindow -PassThru
Start-Sleep -Seconds 3

# Health check
Write-Host "Checking server health..." -ForegroundColor Gray
$healthy = $false
for ($i = 0; $i -lt 10; $i++) {
  try {
    $res = Invoke-WebRequest -Uri "http://localhost:$Port/api/health" -UseBasicParsing -TimeoutSec 2
    if ($res.StatusCode -eq 200) {
      $healthy = $true
      break
    }
  } catch {
    Write-Host "  Waiting... ($i)" -ForegroundColor DarkGray
  }
  Start-Sleep -Seconds 2
}

if (-not $healthy) {
  Write-Host "Server failed to start" -ForegroundColor Red
  Stop-Process -Id $ViteProcess.Id -Force -ErrorAction SilentlyContinue
  Stop-Process -Id $ServerProcess.Id -Force -ErrorAction SilentlyContinue
  exit 1
}

Write-Host "Server is healthy!" -ForegroundColor Green

# Run Playwright tests
Write-Host "`n[4/4] Running Playwright tests..." -ForegroundColor Yellow
Set-Location -Path $RootDir
npx playwright test --config e2e\playwright.config.ts
$exitCode = $LASTEXITCODE

# Cleanup
if (-not $KeepOpen) {
  Write-Host "`nCleaning up..." -ForegroundColor Gray
  Stop-Process -Id $ViteProcess.Id -Force -ErrorAction SilentlyContinue
  Stop-Process -Id $ServerProcess.Id -Force -ErrorAction SilentlyContinue
  Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "`n=== E2E Tests Complete (exit code: $exitCode) ===" -ForegroundColor Cyan
exit $exitCode
