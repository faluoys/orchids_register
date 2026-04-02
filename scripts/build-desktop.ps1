param(
    [string]$ConfigPath,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common.ps1')

$config = Get-RuntimeConfig -ConfigPath $ConfigPath
$repoRoot = $config['_meta']['repo_root']
$resolvedConfigPath = $config['_meta']['config_path']
$workDir = Resolve-RepoPath -RepoRoot $repoRoot -PathValue 'src-tauri'
$commandParts = @('cargo', 'tauri', 'build')

Write-Host "Config file: $resolvedConfigPath"
Write-Host "Desktop build workdir: $workDir"
Write-Host ("Command: " + (Format-CommandPreview -Parts $commandParts))

if ($DryRun) {
    return
}

Set-Location -LiteralPath $workDir
& cargo tauri build
