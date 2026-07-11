[CmdletBinding()]
param(
    [string]$GamePath = 'C:\Program Files (x86)\Against Rome',
    [string]$OriginalPath = ''
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($OriginalPath)) {
    $repositoryRoot = Split-Path -Parent $PSScriptRoot
    $originalCandidates = @(
        Get-ChildItem -LiteralPath $repositoryRoot -Directory |
            Where-Object {
                (Test-Path -LiteralPath (Join-Path $_.FullName 'Against_Rome.exe') -PathType Leaf) -and
                (Test-Path -LiteralPath (Join-Path $_.FullName 'ToEng') -PathType Container)
            }
    )
    if ($originalCandidates.Count -ne 1) {
        throw "Could not uniquely locate the original game directory under: $repositoryRoot"
    }
    $OriginalPath = $originalCandidates[0].FullName
}

function Get-NormalizedRoot([string]$Path) {
    return [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
}

$gameRoot = Get-NormalizedRoot $GamePath
$originalRoot = Get-NormalizedRoot $OriginalPath
$overlayRoot = Join-Path $gameRoot 'ToEng'
$backupRoot = Join-Path $gameRoot '.against-rome-modifier-language-backup'
$manifestPath = Join-Path $backupRoot 'manifest.json'

if (-not (Test-Path -LiteralPath (Join-Path $gameRoot 'Against_Rome.exe') -PathType Leaf)) {
    throw "Invalid game directory: $gameRoot"
}
if (-not (Test-Path -LiteralPath (Join-Path $originalRoot 'Against_Rome.exe') -PathType Leaf)) {
    throw "Invalid original game directory: $originalRoot"
}
if (Test-Path -LiteralPath $backupRoot) {
    if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
        throw "A managed language backup already exists: $backupRoot"
    }
    throw "An incomplete managed language backup directory exists: $backupRoot"
}

$overlayFiles = @(Get-ChildItem -LiteralPath $overlayRoot -File -Recurse)
if ($overlayFiles.Count -eq 0) {
    throw "No language overlay files were found: $overlayRoot"
}

$existingFiles = @()
foreach ($overlayFile in $overlayFiles) {
    $relativePath = $overlayFile.FullName.Substring($overlayRoot.Length + 1)
    $originalFile = Join-Path $originalRoot $relativePath
    $activeFile = Join-Path $gameRoot $relativePath

    if (-not (Test-Path -LiteralPath $originalFile -PathType Leaf)) {
        throw "The original source is incomplete; missing: $relativePath"
    }
    if (-not (Test-Path -LiteralPath $activeFile -PathType Leaf)) {
        throw "The active language overlay is incomplete; missing: $relativePath"
    }

    $overlayHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $overlayFile.FullName).Hash
    $activeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $activeFile).Hash
    $originalHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $originalFile).Hash
    if ($activeHash -ne $overlayHash) {
        throw "The active file does not match the ToEng overlay: $relativePath"
    }
    if ($originalHash -eq $overlayHash) {
        throw "The proposed original file is identical to the overlay: $relativePath"
    }

    $existingFiles += $relativePath.Replace('\', '/')
}

$temporaryRoot = $backupRoot + '.tmp-' + [Guid]::NewGuid().ToString('N')
try {
    $filesRoot = Join-Path $temporaryRoot 'files'
    [IO.Directory]::CreateDirectory($filesRoot) | Out-Null

    foreach ($relativePath in $existingFiles) {
        $windowsPath = $relativePath.Replace('/', '\')
        $source = Join-Path $originalRoot $windowsPath
        $destination = Join-Path $filesRoot $windowsPath
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination)) | Out-Null
        [IO.File]::Copy($source, $destination, $false)
    }

    $manifest = [ordered]@{
        ExistingFiles = $existingFiles
        MissingFiles = @()
    }
    $json = $manifest | ConvertTo-Json -Depth 4
    $utf8WithoutBom = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $temporaryRoot 'manifest.json'), $json, $utf8WithoutBom)
    [IO.Directory]::Move($temporaryRoot, $backupRoot)
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        $resolvedTemporaryRoot = Get-NormalizedRoot $temporaryRoot
        if (-not $resolvedTemporaryRoot.StartsWith($gameRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to clean an unsafe temporary path: $resolvedTemporaryRoot"
        }
        [IO.Directory]::Delete($resolvedTemporaryRoot, $true)
    }
}

Write-Host "Language baseline repaired successfully."
Write-Host "Backup: $backupRoot"
Write-Host "Existing files: $($existingFiles.Count)"
Write-Host "Missing files: 0"
