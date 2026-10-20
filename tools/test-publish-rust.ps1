#Requires -Version 7
<#
.SYNOPSIS
    Tests tools/publish-rust.ps1 contract, parse validity, AST structure, and -WhatIf dry run behavior for six targets.
.DESCRIPTION
    Verifies that tools/publish-rust.ps1 parses without syntax errors, declares the expected
    parameters, targets six distinct executables (against-rome-editor.exe, against-rome-modifier-gui.exe,
    against-rome-launcher.exe, save-manager.exe, against-rome-modifier.exe, and ArmMapPackage.exe) without duplicate
    alias against-rome-save-manager.exe, contains AST assertions for all six Cargo targets, locked cargo fetch bootstrap,
    and zero pre-ShouldProcess filesystem mutation, and executes -WhatIf and -WhatIf -SkipBuild without creating
    or modifying directories. Also verifies clean isolated CARGO_HOME locked fetch bootstrap and offline builds.
#>
[CmdletBinding()]
param([switch]$SkipNetworkFetch)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptDir = $PSScriptRoot
$repo = Split-Path $scriptDir -Parent
$targetScript = Join-Path $scriptDir 'publish-rust.ps1'

if (!(Test-Path -LiteralPath $targetScript -PathType Leaf)) {
    throw "Target script not found: $targetScript"
}

# Reads the headers of a PE file; returns machine, characteristics, magic, export RVA and CLR RVA (PE32 layout only for DLL checks).
function Get-PeInfo([string]$Path) {
    $b = [IO.File]::ReadAllBytes($Path)
    if ($b.Length -lt 512 -or [BitConverter]::ToUInt16($b, 0) -ne 0x5a4d) { throw "Not a PE file: $Path" }
    $pe = [BitConverter]::ToUInt32($b, 60)
    if ([BitConverter]::ToUInt32($b, $pe) -ne 0x4550) { throw "Missing PE signature: $Path" }
    $magic = [BitConverter]::ToUInt16($b, $pe + 24)
    $dirBase = $pe + 24 + $(if ($magic -eq 0x10b) { 96 } else { 112 })
    [pscustomobject]@{
        Machine = [BitConverter]::ToUInt16($b, $pe + 4); Characteristics = [BitConverter]::ToUInt16($b, $pe + 22)
        Magic = $magic; ExportRva = [BitConverter]::ToUInt32($b, $dirBase); ClrRva = [BitConverter]::ToUInt32($b, $dirBase + 14 * 8)
    }
}
function Assert-Pe32Dll([string]$Path, [string]$Label) {
    $i = Get-PeInfo $Path
    if ($i.Machine -ne 0x14c -or $i.Magic -ne 0x10b -or ($i.Characteristics -band 0x2000) -eq 0 -or $i.ExportRva -eq 0 -or $i.ClrRva -ne 0) {
        throw "$Label is not a valid native 32-bit PE DLL with exports: $Path ($($i | ConvertTo-Json -Compress))"
    }
}
# Minimal PE32 i386 DLL header fixture (never executed); flags select the invalid variants.
function New-Pe32DllFixtureBytes([int]$Machine = 0x14c, [bool]$IsDll = $true, [uint32]$ClrRva = 0, [uint32]$ExportRva = 0x1000) {
    $bytes = [byte[]]::new(1024)
    [BitConverter]::GetBytes([uint16]0x5a4d).CopyTo($bytes, 0)
    [BitConverter]::GetBytes([uint32]64).CopyTo($bytes, 60)
    [BitConverter]::GetBytes([uint32]0x4550).CopyTo($bytes, 64)
    [BitConverter]::GetBytes([uint16]$Machine).CopyTo($bytes, 68)
    [BitConverter]::GetBytes([uint16]$(if ($IsDll) { 0x2102 } else { 0x0102 })).CopyTo($bytes, 86)
    [BitConverter]::GetBytes([uint16]0x10b).CopyTo($bytes, 88)
    [BitConverter]::GetBytes([uint32]16).CopyTo($bytes, 180)
    [BitConverter]::GetBytes($ExportRva).CopyTo($bytes, 184)
    [BitConverter]::GetBytes($ClrRva).CopyTo($bytes, 296)
    $bytes
}

Write-Host "Testing publish-rust.ps1 contract..."

# 1. PowerShell AST syntax parse validation
$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($targetScript, [ref]$tokens, [ref]$errors)
if ($errors.Count -gt 0) {
    throw "Parse error in ${targetScript}: $($errors -join '; ')"
}
Write-Host "  [PASS] Parse validation (0 errors)"

# 2. Parameter and metadata verification
if (!$ast.ParamBlock) {
    throw "Missing param block in $targetScript"
}
$paramNames = $ast.ParamBlock.Parameters.Name.VariablePath.UserPath
foreach ($expected in @('OutputDirectory', 'SkipBuild', 'Offline')) {
    if ($paramNames -notcontains $expected) {
        throw "Missing expected parameter '$expected' in $targetScript"
    }
}
Write-Host "  [PASS] Parameters OutputDirectory and SkipBuild declared"

# 3. PowerShell AST assertions: six Cargo targets, six executables, locked fetch bootstrap, and ShouldProcess write guard
$stringNodes = $ast.FindAll({ $args[0] -is [System.Management.Automation.Language.StringConstantExpressionAst] }, $true)
$stringValues = @($stringNodes | ForEach-Object { $_.Value })

$expectedCargoTargets = @(
    'against-rome-editor',
    'against-rome-modifier-gui',
    'against-rome-launcher',
    'save-manager',
    'against-rome-modifier',
    'ArmMapPackage'
)
foreach ($target in $expectedCargoTargets) {
    if ($stringValues -notcontains $target) {
        throw "PowerShell AST assertion failed: Missing Cargo target '$target' in AST string constants."
    }
}

$expectedExecutables = @(
    'against-rome-editor.exe',
    'against-rome-modifier-gui.exe',
    'against-rome-launcher.exe',
    'save-manager.exe',
    'against-rome-modifier.exe',
    'ArmMapPackage.exe'
)
foreach ($exe in $expectedExecutables) {
    if ($stringValues -notcontains $exe) {
        throw "PowerShell AST assertion failed: Missing executable '$exe' in AST string constants."
    }
}

foreach ($token in @('fetch', '--locked')) {
    if ($stringValues -notcontains $token) {
        throw "PowerShell AST assertion failed: Missing '$token' in AST string constants."
    }
}

# Assert duplicate alias against-rome-save-manager.exe is NOT packaged
if ($stringValues -contains 'against-rome-save-manager.exe') {
    throw "Duplicate alias 'against-rome-save-manager.exe' found in AST string constants; should not package duplicate alias."
}

# Verify that ShouldProcess guards all mutation cmdlets and archive generation.
$commandAsts = $ast.FindAll({ $args[0] -is [System.Management.Automation.Language.CommandAst] }, $true)
$ifStatements = $ast.FindAll({ $args[0] -is [System.Management.Automation.Language.IfStatementAst] }, $true)
$shouldProcessIf = @($ifStatements | Where-Object {
    foreach ($clause in $_.Clauses) {
        if ($clause.Item1.Extent.Text -match 'ShouldProcess') { return $true }
    }
    return $false
})
if ($shouldProcessIf.Count -eq 0) {
    throw "PowerShell AST assertion failed: Missing `$PSCmdlet.ShouldProcess guard in script AST."
}
# Ensure no mutating cmdlets exist before the ShouldProcess check
$shouldProcessLine = $shouldProcessIf[0].Extent.StartLineNumber
$archiveCalls = @($commandAsts | Where-Object { $_.GetCommandName() -eq 'New-RustPortableArchive' })
if ($archiveCalls.Count -ne 1 -or $archiveCalls[0].Extent.StartLineNumber -lt $shouldProcessLine) {
    throw 'PowerShell AST assertion failed: portable archive creation must occur exactly once, after ShouldProcess.'
}
$mutatingCmdlets = @('New-Item', 'Copy-Item', 'Set-Content', 'Out-File', 'Remove-Item')
$preShouldProcessMutations = @($commandAsts | Where-Object {
    $cmdName = $_.GetCommandName()
    $cmdName -in $mutatingCmdlets -and $_.Extent.StartLineNumber -lt $shouldProcessLine
})
if ($preShouldProcessMutations.Count -gt 0) {
    throw "PowerShell AST assertion failed: Found mutating cmdlets before ShouldProcess check on lines: $(($preShouldProcessMutations | ForEach-Object { $_.Extent.StartLineNumber }) -join ', ')"
}
Write-Host "  [PASS] PowerShell AST assertions (6 Cargo targets, 6 executables, no duplicate alias, write guard, archive after approval) verified"

# 4. Content checks: ensure manifests and target packages are referenced
$scriptContent = Get-Content -LiteralPath $targetScript -Raw
if ($scriptContent -notmatch '\$rootManifest\s*=\s*Join-Path\s+\$repo\s+''Cargo\.toml''') {
    throw "Script does not declare rootManifest pointing to Cargo.toml"
}
if ($scriptContent -notmatch 'native/against-rome-modifier/Cargo\.toml') {
    throw "Script does not reference native/against-rome-modifier/Cargo.toml"
}
if ($scriptContent -notmatch 'native/against-rome-packaging/Cargo\.toml') {
    throw "Script does not reference native/against-rome-packaging/Cargo.toml"
}
if ($scriptContent -notmatch 'apps/against-rome-editor/Cargo\.toml') {
    throw "Script does not reference apps/against-rome-editor/Cargo.toml"
}
if ($scriptContent -notmatch '\$fetchArgs\s*=') {
    throw "Script does not declare `$fetchArgs"
}
if ($scriptContent -notmatch '\$cargo\s+@fetchArgs') {
    throw "Script does not invoke cargo with @fetchArgs"
}
if ($scriptContent -match 'against-rome-save-manager\.exe') {
    throw "Script must not reference against-rome-save-manager.exe duplicate alias."
}
if ($scriptContent -notmatch 'RustPortableArchive\.psm1' -or $scriptContent -notmatch '\.zip\.sha256') {
    throw 'Script must use the verified portable archive helper and emit a .zip.sha256 sidecar.'
}
if ($scriptContent -notmatch 'native/against-rome-trace/Cargo\.toml') { throw 'Script does not reference native/against-rome-trace/Cargo.toml' }
if ($scriptContent -notmatch '\$traceTargetTriple\s*=\s*''i686-pc-windows-msvc''') { throw 'Trace crate must be built for i686-pc-windows-msvc.' }
if ($scriptContent -notmatch '\$traceBuildArgs\s*=\s*@\(''build'', ''--release'', ''--locked'', ''--offline'', ''--manifest-path'', \$traceManifest') {
    throw 'Trace crate must be built with cargo build --release --locked --offline --manifest-path $traceManifest.'
}
if ($scriptContent -notmatch '&\s+\$cargo\s+@traceBuildArgs') { throw 'Script does not invoke cargo with @traceBuildArgs.' }
if ($scriptContent -notmatch 'Assert-NativeWindowsDll32\s+\$traceDll') { throw 'Script must validate the built trace DLL as a 32-bit PE.' }
# The C++ oracle binary must never be a copy source: it is neither an $assets entry nor Join-Path'ed from the repo.
$assetsAssign = $ast.FindAll({ $args[0] -is [System.Management.Automation.Language.AssignmentStatementAst] -and $args[0].Left.Extent.Text -eq '$assets' }, $true)
if ($assetsAssign.Count -ne 1) { throw 'Expected exactly one $assets assignment.' }
$assetStrings = @($assetsAssign[0].Right.FindAll({ $args[0] -is [System.Management.Automation.Language.StringConstantExpressionAst] }, $true) | ForEach-Object Value)
if ($assetStrings -contains 'ThirdParty/argm-trace/winmm.dll') { throw 'Legacy C++ ThirdParty/argm-trace/winmm.dll must not be a runtime asset copied from the source tree.' }
if ($scriptContent -match 'Copy-Item[^\r\n]*(?:ThirdParty/argm-trace/winmm\.dll|\$traceStagedRelative)') {
    throw 'Script must not copy the C++ source-tree winmm.dll.'
}
if ($scriptContent -notmatch 'matches legacy C\+\+ binary') {
    throw 'Script must explicitly verify that the built trace DLL does not match the legacy C++ binary.'
}
if ($scriptContent -notmatch 'Copy-Item\s+-LiteralPath\s+\$traceDll\s+-Destination\s+\$traceDest') { throw 'Staged trace DLL must be copied from the Cargo build output ($traceDll).' }
if ($scriptContent -match 'cmake|MSBuild|argm-trace/build') { throw 'Publisher must not invoke the C++ toolchain.' }
Write-Host "  [PASS] Manifest declarations, locked fetch bootstrap, and no-duplicate-alias verified"
Write-Host "  [PASS] Rust trace crate: i686 locked/offline Release cdylib build, PE32 validation, C++ source DLL not a copy source, C++ match rejection"

# Execute actual DLL-only blocks with Cargo stubs; do not load a DLL or launch tests.
foreach ($route in @('native/against-rome-trace/build-win32.ps1', 'tools/test-rust-rewrite.ps1', 'tools/publish-rust.ps1')) {
    $routeTokens = $null; $routeErrors = $null
    $routeAst = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $repo $route), [ref]$routeTokens, [ref]$routeErrors)
    if ($routeErrors.Count) { throw "Parse errors in $route" }
    $blocks = @($routeAst.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.TryStatementAst] -and
        @($node.Body.Statements | Where-Object {
            $_ -is [System.Management.Automation.Language.AssignmentStatementAst] -and
            $_.Left.Extent.Text -eq '$env:CARGO_PROFILE_RELEASE_PANIC' -and $_.Right.Extent.Text -eq "'abort'"
        }).Count -eq 1
    }, $true))
    if ($blocks.Count -ne 1) { throw "Expected one DLL-only abort block in $route" }
    $block = $blocks[0]
    $save = @($routeAst.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and
        $node.Left.Extent.Text -eq '$previousReleasePanic' -and
        $node.Right.Extent.Text -eq '$env:CARGO_PROFILE_RELEASE_PANIC'
    }, $true))
    if ($save.Count -ne 1 -or $save[0].Extent.EndOffset -gt $block.Extent.StartOffset) { throw "Missing caller panic snapshot in $route" }
    if (!$block.Finally -or $block.Finally.Extent.Text -notmatch '\$env:CARGO_PROFILE_RELEASE_PANIC\s*=\s*\$previousReleasePanic') {
        throw "Missing panic restoration in $route"
    }
    $probe = [scriptblock]::Create($save[0].Extent.Text + [Environment]::NewLine + $block.Extent.Text)
    & {
        $callerPanic = $env:CARGO_PROFILE_RELEASE_PANIC
        try {
            $cargo = 'cargo'
            $manifest = 'trace/Cargo.toml'; $target = 'trace-target'
            $traceDllName = 'winmm.dll'; $traceTargetTriple = 'i686-pc-windows-msvc'
            $common = @('--release', '--target', $traceTargetTriple)
            $traceBuildArgs = @('build') + $common
            foreach ($original in @($null, '', 'unwind', 'abort')) {
                foreach ($failCommand in @('none', 'rustc', 'build', 'unwind')) {
                    $env:CARGO_PROFILE_RELEASE_PANIC = $original
                    $script:traceStubCalls = 0
                    function cargo {
                        if ($env:CARGO_PROFILE_RELEASE_PANIC -cne 'abort') { throw 'DLL build lost abort setting.' }
                        if ($args[0] -notin @('build', 'rustc')) { throw 'Panic override reached a non-DLL command.' }
                        $script:traceStubCalls++
                        $global:LASTEXITCODE = if ($args[0] -eq $failCommand) { 19 } else { 0 }
                        if ($args[0] -eq 'rustc') {
                            if ($failCommand -eq 'unwind') { 'panic="unwind"' } else { 'panic="abort"' }
                            'target_feature="crt-static"'
                        }
                    }
                    $failed = $false
                    try { & $probe } catch { $failed = $true; if ($failCommand -eq 'none') { throw } }
                    if (($failed -ne ($failCommand -ne 'none')) -or $script:traceStubCalls -eq 0) { throw "DLL failure propagation broken in $route" }
                    if ([Environment]::GetEnvironmentVariable('CARGO_PROFILE_RELEASE_PANIC') -cne $original) {
                        throw "Caller panic value was not restored in $route (failure=$failCommand)"
                    }
                }
            }
        } finally { $env:CARGO_PROFILE_RELEASE_PANIC = $callerPanic }
    }
}
Write-Host '  [PASS] Three DLL routes restore absent/unwind/abort caller values on success and Cargo failure'

# Evaluate the actual provenance table for both build modes.
$traceTables = @($ast.FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.HashtableAst] -and
    @($node.KeyValuePairs | Where-Object { $_.Item1.Extent.Text -eq 'panicStrategy' }).Count -eq 1
}, $true))
if ($traceTables.Count -ne 1) { throw 'Expected one trace panic provenance table.' }
& {
    $traceStagedRelative = 'ThirdParty/argm-trace/winmm.dll'; $traceDll = 'trace/winmm.dll'; $traceDllHash = 'fixture'
    $traceTargetTriple = 'i686-pc-windows-msvc'; $traceBuildArgs = @('build', '--release')
    foreach ($SkipBuild in @($false, $true)) {
        $table = & ([scriptblock]::Create($traceTables[0].Extent.Text))
        $expected = if ($SkipBuild) { 'unverifiable' } else { 'abort' }
        if ($table.panicStrategy -cne $expected) { throw 'Incorrect trace panic provenance.' }
        if (!$SkipBuild -and $table.buildCommand -notmatch '^CARGO_PROFILE_RELEASE_PANIC=abort cargo ') {
            throw 'Trace provenance buildCommand lost its scoped panic environment.'
        }
    }
}
Write-Host '  [PASS] Trace panic provenance records abort for builds and unverifiable for SkipBuild'

# 5. -WhatIf dry-run check (standard build mode)
$artifactsDir = Join-Path $repo 'artifacts/rust-editor'
$artifactsDirExistedBefore = Test-Path -LiteralPath $artifactsDir

$whatIfOutput = & pwsh -NoProfile -ExecutionPolicy Bypass -File $targetScript -WhatIf 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "-WhatIf run failed with exit code ${LASTEXITCODE}: $($whatIfOutput -join "`n")"
}
$whatIfText = $whatIfOutput -join "`n"
if ($whatIfText -notmatch 'Fetch\s+:\s+cargo\s+fetch\s+--locked') {
    throw "-WhatIf output must declare locked cargo fetch step: $whatIfText"
}
foreach ($exe in $expectedExecutables) {
    if ($whatIfText -notmatch [regex]::Escape($exe)) {
        throw "-WhatIf output missing executable '$exe': $whatIfText"
    }
}
foreach ($target in $expectedCargoTargets) {
    if ($whatIfText -notmatch "--bin\s+$([regex]::Escape($target))\b") {
        throw "-WhatIf output must list distinct Cargo build target for '$target': $whatIfText"
    }
}
if ($whatIfText -notmatch 'cargo build --release --locked --offline --manifest-path [^\r\n]*native[\\/]against-rome-trace[\\/]Cargo\.toml --lib --target-dir [^\r\n]*native-against-rome-trace --target i686-pc-windows-msvc') {
    throw "-WhatIf output must show the locked/offline i686 Rust trace crate build: $whatIfText"
}
if ($whatIfText -notmatch 'Dry run: nothing built, nothing written\.') {
    throw "-WhatIf output missing dry run confirmation message."
}
if ($whatIfText -notmatch '\.zip' -or $whatIfText -notmatch '\.zip\.sha256') {
    throw "-WhatIf output must show the archive and checksum destinations: $whatIfText"
}
if (!$artifactsDirExistedBefore -and (Test-Path -LiteralPath $artifactsDir)) {
    throw "-WhatIf unexpectedly created artifacts directory: $artifactsDir"
}
Write-Host "  [PASS] -WhatIf dry-run check verified locked fetch and all $($expectedExecutables.Count) distinct targets without disk writes"

if ($whatIfText -notmatch 'CARGO_PROFILE_RELEASE_PANIC=abort cargo build') { throw 'Trace dry run must disclose panic=abort.' }

# 6. -WhatIf -SkipBuild dry-run check
$skipBuildOutput = & pwsh -NoProfile -ExecutionPolicy Bypass -File $targetScript -WhatIf -SkipBuild 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "-WhatIf -SkipBuild run failed with exit code ${LASTEXITCODE}: $($skipBuildOutput -join "`n")"
}
$skipBuildText = $skipBuildOutput -join "`n"
if ($skipBuildText -notmatch 'skipbuild-unverified') {
    throw "-WhatIf -SkipBuild output missing skipbuild-unverified marker: $skipBuildText"
}
if ($skipBuildText -notmatch 'Fetch\s+:\s+cargo\s+fetch\s+--locked\s+.*\[skipped') {
    throw "-WhatIf -SkipBuild output must show fetch skipped: $skipBuildText"
}
foreach ($exe in $expectedExecutables) {
    if ($skipBuildText -notmatch [regex]::Escape($exe)) {
        throw "-WhatIf -SkipBuild output missing executable reference '$exe': $skipBuildText"
    }
}
if (!$artifactsDirExistedBefore -and (Test-Path -LiteralPath $artifactsDir)) {
    throw "-WhatIf -SkipBuild unexpectedly created artifacts directory: $artifactsDir"
}
Write-Host "  [PASS] -WhatIf -SkipBuild dry-run check succeeded across all $($expectedExecutables.Count) executables"

$offlineOutput = & pwsh -NoProfile -File $targetScript -WhatIf -Offline 2>&1
if ($LASTEXITCODE -ne 0) { throw "Offline dry run failed: $offlineOutput" }
$offlineText = $offlineOutput -join "`n"
if ($offlineText -notmatch '\[skipped; -Offline\]') { throw 'Offline mode must skip network fetch.' }
foreach ($line in @($offlineOutput | Where-Object { "$_" -match 'cargo build' })) {
    $expectedTriple = if ("$line" -match 'against-rome-trace') { 'i686-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
    if ("$line" -notmatch '--locked --offline' -or "$line" -notmatch "--target $expectedTriple") {
        throw "Build command is not locked/offline with explicit Windows x64 target: $line"
    }
}
Write-Host '  [PASS] -Offline dry run skips fetch and pins locked/offline Windows x64 builds'

# 7. -WhatIf custom OutputDirectory non-creation check (proves no disk writes to custom destination)
$customTarget = Join-Path $repo ("artifacts/test-whatif-custom-" + [System.Guid]::NewGuid().ToString('N'))
try {
    $customOutput = & pwsh -NoProfile -ExecutionPolicy Bypass -File $targetScript -OutputDirectory $customTarget -WhatIf 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "-WhatIf custom OutputDirectory failed with exit code ${LASTEXITCODE}: $($customOutput -join "`n")"
    }
    $customText = $customOutput -join "`n"
    foreach ($target in $expectedCargoTargets) {
        if ($customText -notmatch "--bin\s+$([regex]::Escape($target))\b") {
            throw "Custom OutputDirectory -WhatIf missing target '$target': $customText"
        }
    }
    if (Test-Path -LiteralPath $customTarget) {
        throw "Custom OutputDirectory was unexpectedly created during -WhatIf: $customTarget"
    }
    Write-Host "  [PASS] -WhatIf custom target dry-run verified $($expectedExecutables.Count) distinct targets and zero disk creation"
} finally {
    if (Test-Path -LiteralPath $customTarget) {
        Remove-Item -LiteralPath $customTarget -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# 8. Exercise the archive helper against a synthetic portable folder and verify every entry/hash.
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
$archiveTestRoot = Join-Path $tempRoot ("AgainstRomeRustArchiveTest-" + [Guid]::NewGuid().ToString('N'))
if ([IO.Path]::GetDirectoryName($archiveTestRoot) -ine $tempRoot) { throw "Unsafe archive test directory: $archiveTestRoot" }
try {
    $sourceDir = Join-Path $archiveTestRoot 'portable'
    $outputDir = Join-Path $archiveTestRoot 'output'
    New-Item -ItemType Directory -Path (Join-Path $sourceDir 'ThirdParty/dgVoodoo2') -Force | Out-Null
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    [IO.File]::WriteAllBytes((Join-Path $sourceDir 'against-rome-editor.exe'), [Text.Encoding]::UTF8.GetBytes('synthetic executable'))
    [IO.File]::WriteAllBytes((Join-Path $sourceDir 'ThirdParty/dgVoodoo2/D3D8.dll'), [byte[]](0, 1, 2, 3, 255))
    [IO.File]::WriteAllText((Join-Path $sourceDir 'provenance.json'), '{"schema":2}', [Text.UTF8Encoding]::new($false))

    Import-Module (Join-Path $scriptDir 'RustPortableArchive.psm1') -Force
    $archive = Join-Path $outputDir 'against-rome-editor-test-win-x64.zip'
    $checksum = "$archive.sha256"
    $result = New-RustPortableArchive -SourceDirectory $sourceDir -DestinationPath $archive -ChecksumPath $checksum
    if ($result.FileCount -ne 3) { throw "Expected 3 verified archive entries; found $($result.FileCount)." }
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $names = @($zip.Entries | ForEach-Object FullName | Sort-Object)
        $expectedNames = @('ThirdParty/dgVoodoo2/D3D8.dll', 'against-rome-editor.exe', 'provenance.json') | Sort-Object
        if (($names -join "`n") -cne ($expectedNames -join "`n")) { throw "Unexpected ZIP entry list: $($names -join ', ')" }
    } finally { $zip.Dispose() }
    $archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $expectedLine = "$archiveHash  $([IO.Path]::GetFileName($archive))`n"
    if ($result.Sha256 -cne $archiveHash -or [IO.File]::ReadAllText($checksum) -cne $expectedLine) {
        throw 'Archive SHA256 result or sidecar contents do not match the ZIP.'
    }
    $beforeZip = [IO.File]::ReadAllBytes($archive)
    try {
        New-RustPortableArchive -SourceDirectory $sourceDir -DestinationPath $archive -ChecksumPath $checksum | Out-Null
        throw 'Archive helper unexpectedly overwrote existing output.'
    } catch {
        if ($_.Exception.Message -notmatch 'already exists; refusing to overwrite') { throw }
    }
    if ([Convert]::ToBase64String($beforeZip) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($archive))) {
        throw 'Refusing an existing archive changed its bytes.'
    }
    Write-Host '  [PASS] Synthetic ZIP entry list, contents, SHA256 sidecar, and no-overwrite behavior verified'
} finally {
    if (Test-Path -LiteralPath $archiveTestRoot) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($archiveTestRoot)) -ine $tempRoot) {
            throw "Refusing recursive cleanup outside TEMP: $archiveTestRoot"
        }
        Remove-Item -LiteralPath $archiveTestRoot -Recurse -Force
    }
}

# 9. Clean isolated CARGO_HOME bootstrap and offline build verification
$isolatedCargoHome = Join-Path $tempRoot ("AgainstRomeCargoHomeTest-" + [Guid]::NewGuid().ToString('N'))
$isolatedTargetDir = Join-Path $tempRoot ("AgainstRomeTargetTest-" + [Guid]::NewGuid().ToString('N'))
$isolatedStageDir = Join-Path $tempRoot ("AgainstRomeStageTest-" + [Guid]::NewGuid().ToString('N'))

if ([IO.Path]::GetDirectoryName($isolatedCargoHome) -ine $tempRoot) { throw "Unsafe test directory: $isolatedCargoHome" }
if ([IO.Path]::GetDirectoryName($isolatedTargetDir) -ine $tempRoot) { throw "Unsafe test directory: $isolatedTargetDir" }
if ([IO.Path]::GetDirectoryName($isolatedStageDir) -ine $tempRoot) { throw "Unsafe test directory: $isolatedStageDir" }

New-Item -ItemType Directory -Path $isolatedCargoHome -Force | Out-Null
New-Item -ItemType Directory -Path $isolatedTargetDir -Force | Out-Null
New-Item -ItemType Directory -Path $isolatedStageDir -Force | Out-Null

try {
    # Resolve cargo binary using the same convention
    $runtimeCargo = Join-Path $repo 're_workspace/rust-runtime/cargo/bin/cargo.exe'
    $cargoCmd = if (Test-Path -LiteralPath $runtimeCargo) { $runtimeCargo } else { (Get-Command cargo -ErrorAction Stop).Source }
    $runtimeBinDir = Join-Path $repo 're_workspace/rust-runtime/cargo/bin'
    $runtimeRustup = Join-Path $repo 're_workspace/rust-runtime/rustup'

    # (a) Verify caller's CARGO_HOME preservation under publish-rust.ps1 -WhatIf
    $whatIfIsolatedOutput = & pwsh -NoProfile -Command "
        `$env:CARGO_HOME = '$isolatedCargoHome'
        & pwsh -NoProfile -ExecutionPolicy Bypass -File '$targetScript' -WhatIf
    " 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Isolated CARGO_HOME -WhatIf failed (exit $LASTEXITCODE): $($whatIfIsolatedOutput -join "`n")"
    }
    $whatIfIsolatedText = $whatIfIsolatedOutput -join "`n"
    if ($whatIfIsolatedText -notmatch [regex]::Escape($isolatedCargoHome)) {
        throw "publish-rust.ps1 -WhatIf did not preserve caller's isolated CARGO_HOME: $whatIfIsolatedText"
    }
    Write-Host "  [PASS] publish-rust.ps1 preserves isolated CARGO_HOME without reverting to default cache"

    # (b) Prove known gap: clean/empty CARGO_HOME fails offline metadata resolution
    $offlineFailOutput = & pwsh -NoProfile -Command "
        `$env:CARGO_HOME = '$isolatedCargoHome'
        if (Test-Path '$runtimeBinDir') { `$env:PATH = '$runtimeBinDir' + [IO.Path]::PathSeparator + `$env:PATH }
        if (Test-Path '$runtimeRustup') { `$env:RUSTUP_HOME = '$runtimeRustup' }
        & '$cargoCmd' metadata --locked --offline --format-version 1 --manifest-path (Join-Path '$repo' 'Cargo.toml') 2>&1
        exit `$LASTEXITCODE
    " 2>&1
    $offlineFailCode = $LASTEXITCODE
    if ($offlineFailCode -eq 0) {
        throw "Expected clean CARGO_HOME offline metadata to fail, but it unexpectedly succeeded."
    }
    $offlineFailText = $offlineFailOutput -join "`n"
    if ($offlineFailCode -ne 101 -or $offlineFailText -notmatch 'no matching package named|attempting to make an HTTP request, but --offline was specified') {
        throw "Unexpected clean CARGO_HOME failure output: $offlineFailText"
    }
    Write-Host "  [PASS] Clean CARGO_HOME offline resolution fails as expected (proven gap without bootstrap)"

    if ($SkipNetworkFetch) {
        Write-Warning "  [SKIP] Skipping network fetch and post-bootstrap compilation tests (-SkipNetworkFetch specified)"
    } else {
        # (c) Perform locked fetch bootstrap into isolated CARGO_HOME
        $fetchSw = [Diagnostics.Stopwatch]::StartNew()
        $fetchOutput = & pwsh -NoProfile -Command "
            `$env:CARGO_HOME = '$isolatedCargoHome'
            if (Test-Path '$runtimeBinDir') { `$env:PATH = '$runtimeBinDir' + [IO.Path]::PathSeparator + `$env:PATH }
            if (Test-Path '$runtimeRustup') { `$env:RUSTUP_HOME = '$runtimeRustup' }
            & '$cargoCmd' fetch --locked --manifest-path (Join-Path '$repo' 'Cargo.toml') 2>&1
            exit `$LASTEXITCODE
        " 2>&1
        $fetchSw.Stop()
        if ($LASTEXITCODE -ne 0) {
            throw "cargo fetch --locked into isolated CARGO_HOME failed (exit $LASTEXITCODE): $($fetchOutput -join "`n")"
        }
        $registryDir = Join-Path $isolatedCargoHome 'registry'
        if (!(Test-Path -LiteralPath $registryDir)) {
            throw "cargo fetch --locked did not populate registry in isolated CARGO_HOME ($isolatedCargoHome)"
        }
        $downloadedFileCount = @(Get-ChildItem -LiteralPath $registryDir -Recurse -File -ErrorAction SilentlyContinue).Count
        if ($downloadedFileCount -eq 0) {
            throw "Isolated CARGO_HOME registry contains 0 files after locked fetch."
        }
        Write-Host "  [PASS] cargo fetch --locked bootstrapped isolated CARGO_HOME ($downloadedFileCount files in $($fetchSw.ElapsedMilliseconds)ms)"

        # (d) Verify post-bootstrap locked offline resolution succeeds in isolated CARGO_HOME
        $offlineSuccessOutput = & pwsh -NoProfile -Command "
            `$env:CARGO_HOME = '$isolatedCargoHome'
            if (Test-Path '$runtimeBinDir') { `$env:PATH = '$runtimeBinDir' + [IO.Path]::PathSeparator + `$env:PATH }
            if (Test-Path '$runtimeRustup') { `$env:RUSTUP_HOME = '$runtimeRustup' }
            & '$cargoCmd' metadata --locked --offline --format-version 1 --manifest-path (Join-Path '$repo' 'Cargo.toml') 2>&1
            exit `$LASTEXITCODE
        " 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Post-fetch offline metadata failed (exit $LASTEXITCODE): $($offlineSuccessOutput -join "`n")"
        }
        Write-Host "  [PASS] Post-fetch offline metadata resolution succeeded in isolated CARGO_HOME"

        # (e) Verify isolated CARGO_HOME compilation succeeds offline
        $compileSw = [Diagnostics.Stopwatch]::StartNew()
        $compileOutput = & pwsh -NoProfile -Command "
            `$env:CARGO_HOME = '$isolatedCargoHome'
            if (Test-Path '$runtimeBinDir') { `$env:PATH = '$runtimeBinDir' + [IO.Path]::PathSeparator + `$env:PATH }
            if (Test-Path '$runtimeRustup') { `$env:RUSTUP_HOME = '$runtimeRustup' }
            & '$cargoCmd' check --locked --offline --manifest-path (Join-Path '$repo' 'native/against-rome-packaging/Cargo.toml') --target-dir '$isolatedTargetDir' 2>&1
            exit `$LASTEXITCODE
        " 2>&1
        $compileSw.Stop()
        if ($LASTEXITCODE -ne 0) {
            throw "Isolated offline cargo check failed (exit $LASTEXITCODE): $($compileOutput -join "`n")"
        }
        Write-Host "  [PASS] Isolated CARGO_HOME offline crate check succeeded ($($compileSw.ElapsedMilliseconds)ms)"
    }

    # Actual staging/rollback is tested below in a private source snapshot, independent of active agents.
} finally {
    if (Test-Path -LiteralPath $isolatedCargoHome) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($isolatedCargoHome)) -ine $tempRoot) {
            throw "Refusing recursive cleanup outside TEMP: $isolatedCargoHome"
        }
        Remove-Item -LiteralPath $isolatedCargoHome -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $isolatedTargetDir) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($isolatedTargetDir)) -ine $tempRoot) {
            throw "Refusing recursive cleanup outside TEMP: $isolatedTargetDir"
        }
        Remove-Item -LiteralPath $isolatedTargetDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $isolatedStageDir) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($isolatedStageDir)) -ine $tempRoot) {
            throw "Refusing recursive cleanup outside TEMP: $isolatedStageDir"
        }
        Remove-Item -LiteralPath $isolatedStageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# 10. Full publisher tests in a private, local-only clone. No commit, user output or live source is changed.
$fixtureRoot = Join-Path $tempRoot ("AgainstRomePublishTest-" + [Guid]::NewGuid().ToString('N'))
$fixtureRepo = Join-Path $fixtureRoot 'source'
$fixtureOutput = Join-Path $fixtureRoot 'output'
$savedCargoHome = $env:CARGO_HOME
try {
    & git clone --quiet --shared --no-checkout -- $repo $fixtureRepo
    if ($LASTEXITCODE -ne 0) { throw 'Local source snapshot clone failed.' }
    New-Item -ItemType Directory -Path (Join-Path $fixtureRepo '.git/info') -Force | Out-Null
    Add-Content -LiteralPath (Join-Path $fixtureRepo '.git/info/exclude') -Value "`n/re_workspace/"
    $fixtureFiles = @('Cargo.toml', 'Cargo.lock', 'apps/against-rome-editor/Cargo.toml',
        'native/against-rome-modifier/Cargo.toml', 'native/against-rome-packaging/Cargo.toml',
        'native/against-rome-trace/Cargo.toml', 'native/against-rome-trace/Cargo.lock',
        'tools/publish-rust.ps1', 'tools/RustPortableArchive.psm1',
        'ThirdParty/dgVoodoo2/D3D8.dll', 'ThirdParty/dgVoodoo2/DDraw.dll',
        'ThirdParty/dgVoodoo2/dgVoodooCpl.exe', 'ThirdParty/dgVoodoo2/dgVoodoo.conf',
        'ThirdParty/dgVoodoo2/REDISTRIBUTION.md', 'ThirdParty/argm-trace/winmm.dll', 'ThirdParty/argm-trace/README.md')
    foreach ($relative in $fixtureFiles) {
        $dest = Join-Path $fixtureRepo $relative
        New-Item -ItemType Directory -Path (Split-Path $dest -Parent) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $repo $relative) -Destination $dest
    }
    # Minimal PE header fixtures exercise packaging contracts; they are never executed.
    for ($i = 0; $i -lt $expectedExecutables.Count; $i++) {
        $targetDir = if ($i -lt 4) { 're_workspace/rust-editor-app-target' }
            elseif ($i -eq 4) { 're_workspace/rust-rewrite-targets/native-against-rome-modifier' }
            else { 're_workspace/rust-rewrite-targets/native-against-rome-packaging' }
        $dest = Join-Path $fixtureRepo "$targetDir/x86_64-pc-windows-msvc/release/$($expectedExecutables[$i])"
        New-Item -ItemType Directory -Path (Split-Path $dest -Parent) -Force | Out-Null
        $bytes = [byte[]]::new(512)
        [BitConverter]::GetBytes([uint16]0x5a4d).CopyTo($bytes, 0)
        [BitConverter]::GetBytes([uint32]64).CopyTo($bytes, 60)
        [BitConverter]::GetBytes([uint32]0x4550).CopyTo($bytes, 64)
        [BitConverter]::GetBytes([uint16]0x8664).CopyTo($bytes, 68)
        [BitConverter]::GetBytes([uint16]0x20b).CopyTo($bytes, 88)
        [BitConverter]::GetBytes([uint16]$(if ($i -lt 4) { 2 } else { 3 })).CopyTo($bytes, 156)
        [BitConverter]::GetBytes([uint32]16).CopyTo($bytes, 196)
        [IO.File]::WriteAllBytes($dest, $bytes)
    }
    # Fake Rust i686 build output; its bytes differ from the C++ ThirdParty/argm-trace/winmm.dll copied into the fixture above.
    $fixtureTraceDll = Join-Path $fixtureRepo 're_workspace/rust-rewrite-targets/native-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll'
    New-Item -ItemType Directory -Path (Split-Path $fixtureTraceDll -Parent) -Force | Out-Null
    $traceFixtureBytes = New-Pe32DllFixtureBytes
    [BitConverter]::GetBytes([uint32]0x52555354).CopyTo($traceFixtureBytes, 512)   # marker unique to the "Rust build output"
    [IO.File]::WriteAllBytes($fixtureTraceDll, $traceFixtureBytes)
    $cppDllHash = (Get-FileHash -LiteralPath (Join-Path $fixtureRepo 'ThirdParty/argm-trace/winmm.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
    $traceFixtureHash = (Get-FileHash -LiteralPath $fixtureTraceDll -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($cppDllHash -ceq $traceFixtureHash) { throw 'Fixture trace DLL must differ from the C++ DLL.' }
    $fixtureScript = Join-Path $fixtureRepo 'tools/publish-rust.ps1'
    $env:CARGO_HOME = $isolatedCargoHome
    New-Item -ItemType Directory -Path $fixtureOutput | Out-Null
    $sentinel = Join-Path $fixtureOutput 'user-data.txt'
    [IO.File]::WriteAllText($sentinel, 'preserve me')
    function Invoke-FixturePublish([string]$Destination, [string]$ExpectedError) {
        $result = & pwsh -NoProfile -File $fixtureScript -SkipBuild -OutputDirectory $Destination 2>&1
        $code = $LASTEXITCODE
        if ($ExpectedError) {
            if ($code -eq 0 -or ($result -join "`n") -notmatch $ExpectedError) {
                throw "Expected failure '$ExpectedError', got exit ${code}: $result"
            }
        } elseif ($code -ne 0) { throw "Fixture publish failed (exit ${code}): $result" }
    }
    Invoke-FixturePublish $fixtureOutput ''
    $stage = @(Get-ChildItem -LiteralPath $fixtureOutput -Directory)
    if ($stage.Count -ne 1) { throw 'Expected exactly one staging directory.' }
    $provenance = Get-Content -LiteralPath (Join-Path $stage[0].FullName 'provenance.json') -Raw | ConvertFrom-Json
    if ($provenance.exeProvenance -ne 'unverifiable' -or $provenance.rust.cargoHome -ne $isolatedCargoHome -or
        $provenance.fetchCommand -notmatch 'skipped' -or @($provenance.binaries.PSObject.Properties).Count -ne 6) {
        throw 'Incorrect SkipBuild provenance.'
    }
    foreach ($file in $provenance.files.PSObject.Properties) {
        if ((Get-FileHash -LiteralPath (Join-Path $stage[0].FullName $file.Name)).Hash.ToLowerInvariant() -cne $file.Value) {
            throw "Provenance checksum mismatch: $($file.Name)"
        }
    }
    if (@($provenance.files.PSObject.Properties).Count -ne 13) { throw 'Expected six binaries and seven runtime assets.' }
    $stagedTrace = Join-Path $stage[0].FullName 'ThirdParty/argm-trace/winmm.dll'
    $stagedTraceHash = (Get-FileHash -LiteralPath $stagedTrace -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($stagedTraceHash -cne $traceFixtureHash) { throw 'Staged trace DLL is not the Rust build output.' }
    if ($stagedTraceHash -ceq $cppDllHash -or $provenance.files.PSObject.Properties.Value -contains $cppDllHash) { throw 'C++ source winmm.dll leaked into the package.' }
    Assert-Pe32Dll $stagedTrace 'Staged trace DLL'
    if ($provenance.traceDll.sha256 -cne $traceFixtureHash -or $provenance.traceDll.cppSourceBinaryCopied -ne $false -or
        $provenance.traceDll.target -ne 'i686-pc-windows-msvc' -or $provenance.traceDll.cargoManifest -ne 'native/against-rome-trace/Cargo.toml' -or
        $provenance.traceDll.panicStrategy -ne 'unverifiable' -or
        $provenance.traceDll.origin -ne 'existing-rust-build-output-unverifiable') { throw 'Incorrect trace DLL provenance.' }
    $zipPath = $stage[0].FullName + '.zip'
    $beforeHash = (Get-FileHash -LiteralPath $zipPath).Hash
    $sidecar = [IO.File]::ReadAllText("$zipPath.sha256")
    if ($sidecar -cne ($beforeHash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($zipPath) + "`n")) {
        throw 'Publisher ZIP sidecar mismatch.'
    }
    $zip = [IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $expectedNames = @($provenance.files.PSObject.Properties.Name) + 'provenance.json' | Sort-Object
        if ((@($zip.Entries.FullName | Sort-Object) -join "`n") -cne ($expectedNames -join "`n")) { throw 'Publisher ZIP entry set mismatch.' }
        foreach ($entry in $zip.Entries) {
            $stream = $entry.Open()
            try { $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant() }
            finally { $stream.Dispose() }
            $expectedHash = (Get-FileHash -LiteralPath (Join-Path $stage[0].FullName $entry.FullName)).Hash.ToLowerInvariant()
            if ($hash -cne $expectedHash) { throw "ZIP content mismatch: $($entry.FullName)" }
        }
    } finally { $zip.Dispose() }
    Invoke-FixturePublish $fixtureOutput 'already exists; refusing to overwrite|Staging directory already exists'
    if ((Get-FileHash -LiteralPath $zipPath).Hash -ne $beforeHash) { throw 'Refusal changed existing archive.' }
    Write-Host '  [PASS] Full fixture publisher: six binaries, seven assets, provenance, all ZIP hashes, sidecar and no overwrite'

    foreach ($suffix in @('.zip', '.zip.sha256')) {
        $collisionOutput = Join-Path $fixtureRoot ('collision-' + $suffix.Replace('.', '_'))
        New-Item -ItemType Directory -Path $collisionOutput | Out-Null
        $collisionFile = Join-Path $collisionOutput ($stage[0].Name + $suffix)
        [IO.File]::WriteAllText($collisionFile, 'pre-existing user archive/checksum')
        Invoke-FixturePublish $collisionOutput 'already exists; refusing to overwrite'
        if ([IO.File]::ReadAllText($collisionFile) -cne 'pre-existing user archive/checksum' -or
            @(Get-ChildItem -LiteralPath $collisionOutput -Force).Count -ne 1) { throw 'Collision changed existing output.' }
    }
    foreach ($missingFile in @('re_workspace/rust-rewrite-targets/native-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll',
        're_workspace/rust-editor-app-target/x86_64-pc-windows-msvc/release/save-manager.exe')) {
        $path = Join-Path $fixtureRepo $missingFile
        $bytes = [IO.File]::ReadAllBytes($path)
        try {
            Remove-Item -LiteralPath $path
            $missingOutput = Join-Path $fixtureRoot 'missing-input'
            Invoke-FixturePublish $missingOutput 'Missing runtime asset|Release save manager exe not found|Rust trace DLL not found'
            if (Test-Path -LiteralPath $missingOutput) { throw 'Missing input created output.' }
        } finally { [IO.File]::WriteAllBytes($path, $bytes) }
    }
    Write-Host '  [PASS] Existing ZIP/checksum preserved; missing runtime asset or binary fails before staging'

    $helperPath = Join-Path $fixtureRepo 'tools/RustPortableArchive.psm1'
    $helper = [IO.File]::ReadAllText($helperPath)
    # Fail after directory/copy/provenance creation, then after a completed archive via a source race.
    foreach ($mode in @('archive-failure', 'partial-archive', 'source-race')) {
        $failureOutput = Join-Path $fixtureRoot "$mode/new/parent"
        if ($mode -eq 'archive-failure') {
            $injected = $helper.Replace('    $source = [IO.Path]::GetFullPath($SourceDirectory)', "    throw 'Injected archive failure'`n    `$source = [IO.Path]::GetFullPath(`$SourceDirectory)")
            $expectedError = 'Injected archive failure'
        } elseif ($mode -eq 'partial-archive') {
            $injected = $helper.Replace('    $source = [IO.Path]::GetFullPath($SourceDirectory)', "    [IO.File]::WriteAllText((Join-Path (Split-Path `$DestinationPath -Parent) '.rust-portable-partial.tmp.zip'), 'partial')`n    throw 'Injected partial archive failure'`n    `$source = [IO.Path]::GetFullPath(`$SourceDirectory)")
            $expectedError = 'Injected partial archive failure'
        } else {
            $injected = $helper.Replace('        [pscustomobject]@{', "        [IO.File]::WriteAllText((Join-Path (Split-Path `$PSScriptRoot -Parent) 'race.txt'), 'changed')`n        [pscustomobject]@{")
            $expectedError = 'Source tree changed during archive creation'
        }
        [IO.File]::WriteAllText($helperPath, $injected)
        Invoke-FixturePublish $failureOutput $expectedError
        if (Test-Path -LiteralPath (Join-Path $fixtureRoot $mode)) { throw "Rollback left created output ancestors: $mode" }
        if ([IO.File]::ReadAllText($sentinel) -cne 'preserve me') { throw 'Rollback changed user data.' }
    }
    [IO.File]::WriteAllText($helperPath, $helper)
    Write-Host '  [PASS] Archive failure, partial ZIP and post-archive source race roll back staging/ZIP/checksum/new ancestors'

    $invalidExe = Join-Path $fixtureRepo 're_workspace/rust-editor-app-target/x86_64-pc-windows-msvc/release/against-rome-editor.exe'
    $originalBytes = [IO.File]::ReadAllBytes($invalidExe)
    foreach ($case in @('wrong-architecture', 'wrong-subsystem', 'managed', 'truncated')) {
        $bad = [byte[]]$originalBytes.Clone()
        switch ($case) {
            'wrong-architecture' { $bad[68] = 0x4c; $bad[69] = 1 }
            'wrong-subsystem' { $bad[156] = 1 }
            'managed' { $bad[312] = 1 }
            'truncated' { $bad = [byte[]](1, 2, 3) }
        }
        [IO.File]::WriteAllBytes($invalidExe, $bad)
        $invalidOutput = Join-Path $fixtureRoot $case
        Invoke-FixturePublish $invalidOutput 'Invalid native Windows x64 executable'
        if (Test-Path -LiteralPath $invalidOutput) { throw "Invalid PE created output: $case" }
    }
    Write-Host '  [PASS] Wrong architecture/subsystem, managed CLR and truncated binaries rejected before staging'

    $validTraceBytes = [IO.File]::ReadAllBytes($fixtureTraceDll)
    [IO.File]::WriteAllBytes($invalidExe, $originalBytes)   # earlier loop left the editor exe corrupted
    try {
        foreach ($case in @('x64-machine', 'not-a-dll', 'managed', 'no-exports', 'truncated')) {
            $badBytes = switch ($case) {
                'x64-machine' { New-Pe32DllFixtureBytes -Machine 0x8664 }
                'not-a-dll' { New-Pe32DllFixtureBytes -IsDll $false }
                'managed' { New-Pe32DllFixtureBytes -ClrRva 0x2000 }
                'no-exports' { New-Pe32DllFixtureBytes -ExportRva 0 }
                'truncated' { [byte[]](1, 2, 3) }
            }
            [IO.File]::WriteAllBytes($fixtureTraceDll, $badBytes)
            $badTraceOutput = Join-Path $fixtureRoot "trace-$case"
            Invoke-FixturePublish $badTraceOutput 'Invalid native Windows 32-bit DLL'
            if (Test-Path -LiteralPath $badTraceOutput) { throw "Invalid trace DLL created output: $case" }
        }
        # Regression test: if trace DLL in target dir is the legacy C++ binary, publish must reject it before staging
        $cppBytes = [IO.File]::ReadAllBytes((Join-Path $fixtureRepo 'ThirdParty/argm-trace/winmm.dll'))
        [IO.File]::WriteAllBytes($fixtureTraceDll, $cppBytes)
        $badCppOutput = Join-Path $fixtureRoot 'trace-cpp-match'
        Invoke-FixturePublish $badCppOutput 'matches legacy C\+\+ binary'
        if (Test-Path -LiteralPath $badCppOutput) { throw 'Legacy C++ trace DLL created output: trace-cpp-match' }
    } finally { [IO.File]::WriteAllBytes($fixtureTraceDll, $validTraceBytes) }
    Write-Host '  [PASS] Non-i386, non-DLL, managed, export-less, truncated and legacy C++ trace DLLs rejected before staging'
} finally {
    $env:CARGO_HOME = $savedCargoHome
    if (Test-Path -LiteralPath $fixtureRoot) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($fixtureRoot)) -ine $tempRoot) { throw 'Unsafe fixture cleanup.' }
        Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
    }
}

# 11. Real Cargo build: the Rust trace crate produces a valid 32-bit winmm.dll cdylib (offline/locked, static CRT, temp target dir).
$traceBuildDir = Join-Path $tempRoot ("AgainstRomeTraceBuildTest-" + [Guid]::NewGuid().ToString('N'))
if ([IO.Path]::GetDirectoryName($traceBuildDir) -ine $tempRoot) { throw "Unsafe test directory: $traceBuildDir" }
$realRuntime = Join-Path $repo 're_workspace/rust-runtime'
$savedEnv = @{}
foreach ($name in 'CARGO_HOME', 'RUSTUP_HOME', 'PATH', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_PROFILE_RELEASE_PANIC') { $savedEnv[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $realCargo = Join-Path $realRuntime 'cargo/bin/cargo.exe'
    if (Test-Path -LiteralPath $realCargo) {
        $env:CARGO_HOME = Join-Path $realRuntime 'cargo'
        $env:RUSTUP_HOME = Join-Path $realRuntime 'rustup'
        $env:PATH = (Join-Path $realRuntime 'cargo/bin') + [IO.Path]::PathSeparator + $env:PATH
    } else { $realCargo = (Get-Command cargo -ErrorAction Stop).Source }
    $env:CARGO_ENCODED_RUSTFLAGS = $null
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    $env:CARGO_PROFILE_RELEASE_PANIC = 'abort'
    $traceCfg = @(& $realCargo rustc --release --locked --offline --manifest-path (Join-Path $repo 'native/against-rome-trace/Cargo.toml') --lib --target-dir $traceBuildDir --target i686-pc-windows-msvc -- --print cfg)
    if ($LASTEXITCODE -ne 0 -or $traceCfg -notcontains 'panic="abort"') { throw 'Trace release cfg must report panic="abort".' }
    $realBuildOutput = & $realCargo build --release --locked --offline --manifest-path (Join-Path $repo 'native/against-rome-trace/Cargo.toml') --lib --target-dir $traceBuildDir --target i686-pc-windows-msvc 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Rust trace crate i686 Release build failed (exit ${LASTEXITCODE}): $($realBuildOutput -join "`n")" }
    $builtTrace = Join-Path $traceBuildDir 'i686-pc-windows-msvc/release/winmm.dll'
    if (!(Test-Path -LiteralPath $builtTrace -PathType Leaf)) { throw "Cargo did not produce $builtTrace" }
    Assert-Pe32Dll $builtTrace 'Cargo-built Rust trace DLL'
    $builtHash = (Get-FileHash -LiteralPath $builtTrace -Algorithm SHA256).Hash.ToLowerInvariant()
    $cppHash = (Get-FileHash -LiteralPath (Join-Path $repo 'ThirdParty/argm-trace/winmm.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($builtHash -ceq $cppHash) { throw 'Cargo-built Rust DLL unexpectedly equals the C++ source DLL.' }
    Write-Host "  [PASS] Cargo built Rust cdylib winmm.dll (i686, PE32, exports, no CLR; SHA256 $builtHash) distinct from C++ DLL $cppHash"

    # 12. Regression test: cached/up-to-date Cargo trace build produces an identical, valid artifact without rebuild failure
    # even when unrelated documentation timestamps (e.g. README.md, HANDOFF.md) are newer than the cached artifact.
    $readmePath = Join-Path $repo 'native/against-rome-trace/README.md'
    $origReadmeTime = if (Test-Path -LiteralPath $readmePath -PathType Leaf) { (Get-Item -LiteralPath $readmePath).LastWriteTime } else { $null }
    try {
        if ($null -ne $origReadmeTime) {
            # Advance documentation mtime beyond the existing build artifact to simulate editing notes/docs
            (Get-Item -LiteralPath $readmePath).LastWriteTime = (Get-Date).AddMinutes(2)
        }
        $cachedBuildOutput = & $realCargo build --release --locked --offline --manifest-path (Join-Path $repo 'native/against-rome-trace/Cargo.toml') --lib --target-dir $traceBuildDir --target i686-pc-windows-msvc 2>&1
        if ($LASTEXITCODE -ne 0) { throw "Cached Rust trace crate build failed (exit ${LASTEXITCODE}): $($cachedBuildOutput -join "`n")" }
        $cachedHash = (Get-FileHash -LiteralPath $builtTrace -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($cachedHash -cne $builtHash) { throw 'Cached trace build output hash changed unexpectedly.' }
        Assert-Pe32Dll $builtTrace 'Cached Cargo-built Rust trace DLL'
        if ($null -ne $origReadmeTime -and (Get-Item -LiteralPath $builtTrace).LastWriteTime -ge (Get-Item -LiteralPath $readmePath).LastWriteTime) {
            throw 'Expected cached trace artifact to predate updated documentation timestamp.'
        }
        Write-Host "  [PASS] Cached/up-to-date Cargo trace build verified despite newer documentation timestamp (retained valid SHA256 $cachedHash)"
    } finally {
        if ($null -ne $origReadmeTime) {
            (Get-Item -LiteralPath $readmePath).LastWriteTime = $origReadmeTime
        }
    }
} finally {
    foreach ($name in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnv[$name]) }
    if (Test-Path -LiteralPath $traceBuildDir) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($traceBuildDir)) -ine $tempRoot) { throw "Refusing recursive cleanup outside TEMP: $traceBuildDir" }
        Remove-Item -LiteralPath $traceBuildDir -Recurse -Force
    }
}

Write-Host "All publish-rust.ps1 tests passed successfully."
