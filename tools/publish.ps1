param(
    [string]$Version = "1.0.0",
    [bool]$SelfContained = $true
)

$ErrorActionPreference = "Stop"

Write-Host "Starting publish for Against Rome Modifier, Version: $Version, SelfContained: $SelfContained..."

# 1. Get Repo Root
$scriptPath = $MyInvocation.MyCommand.Path
$toolsDir = Split-Path -Parent $scriptPath
$repoRoot = Split-Path -Parent $toolsDir

$stagingDir = Join-Path $repoRoot "bin\Release\publish_staging"
if (Test-Path $stagingDir) {
    Write-Host "Cleaning existing staging directory: $stagingDir"
    Remove-Item -Recurse -Force $stagingDir
}
New-Item -ItemType Directory -Path $stagingDir | Out-Null

# 2. Define common publish parameters
$commonParams = @{
    Configuration = "Release"
    Runtime = "win-x64"
    SelfContained = $SelfContained.ToString().ToLower()
}

$extraArgs = @()
if ($SelfContained) {
    $extraArgs += "-p:PublishSingleFile=true"
    $extraArgs += "-p:PublishReadyToRun=true"
}

# 3. Publish MapEditor
Write-Host "Publishing MapEditor..."
$mapEditorProj = Join-Path $repoRoot "src.MapEditor\AgainstRomeMapEditor.csproj"
dotnet publish $mapEditorProj -c $commonParams.Configuration -r $commonParams.Runtime --self-contained $commonParams.SelfContained $extraArgs

# 4. Publish Modifier
Write-Host "Publishing AgainstRomeModifier..."
$modifierProj = Join-Path $repoRoot "AgainstRomeModifier.csproj"
dotnet publish $modifierProj -c $commonParams.Configuration -r $commonParams.Runtime --self-contained $commonParams.SelfContained $extraArgs

# 5. Copy outputs to staging
$modifierPublishDir = Join-Path $repoRoot "bin\Release\net8.0-windows\win-x64\publish"
$mapEditorPublishDir = Join-Path $repoRoot "src.MapEditor\bin\Release\net8.0-windows\win-x64\publish"

Write-Host "Copying files to staging: $stagingDir"
Copy-Item -Path "$modifierPublishDir\*" -Destination $stagingDir -Recurse -Force

if ($SelfContained) {
    Copy-Item -Path "$mapEditorPublishDir\AgainstRomeMapEditor.exe" -Destination $stagingDir -Force
} else {
    Copy-Item -Path "$mapEditorPublishDir\AgainstRomeMapEditor.exe" -Destination $stagingDir -Force
    Copy-Item -Path "$mapEditorPublishDir\AgainstRomeMapEditor.dll" -Destination $stagingDir -Force
    Copy-Item -Path "$mapEditorPublishDir\AgainstRomeMapEditor.runtimeconfig.json" -Destination $stagingDir -Force
}

# Remove pdb files
Get-ChildItem -Path $stagingDir -Filter "*.pdb" | Remove-Item -Force

# 6. Create ZIP archive
$zipName = "AgainstRomeModifier_v$Version`_win-x64.zip"
$zipPath = Join-Path $repoRoot $zipName
if (Test-Path $zipPath) {
    Remove-Item $zipPath -Force
}

Write-Host "Compressing staging directory to $zipName..."
Compress-Archive -Path "$stagingDir\*" -DestinationPath $zipPath -Force

Write-Host "Publish complete! Package saved to: $zipPath"
