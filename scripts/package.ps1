param(
    [ValidatePattern('^[a-zA-Z0-9-]*$')]
    [string]$Variant = '',
    [string]$ExecutablePath = '',
    [switch]$SkipArchive
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
$variantTag = if ($Variant) { "-$Variant" } else { '' }
$bundleRoot = Join-Path $projectRoot "release\Rove-$version$variantTag-windows-x64"
New-Item -ItemType Directory -Force -Path $bundleRoot | Out-Null
$binary = if ($ExecutablePath) { (Resolve-Path -LiteralPath $ExecutablePath).Path } else { Join-Path $projectRoot 'native\target\release\rove.exe' }
Copy-Item -LiteralPath $binary -Destination (Join-Path $bundleRoot 'Rove.exe')
foreach ($name in @('LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md', 'licenses', 'docs')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $bundleRoot -Recurse -Force
}
Set-Content -LiteralPath (Join-Path $bundleRoot '运行说明.txt') -Encoding utf8 -Value @'
双击 Rove.exe 启动。
需要 Windows 10/11 和 Microsoft Edge WebView2 Evergreen Runtime。
前端资源已嵌入，不需要安装 Node.js 或 Rust 才能运行。
默认数据沿用当前用户 LocalAppData 下的 EraBrowser 目录，保留已有标签和登录状态。
这是基础开发版，功能范围与已知限制见 README.md 和 docs/VALIDATION.md。
'@
if ($SkipArchive) {
    Get-Item -LiteralPath $bundleRoot
    return
}
$archive = "$bundleRoot.zip"
# Some Cargo license files retain Unix-epoch timestamps. ZIP only supports
# 1980-2107; normalize copied payload metadata without touching source files.
Get-ChildItem -LiteralPath $bundleRoot -Recurse -Force | ForEach-Object {
    if ($_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107) {
        $_.LastWriteTime = [datetime]'1980-01-02T00:00:00'
    }
}
Compress-Archive -Path (Join-Path $bundleRoot '*') -DestinationPath $archive -Force
Get-Item -LiteralPath $archive | Select-Object FullName, Length
