param([string]$ReleaseDirectory = '', [string]$Version = '')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
if (-not $Version) {
    $manifest = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
    $Version = [regex]::Match($manifest, '(?m)^version = "(\d+\.\d+\.\d+)"\r?$').Groups[1].Value
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a numeric release version' }
if (-not $ReleaseDirectory) { $ReleaseDirectory = Join-Path $repoRoot "dist/$Version" }
$releaseRoot = [IO.Path]::GetFullPath($ReleaseDirectory)
$aliases = [ordered]@{
    "Jenga-$Version-windows-x64-Setup.exe" = 'Jenga-windows-x64-Setup.exe'
    "Jenga-$Version-windows-x64-portable.zip" = 'Jenga-windows-x64-portable.zip'
    "Jenga-$Version-source.zip" = 'Jenga-source.zip'
}
# Validate all inputs before writing any aliases. Never publish or change a tag here.
foreach ($name in $aliases.Keys) {
    if (-not (Test-Path -LiteralPath (Join-Path $releaseRoot $name) -PathType Leaf)) {
        throw "Missing release input: $name"
    }
}
foreach ($name in $aliases.Keys) {
    $source = Join-Path $releaseRoot $name
    $destination = Join-Path $releaseRoot $aliases[$name]
    $expected = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    if (Test-Path -LiteralPath $destination) {
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $expected) {
            throw "Stale download alias: $($aliases[$name]). Refusing to replace a possibly published artifact. Use a fresh release directory."
        }
    } else {
        try {
            New-Item -ItemType HardLink -Path $destination -Value $source -ErrorAction Stop | Out-Null
        } catch {
            # Non-NTFS volumes may not support links; copies need extra disk space.
            Copy-Item -LiteralPath $source -Destination $destination -ErrorAction Stop
        }
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $expected) {
            throw "Download alias verification failed: $($aliases[$name])"
        }
    }
}
$releaseFiles = Get-ChildItem -LiteralPath $releaseRoot -File | Where-Object Extension -In '.exe','.zip' | Sort-Object Name
$releaseFiles | ForEach-Object {
    "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)"
} | Set-Content -LiteralPath (Join-Path $releaseRoot 'SHA256SUMS') -Encoding utf8
Write-Output 'Download aliases verified; checksums include both canonical and versioned names. Nothing was uploaded or published.'
