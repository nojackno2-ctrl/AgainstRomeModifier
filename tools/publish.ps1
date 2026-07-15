param(
    [string]$Version = "1.0.0",
    [bool]$SelfContained = $true
)

$ErrorActionPreference = "Stop"

Write-Host "Starting publish for Against Rome Modifier suite, Version: $Version, SelfContained: $SelfContained..."

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
$configuration = "Release"
$runtime = "win-x64"
$selfContainedArg = $SelfContained.ToString().ToLower()

$extraArgs = @()
if ($SelfContained) {
    $extraArgs += "-p:PublishSingleFile=true"
    $extraArgs += "-p:PublishReadyToRun=true"
}

# 3. Publish every app in the suite
$apps = @(
    @{ Name = "AgainstRomeLauncher";    Project = "src.Launcher\AgainstRomeLauncher.csproj" },
    @{ Name = "AgainstRomeModifier";    Project = "src.Modifier\AgainstRomeModifier.csproj" },
    @{ Name = "AgainstRomeSaveManager"; Project = "src.SaveManager\AgainstRomeSaveManager.csproj" },
    @{ Name = "AgainstRomeMapEditor";   Project = "src.MapEditor\AgainstRomeMapEditor.csproj" }
)

foreach ($app in $apps) {
    Write-Host "Publishing $($app.Name)..."
    $projPath = Join-Path $repoRoot $app.Project
    dotnet publish $projPath -c $configuration -r $runtime --self-contained $selfContainedArg $extraArgs
    if ($LASTEXITCODE -ne 0) {
        throw "dotnet publish failed for $($app.Name) (exit code $LASTEXITCODE)."
    }
}

# 4. Copy outputs to staging.
# The Launcher publish output is copied wholesale (it is the entry point and, for
# framework-dependent builds, carries the shared runtime config layout); the other
# apps contribute their executables (plus dll/runtimeconfig when not single-file).
Write-Host "Copying files to staging: $stagingDir"

foreach ($app in $apps) {
    $projDir = Split-Path -Parent (Join-Path $repoRoot $app.Project)
    $publishDir = Join-Path $projDir "bin\$configuration\net8.0-windows\$runtime\publish"
    if (-not (Test-Path $publishDir)) {
        throw "Publish output not found for $($app.Name): $publishDir"
    }

    if ($SelfContained) {
        Copy-Item -Path (Join-Path $publishDir "$($app.Name).exe") -Destination $stagingDir -Force
    } else {
        Copy-Item -Path "$publishDir\*" -Destination $stagingDir -Recurse -Force
    }
}

# Remove pdb files
Get-ChildItem -Path $stagingDir -Filter "*.pdb" -Recurse | Remove-Item -Force

# 5. Create ZIP archive
$zipName = "AgainstRomeModifier_v$Version`_win-x64.zip"
$zipPath = Join-Path $repoRoot $zipName
if (Test-Path $zipPath) {
    Remove-Item $zipPath -Force
}

Write-Host "Compressing staging directory to $zipName..."
Compress-Archive -Path "$stagingDir\*" -DestinationPath $zipPath -Force

Write-Host "Publish complete! Package saved to: $zipPath"
