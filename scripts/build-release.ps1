param([string]$IsccPath = '', [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot
$manifestText = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
$version = [regex]::Match($manifestText, '(?m)^version = "(\d+\.\d+\.\d+)"\r?$').Groups[1].Value
if (-not $version) { throw 'Missing workspace semantic version' }
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
if (-not $SkipBuild) {
    & cargo test --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed' }
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release compilation failed' }
}
foreach ($kind in @('symbols','footprints','licenses')) {
    if (-not (Test-Path -LiteralPath "resources\libraries\$kind")) { throw "Missing bundled $kind; run scripts/prepare-libraries.ps1 first" }
}
$releaseDir = Join-Path $repoRoot "dist\$version"
$payload = Join-Path $releaseDir 'Jenga'
New-Item -ItemType Directory -Force -Path $payload | Out-Null
Copy-Item -LiteralPath 'target\release\Jenga.exe' -Destination $payload
Copy-Item -LiteralPath 'target\release\jenga-cli.exe' -Destination $payload
foreach ($file in @('LICENSE','README.md','THIRD_PARTY_NOTICES.md','CHANGELOG.md')) {Copy-Item -LiteralPath $file -Destination $payload}
Copy-Item -LiteralPath 'docs\USER_GUIDE.md' -Destination $payload
Copy-Item -LiteralPath 'docs\AUTOMATION.md' -Destination $payload
foreach ($asset in @('assistant-skill','examples','fonts','branding')) {
    $assetRoot = if ($asset -in @('assistant-skill','examples')) {'docs'} else {'resources'}
    & robocopy "$assetRoot\$asset" "$payload\$asset" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "Payload copy failed: $asset" }
}
Copy-Item -LiteralPath 'resources\DEPENDENCIES.txt' -Destination $payload
foreach ($kind in @('symbols','footprints','licenses')) {
    & robocopy "resources\libraries\$kind" "$payload\libraries\$kind" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "Payload copy failed: $kind" }
}
foreach ($file in @('manifest.json','SHA256SUMS')) {Copy-Item -LiteralPath "resources\libraries\$file" -Destination "$payload\libraries"}
# Windows CRT is linked statically through .cargo/config.toml, so no runtime installer is required.
$portable = Join-Path $releaseDir "Jenga-$version-windows-x64-portable.zip"
& tar -a -c -f $portable -C $releaseDir Jenga
if ($LASTEXITCODE -ne 0) {throw 'Portable archive failed'}
if (-not $IsccPath) {
    $candidates = @((Join-Path $repoRoot 'tools\inno\ISCC.exe'), 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe', 'C:\Program Files\Inno Setup 7\ISCC.exe')
    $IsccPath = $candidates | Where-Object {Test-Path -LiteralPath $_} | Select-Object -First 1
}
if (-not $IsccPath) { throw "Portable ZIP built. Install Inno Setup or pass -IsccPath to also create Setup.exe." }
& $IsccPath '/Q' "/DAppVersion=$version" "/DPayloadDir=$payload" "/DOutputDir=$releaseDir" 'packaging\windows\jenga.iss'
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
$sourceDir = Join-Path $repoRoot ".cache\source-release-$version"
New-Item -ItemType Directory -Force -Path $sourceDir | Out-Null
foreach ($name in @('crates','scripts','packaging','docs','.cargo','.github','resources')) {
    # Resource binaries are distributed in the application package; source includes their provenance/licenses.
    if ($name -eq 'resources') {
        New-Item -ItemType Directory -Force -Path "$sourceDir\resources\libraries" | Out-Null
        Copy-Item -LiteralPath 'resources\libraries\licenses' -Destination "$sourceDir\resources\libraries" -Recurse -Force
        foreach ($file in @('manifest.json','SHA256SUMS')) {Copy-Item -LiteralPath "resources\libraries\$file" -Destination "$sourceDir\resources\libraries" -Force}
        Copy-Item -LiteralPath 'resources\fonts' -Destination "$sourceDir\resources" -Recurse -Force
        Copy-Item -LiteralPath 'resources\branding' -Destination "$sourceDir\resources" -Recurse -Force
        Copy-Item -LiteralPath 'resources\DEPENDENCIES.txt' -Destination "$sourceDir\resources" -Force
    } else {Copy-Item -LiteralPath $name -Destination $sourceDir -Recurse -Force}
}
foreach ($file in @('Cargo.toml','Cargo.lock','LICENSE','README.md','THIRD_PARTY_NOTICES.md','CHANGELOG.md','.gitignore')) {Copy-Item -LiteralPath $file -Destination $sourceDir -Force}
$sourceZip = Join-Path $releaseDir "Jenga-$version-source.zip"
if (-not (Test-Path -LiteralPath "$sourceDir\vendor")) {
    if (Test-Path -LiteralPath "$repoRoot\.cache\source-release\vendor") {
        & robocopy "$repoRoot\.cache\source-release\vendor" "$sourceDir\vendor" /E /NFL /NDL /NJH /NJS /NP | Out-Null
        if ($LASTEXITCODE -gt 7) {throw 'Dependency source copy failed'}
    }
}
& {
    & cargo vendor --locked "$sourceDir\vendor" 1> "$repoRoot\.cache\vendor-config.toml"
    if ($LASTEXITCODE -ne 0) {throw 'Dependency source vendoring failed'}
}
if (-not (Select-String -LiteralPath "$sourceDir\.cargo\config.toml" -Pattern '^\[source\.vendored-sources\]' -Quiet)) {
Add-Content -LiteralPath "$sourceDir\.cargo\config.toml" -Value @'

[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
'@
}
& tar -a -c -f $sourceZip -C $sourceDir .
if ($LASTEXITCODE -ne 0) {throw 'Source archive failed'}
$releaseFiles = Get-ChildItem -LiteralPath $releaseDir -File | Where-Object Extension -In '.exe','.zip'
$releaseFiles | ForEach-Object {"$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)"} | Set-Content -LiteralPath (Join-Path $releaseDir 'SHA256SUMS')
Write-Output "Release saved in $releaseDir"
