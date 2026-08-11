param(
  [string]$Port = "3001",
  [switch]$KeepOpen
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
$Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$TempDir = "$env:TEMP\atendemente-e2e-$Timestamp"

# Clean up previous job outputs
Get-Job -Name "e2e-*" -ErrorAction SilentlyContinue | Stop-Job -PassThru | Remove-Job

# Create temp directories
New-Item -ItemType Directory -Path "$TempDir\data" -Force | Out-Null
New-Item -ItemType Directory -Path "$TempDir\uploads" -Force | Out-Null

Write-Host "=== AtendeMente E2E Test Runner ===" -ForegroundColor Cyan
Write-Host "Temp dir: $TempDir" -ForegroundColor Gray

# Set environment variables for temp DBs
$env:DATABASE_URL = "sqlite:$TempDir\app.db?mode=rwc"
$env:AUTH_DATABASE_URL = "sqlite:$TempDir\auth.db?mode=rwc"
$env:SERVER_PORT = $Port
$env:STORAGE_DIR = "$TempDir\uploads"
# Sem isto, cada rodada deixa um diretorio de dados de teste no config real do
# usuario (~/.config/atendemente/data/<uuid>).
$env:DATA_DIR = "$TempDir\data"
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

# Start Vite dev server as background job
Write-Host "`n[2/4] Starting Vite dev server..." -ForegroundColor Yellow
$ViteJob = Start-Job -Name "e2e-vite" -ScriptBlock {
  param($dir)
  Set-Location $dir
  npx vite --port 1420
} -ArgumentList $RootDir

Start-Sleep -Seconds 5

# Start Rust server as background job
Write-Host "`n[3/4] Starting API server on port $Port..." -ForegroundColor Yellow
$ServerJob = Start-Job -Name "e2e-server" -ScriptBlock {
  param($exe, $port, $dbUrl, $authDbUrl, $storeDir, $dataDir)
  $env:DATABASE_URL = $dbUrl
  $env:AUTH_DATABASE_URL = $authDbUrl
  $env:SERVER_PORT = $port
  $env:STORAGE_DIR = $storeDir
  $env:DATA_DIR = $dataDir
  & $exe "--port" $port
} -ArgumentList $ServerBin, $Port, $env:DATABASE_URL, $env:AUTH_DATABASE_URL, $env:STORAGE_DIR, $env:DATA_DIR

Start-Sleep -Seconds 4

# Health check
Write-Host "Checking server health..." -ForegroundColor Gray
$healthy = $false
for ($i = 0; $i -lt 15; $i++) {
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
  Get-Job -Name "e2e-*" | Stop-Job -PassThru | Remove-Job
  exit 1
}

Write-Host "Server is healthy!" -ForegroundColor Green

# Also check Vite
try {
  $viteRes = Invoke-WebRequest -Uri "http://localhost:1420" -UseBasicParsing -TimeoutSec 3
  Write-Host "Vite is up! ($($viteRes.StatusCode))" -ForegroundColor Green
} catch {
  Write-Host "Warning: Vite may not be ready yet: $_" -ForegroundColor Yellow
}

# Run Playwright tests
Write-Host "`n[4/4] Running Playwright tests..." -ForegroundColor Yellow
Set-Location -Path $RootDir
npx playwright test --config e2e/playwright.config.ts
$exitCode = $LASTEXITCODE

# Cleanup
if (-not $KeepOpen) {
  Write-Host "`nCleaning up..." -ForegroundColor Gray
  Get-Job -Name "e2e-*" | Stop-Job -PassThru | Remove-Job
  Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "`n=== E2E Tests Complete (exit code: $exitCode) ===" -ForegroundColor Cyan
exit $exitCode
