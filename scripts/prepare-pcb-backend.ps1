param([string]$KiCadRoot = 'C:\Program Files\KiCad\10.0')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $repoRoot 'resources\backend\kicad'
$cli = Join-Path $KiCadRoot 'bin\kicad-cli.exe'
if (-not (Test-Path -LiteralPath $cli)) {throw 'KiCad 10 native backend not found'}
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$binaryDir = Join-Path $destination 'bin'
New-Item -ItemType Directory -Force -Path $binaryDir | Out-Null
Copy-Item -LiteralPath $cli -Destination $binaryDir -Force
Get-ChildItem -LiteralPath (Join-Path $KiCadRoot 'bin') -File -Filter '*.dll' | ForEach-Object {Copy-Item -LiteralPath $_.FullName -Destination $binaryDir -Force}
Copy-Item -LiteralPath (Join-Path $KiCadRoot 'COPYRIGHT.txt') -Destination $destination -Force
$backendVersion = & $cli version
@"
Native backend: unmodified official KiCad $backendVersion Windows CLI and runtime DLLs.
Separate process backend; native files are the interchange boundary.
Licenses and third-party attribution: COPYRIGHT.txt.
Corresponding KiCad source: https://gitlab.com/kicad/code/kicad/-/tree/$backendVersion
Windows dependency build recipes: https://gitlab.com/kicad/packaging/kicad-win-builder
OpenCascade source: https://github.com/Open-Cascade-SAS/OCCT
wxWidgets source: https://github.com/wxWidgets/wxWidgets
Official component 3D libraries are included separately in libraries/3dmodels under the KiCad library license and exception; see libraries/licenses and manifest.json.
"@ | Set-Content -LiteralPath (Join-Path $destination 'SOURCE_AND_LICENSES.txt') -Encoding utf8
New-Item -ItemType Directory -Force -Path (Join-Path $destination 'share\kicad') | Out-Null
Copy-Item -LiteralPath (Join-Path $KiCadRoot 'share\kicad\schemas') -Destination (Join-Path $destination 'share\kicad') -Recurse -Force
& (Join-Path $binaryDir 'kicad-cli.exe') version
if ($LASTEXITCODE -ne 0) {throw 'Bundled backend startup failed'}
