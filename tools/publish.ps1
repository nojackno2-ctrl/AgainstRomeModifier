param(
    [string]$Version = ''
)

$ErrorActionPreference = 'Stop'

$scriptPath = $MyInvocation.MyCommand.Path
$toolsDir = Split-Path -Parent $scriptPath
$repoRoot = Split-Path -Parent $toolsDir
$projectPath = Join-Path $repoRoot 'src.Modifier\AgainstRomeModifier.csproj'

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = (& dotnet msbuild $projectPath `
        -getProperty:Version `
        -p:Configuration=Release `
        -p:IncludeBackupZip=false).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($Version)) {
        throw 'Unable to read the project version.'
    }
}

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version must be MAJOR.MINOR.PATCH; got '$Version'."
}

$projectVersion = (& dotnet msbuild $projectPath `
    -getProperty:Version `
    -p:Configuration=Release `
    -p:IncludeBackupZip=false).Trim()
if ($LASTEXITCODE -ne 0) {
    throw 'Unable to verify the project version.'
}
if ($Version -ne $projectVersion) {
    throw "Requested version $Version does not match project version $projectVersion."
}

$stagingDir = Join-Path $repoRoot 'bin\Release\publish_staging'
if (Test-Path -LiteralPath $stagingDir) {
    Remove-Item -LiteralPath $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir | Out-Null

Write-Host "Publishing Against Rome Modifier $Version without proprietary Backup.zip..."
dotnet restore $projectPath `
    -r win-x64 `
    --configfile (Join-Path $repoRoot 'NuGet.Config') `
    -p:IncludeBackupZip=false
if ($LASTEXITCODE -ne 0) {
    throw "dotnet restore for win-x64 failed with exit code $LASTEXITCODE."
}

dotnet publish $projectPath `
    -c Release `
    -r win-x64 `
    --self-contained true `
    --no-restore `
    -p:PublishSingleFile=true `
    -p:PublishReadyToRun=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:IncludeBackupZip=false `
    -p:DebugType=None `
    -p:DebugSymbols=false `
    -o $stagingDir
if ($LASTEXITCODE -ne 0) {
    throw "dotnet publish failed with exit code $LASTEXITCODE."
}

Get-ChildItem -LiteralPath $stagingDir -File -Recurse |
    Where-Object { $_.Extension -in @('.pdb', '.xml') } |
    Remove-Item -Force

$zipName = "AgainstRomeModifier_v${Version}_win-x64.zip"
$zipPath = Join-Path $repoRoot $zipName
if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}
$publishedExe = (Get-Item -LiteralPath (Join-Path $stagingDir 'AgainstRomeModifier.exe')).FullName
Compress-Archive `
    -Path $publishedExe `
    -DestinationPath $zipPath `
    -Force

& (Join-Path $toolsDir 'Test-ReleaseArtifacts.ps1') `
    -ArtifactDirectory $stagingDir `
    -ArchivePath $zipPath `
    -AssemblyPath (Join-Path $repoRoot 'src.Core\bin\Release\net8.0-windows\AgainstRome.Core.dll') `
    -ExpectedVersion $Version

$hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash
Write-Host "Publish complete: $zipPath"
Write-Host "SHA256: $hash"
