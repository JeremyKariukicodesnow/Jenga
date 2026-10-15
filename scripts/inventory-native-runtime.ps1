param(
    [string]$BackendRoot = (Join-Path $PSScriptRoot '../resources/backend/kicad'),
    [string]$OutputPath = (Join-Path $PSScriptRoot '../docs/native-runtime-inventory.json')
)
$ErrorActionPreference = 'Stop'
$backend = [IO.Path]::GetFullPath($BackendRoot)
$cli = Join-Path $backend 'bin/kicad-cli.exe'
if (-not (Test-Path -LiteralPath $cli)) { throw 'Prepare the native backend first' }
$version = & $cli version
if ($LASTEXITCODE -ne 0) { throw 'Backend version query failed' }
$entries = foreach ($file in Get-ChildItem -LiteralPath (Join-Path $backend 'bin') -File | Where-Object Extension -In '.dll','.exe' | Sort-Object Name) {
    $info = $file.VersionInfo
    [ordered]@{
        file = "bin/$($file.Name)"
        size_bytes = $file.Length
        sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        product_name = $info.ProductName
        file_version = $info.FileVersion
        company = $info.CompanyName
        embedded_copyright = $info.LegalCopyright
        license_review = 'pending'
        corresponding_source_review = 'pending'
    }
}
$inventory = [ordered]@{
    schema_version = 1
    backend_version = "$version".Trim()
    provenance = 'Copied without intentional modification from the official local KiCad Windows installation; see prepare-pcb-backend.ps1.'
    audit_status = 'Inventory only. Embedded metadata is not authoritative license evidence. Per-component licenses, redistribution rights and matching source remain pending.'
    files = @($entries)
}
$inventory | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $OutputPath -Encoding utf8
Write-Output "Inventoried $(@($entries).Count) executable/runtime files; license/source reviews remain pending."
