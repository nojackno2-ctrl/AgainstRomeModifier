Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RustArchiveFileSha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try { Get-RustArchiveStreamSha256 $stream }
    finally { $stream.Dispose() }
}

function Get-RustArchiveStreamSha256([IO.Stream]$Stream) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { [Convert]::ToHexString($sha.ComputeHash($Stream)).ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function New-RustPortableArchive {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$SourceDirectory,
        [Parameter(Mandatory)][string]$DestinationPath,
        [Parameter(Mandatory)][string]$ChecksumPath
    )

    $source = [IO.Path]::GetFullPath($SourceDirectory)
    $archive = [IO.Path]::GetFullPath($DestinationPath)
    $checksum = [IO.Path]::GetFullPath($ChecksumPath)
    if (!(Test-Path -LiteralPath $source -PathType Container)) { throw "Archive source directory not found: $source" }
    if ([IO.Path]::GetExtension($archive) -ine '.zip') { throw "Archive destination must end in .zip: $archive" }
    if ($archive -ieq $checksum) { throw 'Archive and checksum paths must be different.' }

    $sourcePrefix = $source.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if ($archive.StartsWith($sourcePrefix, [StringComparison]::OrdinalIgnoreCase) -or
        $checksum.StartsWith($sourcePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Archive and checksum destinations must be outside the source directory.'
    }
    if (Test-Path -LiteralPath $archive) { throw "Archive already exists; refusing to overwrite: $archive" }
    if (Test-Path -LiteralPath $checksum) { throw "Checksum already exists; refusing to overwrite: $checksum" }

    $files = @(Get-ChildItem -LiteralPath $source -File -Recurse -Force | Sort-Object -Property FullName)
    if ($files.Count -eq 0) { throw "Archive source directory contains no files: $source" }

    $expected = [Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
    foreach ($file in $files) {
        if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Portable archives cannot include reparse-point files: $($file.FullName)"
        }
        $relative = [IO.Path]::GetRelativePath($source, $file.FullName).Replace('\', '/')
        if (!$relative -or $relative.StartsWith('/') -or $relative.Split('/') -contains '..') {
            throw "Unsafe archive entry path: $relative"
        }
        $expected.Add($relative, (Get-RustArchiveFileSha256 $file.FullName))
    }

    $archiveParent = Split-Path -Parent $archive
    $checksumParent = Split-Path -Parent $checksum
    if (!(Test-Path -LiteralPath $archiveParent -PathType Container)) { throw "Archive output directory not found: $archiveParent" }
    if ($archiveParent -ine $checksumParent) { throw 'Archive and checksum must be siblings in the same output directory.' }

    $tempArchive = Join-Path $archiveParent ('.rust-portable-' + [Guid]::NewGuid().ToString('N') + '.tmp.zip')
    $tempCreated = $false
    $archiveCreated = $false
    $checksumCreated = $false
    try {
        [IO.Compression.ZipFile]::CreateFromDirectory($source, $tempArchive, [IO.Compression.CompressionLevel]::Optimal, $false)
        $tempCreated = $true

        $zip = [IO.Compression.ZipFile]::OpenRead($tempArchive)
        try {
            if ($zip.Entries.Count -ne $expected.Count) {
                throw "Archive entry count mismatch: expected $($expected.Count), found $($zip.Entries.Count)."
            }
            $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach ($entry in $zip.Entries) {
                $name = $entry.FullName
                if (!$expected.ContainsKey($name) -or !$seen.Add($name)) { throw "Unexpected or duplicate archive entry: $name" }
                $entryStream = $entry.Open()
                try { $actualHash = Get-RustArchiveStreamSha256 $entryStream }
                finally { $entryStream.Dispose() }
                if ($actualHash -ne $expected[$name]) { throw "Archive content SHA256 mismatch: $name" }
            }
            if ($seen.Count -ne $expected.Count) { throw 'Archive is missing one or more source files.' }
        } finally { $zip.Dispose() }

        $archiveHash = Get-RustArchiveFileSha256 $tempArchive
        # Move fails if another process created the destination after the preflight check.
        [IO.File]::Move($tempArchive, $archive)
        $tempCreated = $false
        $archiveCreated = $true

        $line = "$archiveHash  $([IO.Path]::GetFileName($archive))`n"
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes($line)
        $stream = [IO.File]::Open($checksum, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $checksumCreated = $true
        try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) }
        finally { $stream.Dispose() }

        if ((Get-RustArchiveFileSha256 $archive) -ne $archiveHash) { throw 'Archive changed after creation.' }
        if ([IO.File]::ReadAllText($checksum) -cne $line) { throw 'Checksum file verification failed.' }
        [pscustomobject]@{
            ArchivePath = $archive
            ChecksumPath = $checksum
            Sha256 = $archiveHash
            FileCount = $expected.Count
        }
    } catch {
        if ($checksumCreated -and (Test-Path -LiteralPath $checksum -PathType Leaf)) { Remove-Item -LiteralPath $checksum -Force }
        if ($archiveCreated -and (Test-Path -LiteralPath $archive -PathType Leaf)) { Remove-Item -LiteralPath $archive -Force }
        if ($tempCreated -and (Test-Path -LiteralPath $tempArchive -PathType Leaf)) { Remove-Item -LiteralPath $tempArchive -Force }
        throw
    }
}

Export-ModuleMember -Function New-RustPortableArchive
