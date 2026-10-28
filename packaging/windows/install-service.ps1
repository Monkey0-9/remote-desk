# Remote-Desk Windows Host Service Deployment Script
# Run with Administrative Privileges

[CmdletBinding()]
param (
    [string]$InstallDir = "$env:ProgramFiles\Remote-Desk",
    [string]$ServiceName = "RemoteDeskHost",
    [switch]$Unattended = $true
)

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "     REMOTE-DESK WINDOWS HOST SERVICE INSTALLER            " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script must be executed in an elevated PowerShell session (Run as Administrator)."
    exit 1
}

# 1. Create install directory
if (-not (Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Write-Host "Created target installation directory: $InstallDir" -ForegroundColor Green
}

# 2. Copy binary
$SourceExe = "$PSScriptRoot\..\..\target\release\remote-desk-host.exe"
if (Test-Path $SourceExe) {
    Copy-Item -Path $SourceExe -Destination "$InstallDir\remote-desk-host.exe" -Force
    Write-Host "Installed remote-desk-host.exe into $InstallDir" -ForegroundColor Green
}

# 3. Register and Start Windows Service
$BinPath = "`"$InstallDir\remote-desk-host.exe`""
if ($Unattended) {
    $BinPath += " --unattended"
}

$ExistingService = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($ExistingService) {
    Write-Host "Stopping existing service $ServiceName..." -ForegroundColor Yellow
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $ServiceName | Out-Null
    Start-Sleep -Seconds 2
}

Write-Host "Creating Windows Service: $ServiceName..." -ForegroundColor Cyan
New-Service -Name $ServiceName `
            -BinaryPathName $BinPath `
            -DisplayName "Remote-Desk Streaming Host" `
            -Description "Low-latency WebRTC desktop streaming and input injection service" `
            -StartupType Automatic | Out-Null

Write-Host "Starting Remote-Desk service..." -ForegroundColor Green
Start-Service -Name $ServiceName

Write-Host "Installation complete! Service status: $((Get-Service -Name $ServiceName).Status)" -ForegroundColor Green
