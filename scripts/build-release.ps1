param([string]$IsccPath = '', [switch]$SkipBuild, [switch]$LinkLibraries, [switch]$ReuseLibraries, [switch]$ReusePortable)
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
foreach ($kind in @('symbols','footprints','3dmodels','licenses')) {
    if (-not (Test-Path -LiteralPath "resources\libraries\$kind")) { throw "Missing bundled $kind; run scripts/prepare-libraries.ps1 first" }
}
$releaseDir = Join-Path $repoRoot "dist\$version"
$payload = Join-Path $releaseDir 'Jenga'
New-Item -ItemType Directory -Force -Path $payload | Out-Null
Copy-Item -LiteralPath 'target\release\Jenga.exe' -Destination $payload
Copy-Item -LiteralPath 'target\release\jenga-cli.exe' -Destination $payload
Copy-Item -LiteralPath 'target\release\jenga-connect.exe' -Destination $payload
foreach ($file in @('LICENSE','README.md','THIRD_PARTY_NOTICES.md','CHANGELOG.md')) {Copy-Item -LiteralPath $file -Destination $payload}
Copy-Item -LiteralPath 'docs\USER_GUIDE.md' -Destination $payload
Copy-Item -LiteralPath 'docs\AUTOMATION.md' -Destination $payload
foreach ($asset in @('assistant-skill','examples','fonts','branding')) {
    $assetRoot = if ($asset -in @('assistant-skill','examples')) {'docs'} else {'resources'}
    & robocopy "$assetRoot\$asset" "$payload\$asset" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "Payload copy failed: $asset" }
}
if (-not (Test-Path -LiteralPath 'resources\backend\kicad\bin\kicad-cli.exe')) {
    & (Join-Path $PSScriptRoot 'prepare-pcb-backend.ps1')
}
& (Join-Path $PSScriptRoot 'verify-native-notices.ps1')
foreach ($sub in @('bin','share')) {
    & robocopy "resources\backend\kicad\$sub" "$payload\backend\kicad\$sub" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -gt 7) {throw 'Native backend payload copy failed'}
}
foreach ($file in @('COPYRIGHT.txt','SOURCE_AND_LICENSES.txt','NOTICES-SOURCES.zip')) {Copy-Item -LiteralPath "resources\backend\kicad\$file" -Destination "$payload\backend\kicad"}
Copy-Item -LiteralPath 'docs\PCB_GUIDE.md' -Destination $payload
Copy-Item -LiteralPath 'resources\DEPENDENCIES.txt' -Destination $payload
if ($LinkLibraries -and -not ('JengaPackaging.NativeFiles' -as [type])) {
    Add-Type -Namespace JengaPackaging -Name NativeFiles -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern bool CreateHardLink(string newFileName, string existingFileName, System.IntPtr securityAttributes);
'@
}
$reuseLibraryPayload = $false
if ($ReuseLibraries) {
    # Opt-in for rebuilding code against the same immutable, already verified
    # library payload. Updating library data requires prepare-libraries first.
    foreach ($file in @('manifest.json','SHA256SUMS')) {
        $previous = Join-Path $payload "libraries\$file"
        if (-not (Test-Path -LiteralPath $previous) -or (Get-FileHash -LiteralPath $previous).Hash -ne (Get-FileHash -LiteralPath "resources\libraries\$file").Hash) {
            throw 'Library payload has changed; rebuild without -ReuseLibraries'
        }
    }
    $reuseLibraryPayload = $true
}
foreach ($kind in @('symbols','footprints','3dmodels','licenses')) {
    Write-Output "Bundling $kind"
    if ($reuseLibraryPayload) {
        if (-not (Test-Path -LiteralPath "$payload\libraries\$kind")) {throw "Missing cached library payload: $kind"}
        continue
    }
    if ($LinkLibraries) {
        # Same-volume hard links conserve build disk space. Archives and installed
        # files are ordinary independent files. Treat library inputs as immutable.
        $inputRoot = [IO.Path]::GetFullPath("resources\libraries\$kind")
        $outputRoot = Join-Path $payload "libraries\$kind"
        foreach ($entry in Get-ChildItem -LiteralPath $inputRoot -Recurse -File) {
            $destination = Join-Path $outputRoot ($entry.FullName.Substring($inputRoot.Length).TrimStart([char]'\', [char]'/'))
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination)) | Out-Null
            if (-not (Test-Path -LiteralPath $destination)) {
                if (-not [JengaPackaging.NativeFiles]::CreateHardLink($destination, $entry.FullName, [IntPtr]::Zero)) {
                    throw "Unable to link library file: $destination (Windows error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
                }
            } elseif ((Get-FileHash -LiteralPath $destination).Hash -ne (Get-FileHash -LiteralPath $entry.FullName).Hash) {
                throw "Payload contains stale library file: $destination"
            }
        }
    } else {
        & robocopy "resources\libraries\$kind" "$payload\libraries\$kind" /E /NFL /NDL /NJH /NJS /NP | Out-Null
        if ($LASTEXITCODE -gt 7) { throw "Payload copy failed: $kind" }
    }
}
foreach ($file in @('manifest.json','SHA256SUMS')) {Copy-Item -LiteralPath "resources\libraries\$file" -Destination "$payload\libraries"}
# Windows CRT is linked statically through .cargo/config.toml, so no runtime installer is required.
$portable = Join-Path $releaseDir "Jenga-$version-windows-x64-portable.zip"
if ($ReusePortable) {
    # Explicit opt-in after verifying this archive matches the unchanged payload.
    if (-not (Test-Path -LiteralPath $portable)) { throw 'No portable archive to reuse' }
} else {
    & tar -a -c -f $portable -C $releaseDir Jenga
    if ($LASTEXITCODE -ne 0) {throw 'Portable archive failed'}
}
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
        New-Item -ItemType Directory -Force -Path "$sourceDir\resources\backend\kicad" | Out-Null
        foreach ($notice in @('COPYRIGHT.txt','SOURCE_AND_LICENSES.txt','NOTICES-SOURCES.zip')) {Copy-Item -LiteralPath "resources\backend\kicad\$notice" -Destination "$sourceDir\resources\backend\kicad" -Force}
    } else {
        # Publish repository sources only. Recursive directory copies also pick
        # up ignored local environment files and deployment credentials.
        $tracked = & git -c core.quotepath=false ls-files -- $name
        if ($LASTEXITCODE -ne 0) {throw 'Could not enumerate tracked release sources'}
        foreach ($relative in $tracked) {
            $destination = Join-Path $sourceDir $relative
            New-Item -ItemType Directory -Force -Path (Split-Path $destination -Parent) | Out-Null
            Copy-Item -LiteralPath $relative -Destination $destination -Force
        }
    }
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
& (Join-Path $PSScriptRoot 'prepare-download-aliases.ps1') -ReleaseDirectory $releaseDir -Version $version
Write-Output "Release saved in $releaseDir"
