#Requires -Version 5.1
<#
.SYNOPSIS
    Windows 本地构建脚本，用于生成 PHP-Stack 的测试产物。

.DESCRIPTION
    统一调用 Tauri 构建命令，产物直接落在 src-tauri/target/<profile>/ 下，
    不再做额外目录整理。直接从 target/<profile>/ 运行 app.exe 即可：
    - tauri build 会将 bundle.resources（services/ 服务模板）复制到
      target/<profile>/services/，与 exe 同级；
    - 即使 --no-bundle 构建未复制资源，程序运行时也会回退到
      src-tauri/services/ 查找模板。

    安装包（MSI + NSIS）则会将 services/ 模板释放到安装目录（exe 旁），
    程序在用户「应用配置」时再把模板释放到 workspace 的 services/ 目录，
    用户可自行修复或修改这些已释放的配置/模板文件。

.PARAMETER DebugBuild
    构建 Debug 版本（带有 DevTools，方便调试）。

.PARAMETER NoBundle
    只生成可执行文件，跳过安装包打包（等价于 tauri build --no-bundle）。

.PARAMETER SkipInstall
    跳过 npm install，直接复用已有的 node_modules。

.PARAMETER Help
    显示此帮助信息。

.EXAMPLE
    .\scripts\build.ps1
    构建 Release 安装包（MSI + NSIS），产物位于 src-tauri/target/release/ 及其 bundle/ 子目录

.EXAMPLE
    .\scripts\build.ps1 -NoBundle
    只构建 Release 可执行文件，产物位于 src-tauri/target/release/app.exe

.EXAMPLE
    .\scripts\build.ps1 -DebugBuild -NoBundle
    只构建 Debug 可执行文件，产物位于 src-tauri/target/debug/app.exe
#>
[CmdletBinding()]
param(
    [switch]$DebugBuild,
    [switch]$NoBundle,
    [switch]$SkipInstall,
    [switch]$Help
)

Set-StrictMode -Version 1.0
$ErrorActionPreference = 'Stop'

if ($Help) {
    Get-Help $PSCommandPath -Detailed
    exit 0
}

$isWin = if ($null -ne (Get-Variable -Name IsWindows -ErrorAction SilentlyContinue)) {
    $IsWindows
} else {
    $env:OS -eq 'Windows_NT'
}

if (-not $isWin) {
    Write-Warning "该脚本主要针对 Windows 平台；当前平台可能不支持部分产物。"
}

function Test-CommandAvailable {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

foreach ($cmd in @('npm', 'cargo')) {
    if (-not (Test-CommandAvailable $cmd)) {
        throw "未检测到 '$cmd'，请确认已安装 Node.js 和 Rust 工具链后再运行此脚本。"
    }
}

function Resolve-CommandPath {
    param([string]$Name)

    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $cmd) {
        return $Name
    }

    # npm 在 Windows 上同时提供了 npm.ps1 和 npm.cmd。
    # 当从函数内部使用 splatting 调用 npm.ps1 时，它会通过 $MyInvocation 重构命令行，导致参数解析错误。
    # 因此优先使用 npm.cmd。
    if ($cmd.CommandType -eq 'ExternalScript' -and $cmd.Source -match '\.ps1$') {
        $cmdWrapper = Get-Command "$Name.cmd" -ErrorAction SilentlyContinue
        if ($cmdWrapper) {
            return $cmdWrapper.Source
        }
    }

    return $cmd.Source
}

function Invoke-NativeCommand {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$FilePath,

        [string[]]$ArgumentList = @(),

        [string]$Activity
    )

    if ($Activity) {
        Write-Host ">>> $Activity" -ForegroundColor Cyan
    }

    $commandPath = Resolve-CommandPath $FilePath

    # 临时把 ErrorActionPreference 设为 Continue，避免外部命令的 stderr 被当作终止错误
    $previousEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & $commandPath @ArgumentList
    } finally {
        $ErrorActionPreference = $previousEap
    }

    if ($LASTEXITCODE -ne 0) {
        throw "命令失败: $FilePath $ArgumentList，退出码: $LASTEXITCODE"
    }
}

$root = Split-Path -Parent $PSScriptRoot
$profileDir = if ($DebugBuild) { 'debug' } else { 'release' }

Push-Location $root
try {
    if (-not $SkipInstall) {
        if (-not (Test-Path (Join-Path $root 'node_modules'))) {
            Invoke-NativeCommand -FilePath 'npm' -ArgumentList @('install') -Activity '安装 Node 依赖...'
        } else {
            Write-Host ">>> node_modules 已存在，跳过 npm install（可用 -SkipInstall 加速）" -ForegroundColor DarkGray
        }
    } else {
        Write-Host ">>> 跳过依赖安装" -ForegroundColor DarkGray
    }

    $tauriArgs = @('run', 'tauri', '--', 'build')
    if ($DebugBuild) { $tauriArgs += '--debug' }
    if ($NoBundle) { $tauriArgs += '--no-bundle' }

    $bundleHint = if ($NoBundle) { 'exe-only' } else { 'msi+nsis' }
    Invoke-NativeCommand -FilePath 'npm' -ArgumentList $tauriArgs -Activity "执行 Tauri 构建 (profile=$profileDir, bundle=$bundleHint)..."

    $targetDir = [System.IO.Path]::Combine($root, 'src-tauri', 'target', $profileDir)
    $exe = [System.IO.Path]::Combine($targetDir, 'app.exe')
    if (-not (Test-Path $exe)) {
        throw "未找到构建产物: $exe"
    }

    Write-Host "`n✅ 构建完成" -ForegroundColor Green
    Write-Host "可执行文件: $exe" -ForegroundColor DarkGray

    # tauri build（非 --no-bundle）会将 services/ 服务模板复制到 target/<profile>/ 下与 exe 同级
    $servicesDir = Join-Path $targetDir 'services'
    if (Test-Path $servicesDir) {
        Write-Host "服务模板目录: $servicesDir" -ForegroundColor DarkGray
    } else {
        Write-Warning "target/$profileDir 下未发现 services/ 模板目录（--no-bundle 构建不会复制资源）。"
        Write-Warning "程序运行时会回退到 src-tauri/services/ 查找模板，直接从 target/$profileDir/ 运行不受影响。"
    }

    if (-not $NoBundle) {
        $confPath = [System.IO.Path]::Combine($root, 'src-tauri', 'tauri.conf.json')
        $version = (Get-Content $confPath -Raw | ConvertFrom-Json).version

        $bundleDir = [System.IO.Path]::Combine($targetDir, 'bundle')
        $msiDir = Join-Path $bundleDir 'msi'
        $nsisDir = Join-Path $bundleDir 'nsis'

        $expectedFiles = @(
            [System.IO.Path]::Combine($msiDir, "php-stack_${version}_x64_en-US.msi")
            [System.IO.Path]::Combine($nsisDir, "php-stack_${version}_x64-setup.exe")
        )

        $found = $false
        foreach ($file in $expectedFiles) {
            if (Test-Path $file) {
                Write-Host "安装包: $file" -ForegroundColor DarkGray
                $found = $true
            }
        }

        if (-not $found) {
            throw "在 $bundleDir 下未找到当前版本 $version 的安装包产物（MSI/NSIS）。"
        }
    }
} finally {
    Pop-Location
}
