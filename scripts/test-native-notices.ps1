$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$repo = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$work = Join-Path $repo ('.cache/native-notice-test-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory((Join-Path $work 'backend/bin')) | Out-Null
try {
    $binary = Join-Path $work 'backend/bin/fixture.dll'
    [IO.File]::WriteAllText($binary, 'fixture runtime bytes')
    $manifest = @{ schema_version = 2; components = @{ fixture = @{ license = 'MIT'; notices = 'notices/fixture/LICENSE' } }; files = @(@{ file = 'bin/fixture.dll'; component = 'fixture'; size_bytes = (Get-Item -LiteralPath $binary).Length; sha256 = (Get-FileHash -LiteralPath $binary).Hash.ToLowerInvariant() }) }
    $archive = Join-Path $work 'notices.zip'
    $zip = [IO.Compression.ZipFile]::Open($archive, [IO.Compression.ZipArchiveMode]::Create)
    foreach ($entry in @(@('NATIVE_COMPONENTS.json', ($manifest | ConvertTo-Json -Depth 8)), @('notices/fixture/LICENSE', 'Fixture license notice'))) {
        $writer = [IO.StreamWriter]::new($zip.CreateEntry($entry[0]).Open())
        try { $writer.Write($entry[1]) } finally { $writer.Dispose() }
    }
    $zip.Dispose()
    & "$PSScriptRoot/verify-native-notices.ps1" -BackendRoot "$work/backend" -NoticeArchive $archive
    [IO.File]::WriteAllText($binary, 'changed runtime bytes')
    $rejected = $false
    try { & "$PSScriptRoot/verify-native-notices.ps1" -BackendRoot "$work/backend" -NoticeArchive $archive } catch { if ($_.Exception.Message -notmatch 'hash mismatch') { throw }; $rejected = $true }
    if (-not $rejected) { throw 'Changed DLL was accepted' }
    [IO.File]::WriteAllText($binary, 'fixture runtime bytes')
    $extra = Join-Path $work 'backend/bin/unmapped.dll'
    [IO.File]::WriteAllText($extra, 'extra runtime')
    $rejected = $false
    try { & "$PSScriptRoot/verify-native-notices.ps1" -BackendRoot "$work/backend" -NoticeArchive $archive } catch { if ($_.Exception.Message -notmatch 'Unmapped native') { throw }; $rejected = $true }
    if (-not $rejected) { throw 'Unmapped DLL was accepted' }
    & "$PSScriptRoot/verify-native-notices.ps1" -MetadataOnly
    Write-Output 'Native notice tests passed: complete fixture, changed DLL rejection, unmapped DLL rejection and real notice bundle coverage.'
} finally {
    $resolved = [IO.Path]::GetFullPath($work)
    $expected = [IO.Path]::GetFullPath((Join-Path $repo '.cache')) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($expected, [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing cleanup outside workspace cache' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
