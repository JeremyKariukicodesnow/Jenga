param([Parameter(Mandatory)][ValidateSet('major','minor','patch')][string]$Part)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$path = Join-Path $repoRoot 'Cargo.toml'
$text = Get-Content -LiteralPath $path -Raw
$match = [regex]::Match($text, '(?m)^version = "(\d+)\.(\d+)\.(\d+)"\r?$')
if (-not $match.Success) {throw 'Workspace version not found'}
$major = [int]$match.Groups[1].Value; $minor = [int]$match.Groups[2].Value; $patch = [int]$match.Groups[3].Value
switch ($Part) {'major' {$major++;$minor=0;$patch=0} 'minor' {$minor++;$patch=0} 'patch' {$patch++}}
$version = "$major.$minor.$patch"
$text = $text.Remove($match.Index,$match.Length).Insert($match.Index,"version = `"$version`"")
Set-Content -LiteralPath $path -Value $text -NoNewline
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Push-Location $repoRoot
try { & cargo update --workspace; if ($LASTEXITCODE -ne 0) {throw 'Cargo lockfile update failed'} } finally {Pop-Location}
Write-Output "Version is now $version. Update CHANGELOG.md before building the release."
