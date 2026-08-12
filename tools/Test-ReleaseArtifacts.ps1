param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactDirectory,

    [Parameter(Mandatory = $true)]
    [string]$ArchivePath,

    [Parameter(Mandatory = $true)]
    [string]$AssemblyPath,

    [Parameter(Mandatory = $true)]
    [string]$ExpectedVersion
)

$ErrorActionPreference = 'Stop'

function Assert-SafeReleasePath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $normalized = $Path.Replace('\', '/').TrimStart('/')
    $leaf = [IO.Path]::GetFileName($normalized)
    $forbiddenLeafNames = @(
        'Backup.zip',
        'Against_Rome.exe',
        'GameAssembly.dll',
        'global-metadata.dat',
        'SaveData'
    )
    $forbiddenTopLevelDirectories = @('MAPS', 'SYSTEM', 'SAVE', 'ToEng', 'Original game archives')

    if ($forbiddenLeafNames -contains $leaf) {
        throw "Forbidden release file: $Path"
    }

    $segments = $normalized.Split('/', [StringSplitOptions]::RemoveEmptyEntries)
    if ($segments | Where-Object { $forbiddenTopLevelDirectories -contains $_ }) {
        throw "Forbidden release directory: $Path"
    }

    if ($leaf -match '(?i)^(SaveData.*|dump\.cs|il2cpp\.h|script\.json)$' -or
        $leaf -match '(?i)\.(pdb|dmp|dump)$') {
        throw "Forbidden release content: $Path"
    }
}

$artifactRoot = (Resolve-Path -LiteralPath $ArtifactDirectory).Path
$archive = (Resolve-Path -LiteralPath $ArchivePath).Path
$assembly = (Resolve-Path -LiteralPath $AssemblyPath).Path

Get-ChildItem -LiteralPath $artifactRoot -File -Recurse | ForEach-Object {
    if ($_.FullName -ne $archive) {
        $relative = $_.FullName.Substring($artifactRoot.Length).TrimStart([char[]]@('\', '/'))
        Assert-SafeReleasePath -Path $relative
    }
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    foreach ($entry in $zip.Entries) {
        Assert-SafeReleasePath -Path $entry.FullName
    }
}
finally {
    $zip.Dispose()
}

$loadedAssembly = [Reflection.Assembly]::LoadFile($assembly)
$backupResource = $loadedAssembly.GetManifestResourceNames() |
    Where-Object { $_.EndsWith('Backup.zip', [StringComparison]::OrdinalIgnoreCase) }
if ($backupResource) {
    throw "Proprietary Backup.zip is embedded in $AssemblyPath as $backupResource"
}

$expectedFourPartVersion = "$ExpectedVersion.0"
$assemblyVersion = [Reflection.AssemblyName]::GetAssemblyName($assembly).Version.ToString()
$fileInfo = [Diagnostics.FileVersionInfo]::GetVersionInfo($assembly)
if ($assemblyVersion -ne $expectedFourPartVersion) {
    throw "AssemblyVersion mismatch: expected $expectedFourPartVersion, got $assemblyVersion"
}
if ($fileInfo.FileVersion -ne $expectedFourPartVersion) {
    throw "FileVersion mismatch: expected $expectedFourPartVersion, got $($fileInfo.FileVersion)"
}
if ($fileInfo.ProductVersion -ne $ExpectedVersion) {
    throw "ProductVersion mismatch: expected $ExpectedVersion, got $($fileInfo.ProductVersion)"
}

Write-Host "Release audit passed: no proprietary backup content; version $ExpectedVersion is consistent."
