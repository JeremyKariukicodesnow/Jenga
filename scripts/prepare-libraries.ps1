param([string]$KiCadRoot = 'C:\Program Files\KiCad\10.0')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$libraryRoot = Join-Path $repoRoot 'resources\libraries'
New-Item -ItemType Directory -Force -Path $libraryRoot | Out-Null
$origins = @()
foreach ($kind in @('symbols', 'footprints', '3dmodels')) {
    $source = Join-Path $KiCadRoot "share\kicad\$kind"
    if (-not (Test-Path -LiteralPath $source)) { throw "KiCad $kind not found at $source" }
    $destination = Join-Path $libraryRoot $kind
    & robocopy $source $destination /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "Failed to copy $kind" }
    $files = @(Get-ChildItem -LiteralPath $destination -Recurse -File)
    $upstream = if ($kind -eq '3dmodels') {'kicad-packages3D'} else {"kicad-$kind"}
    $origins += [ordered]@{kind=$kind; upstream="https://gitlab.com/kicad/libraries/$upstream"; installation=$KiCadRoot; count=$files.Count; bytes=($files | Measure-Object Length -Sum).Sum}
}
$licenseDir = Join-Path $libraryRoot 'licenses'
New-Item -ItemType Directory -Force -Path $licenseDir | Out-Null
foreach ($kind in @('symbols', 'footprints', '3dmodels')) {
    $upstream = if ($kind -eq '3dmodels') {'kicad-packages3D'} else {"kicad-$kind"}
    Invoke-WebRequest -Uri "https://raw.githubusercontent.com/KiCad/$upstream/master/LICENSE.md" -OutFile (Join-Path $licenseDir "KiCad-$kind-LICENSE.md")
}
Invoke-WebRequest -Uri 'https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt' -OutFile (Join-Path $licenseDir 'CC-BY-SA-4.0.txt')
$manifest = [ordered]@{schema_version=1; license='CC-BY-SA-4.0 WITH KiCad-libraries-exception'; models_included=$true; sources=$origins}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $libraryRoot 'manifest.json') -Encoding utf8
# Pin exact local resource contents independently of the installed KiCad version.
$hashes = foreach ($kind in @('symbols','footprints','3dmodels')) {
    Get-ChildItem -LiteralPath (Join-Path $libraryRoot $kind) -Recurse -File | Sort-Object FullName | ForEach-Object {
        $relative = [IO.Path]::GetRelativePath($libraryRoot, $_.FullName).Replace('\','/')
        "$( (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() )  $relative"
    }
}
$hashes | Set-Content -LiteralPath (Join-Path $libraryRoot 'SHA256SUMS') -Encoding utf8
Write-Output 'Prepared KiCad symbols, footprints and the complete installed 3D model library.'
