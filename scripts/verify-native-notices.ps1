param(
    [string]$BackendRoot = (Join-Path $PSScriptRoot '../resources/backend/kicad'),
    [string]$NoticeArchive = (Join-Path $PSScriptRoot '../resources/backend/kicad/NOTICES-SOURCES.zip'),
    [switch]$MetadataOnly
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$backend = [IO.Path]::GetFullPath($BackendRoot)
$zip = [IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($NoticeArchive))
try {
    if ($zip.Entries.Count -gt 5000 -or ($zip.Entries | Measure-Object Length -Sum).Sum -gt 33554432) { throw 'Native notice archive exceeds metadata limits' }
    $catalog = $zip.GetEntry('NATIVE_COMPONENTS.json')
    if (-not $catalog -or $catalog.Length -gt 2097152) { throw 'Missing or oversized NATIVE_COMPONENTS.json' }
    $reader = [IO.StreamReader]::new($catalog.Open())
    try { $meta = $reader.ReadToEnd() | ConvertFrom-Json } finally { $reader.Dispose() }
    if ($meta.schema_version -ne 2 -or @($meta.files).Count -eq 0) { throw 'Invalid native component catalog' }
    $entries = @($zip.Entries | ForEach-Object FullName)
    $names = @{}
    foreach ($item in $meta.files) {
        if ($item.file -notmatch '^bin/[A-Za-z0-9_.-]+\.(dll|exe)$' -or $item.sha256 -notmatch '^[0-9a-f]{64}$' -or $names.ContainsKey($item.file)) { throw "Invalid or duplicate native file record: $($item.file)" }
        $names[$item.file] = $true
        $componentProperty = $meta.components.PSObject.Properties[$item.component]
        if (-not $componentProperty) { throw "Missing component for $($item.file): $($item.component)" }
        $component = $componentProperty.Value
        $notice = "$($component.notices)"
        if (-not $component.license -or -not $notice.StartsWith('notices/') -or -not ($entries | Where-Object { $_ -eq $notice -or $_.StartsWith($notice.TrimEnd('/') + '/') })) { throw "Missing license material for $($item.file): $($item.component)" }
        if (-not $MetadataOnly) {
            $path = Join-Path $backend $item.file
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing native runtime file: $($item.file)" }
            if ((Get-Item -LiteralPath $path).Length -ne $item.size_bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $item.sha256) { throw "Native runtime hash mismatch: $($item.file); collect matching sources/notices before packaging" }
        }
    }
    if (-not $MetadataOnly) {
        foreach ($file in Get-ChildItem -LiteralPath (Join-Path $backend 'bin') -File | Where-Object Extension -In '.dll','.exe') {
            if (-not $names.ContainsKey("bin/$($file.Name)")) { throw "Unmapped native runtime file: bin/$($file.Name)" }
        }
    }
    $scope = if ($MetadataOnly) { 'metadata and notice coverage only; runtime bytes not checked' } else { 'runtime hashes and notice coverage' }
    Write-Output "Native release verification passed: $(@($meta.files).Count) files; $scope. Not a legal/security audit."
} finally { $zip.Dispose() }
