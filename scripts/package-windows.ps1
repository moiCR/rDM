param (
    [string]$Version = "1.0.0"
)

$ErrorActionPreference = "Stop"

$rootDir = Split-Path -Parent $PSScriptRoot
Set-Location $rootDir

Write-Host "==> Compilando rDM en modo Release..." -ForegroundColor Cyan
cargo build --release --locked --workspace

$distDir = Join-Path $rootDir "dist"
if (Test-Path $distDir) {
    Remove-Item -Recurse -Force $distDir
}
New-Item -ItemType Directory -Path $distDir | Out-Null

$portableDir = Join-Path $distDir "rDM-v$Version-portable-x64"
New-Item -ItemType Directory -Path $portableDir | Out-Null

Copy-Item (Join-Path $rootDir "target\release\rDM.exe") $portableDir
Copy-Item (Join-Path $rootDir "target\release\rdm-cli.exe") $portableDir
Copy-Item -Recurse (Join-Path $rootDir "crates\app\public") (Join-Path $portableDir "public")

$zipPath = Join-Path $distDir "rDM-v$Version-portable-x64.zip"
Write-Host "==> Creando paquete portable: $zipPath" -ForegroundColor Cyan
Compress-Archive -Path "$portableDir\*" -DestinationPath $zipPath -Force

$isccCandidates = @(
    "ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe"
)

$isccPath = $null
foreach ($candidate in $isccCandidates) {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) {
        $isccPath = (Get-Command $candidate).Source
        break
    }
    if (Test-Path $candidate) {
        $isccPath = $candidate
        break
    }
}

if ($isccPath) {
    Write-Host "==> Compilando instalador Inno Setup con $isccPath..." -ForegroundColor Cyan
    $issFile = Join-Path $rootDir "installer\rDM.iss"
    & $isccPath "/DMyAppVersion=$Version" $issFile
    Write-Host "==> Instalador generado exitosamente en dist/" -ForegroundColor Green
} else {
    Write-Host "==> Inno Setup (ISCC.exe) no encontrado en el sistema." -ForegroundColor Yellow
    Write-Host "    Para generar el instalador .exe, instala Inno Setup 6: winget install JRSoftware.InnoSetup" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "==> Archivos generados en dist/:" -ForegroundColor Green
Get-ChildItem $distDir | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize
