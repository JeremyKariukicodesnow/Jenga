$ErrorActionPreference = 'Stop'
$fixture = Join-Path (Split-Path $PSScriptRoot -Parent) ('.cache/download-alias-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$version = '0.8.0'
foreach ($name in @("Jenga-$version-windows-x64-Setup.exe", "Jenga-$version-windows-x64-portable.zip", "Jenga-$version-source.zip")) {
    Set-Content -LiteralPath (Join-Path $fixture $name) -Value "Original fixture $name" -Encoding utf8
}
$helper = Join-Path $PSScriptRoot 'prepare-download-aliases.ps1'
& $helper -ReleaseDirectory $fixture -Version $version
& $helper -ReleaseDirectory $fixture -Version $version
foreach ($pair in @(
    @("Jenga-$version-windows-x64-Setup.exe", 'Jenga-windows-x64-Setup.exe'),
    @("Jenga-$version-windows-x64-portable.zip", 'Jenga-windows-x64-portable.zip'),
    @("Jenga-$version-source.zip", 'Jenga-source.zip')
)) {
    if ((Get-FileHash -LiteralPath (Join-Path $fixture $pair[0])).Hash -ne (Get-FileHash -LiteralPath (Join-Path $fixture $pair[1])).Hash) { throw 'Alias differs from original' }
}
if (@(Get-Content -LiteralPath (Join-Path $fixture 'SHA256SUMS')).Count -ne 6) { throw 'Missing checksum entries' }
$source = Join-Path $fixture "Jenga-$version-windows-x64-Setup.exe"
$before = (Get-FileHash -LiteralPath $source).Hash
$alias = Join-Path $fixture 'Jenga-windows-x64-Setup.exe'
# Remove this exact owned fixture link before replacing it; never mutate a linked original.
Remove-Item -LiteralPath $alias
Set-Content -LiteralPath $alias -Value 'Stale fixture alias' -Encoding utf8
$rejected = $false
try { & $helper -ReleaseDirectory $fixture -Version $version } catch {
    if ($_.Exception.Message -notlike 'Stale download alias:*') { throw }
    $rejected = $true
}
if (-not $rejected -or (Get-FileHash -LiteralPath $source).Hash -ne $before) { throw 'Stale alias guard failed' }
Write-Output 'Download alias tests passed: byte identity, repeatability, checksums and stale-alias rejection.'
