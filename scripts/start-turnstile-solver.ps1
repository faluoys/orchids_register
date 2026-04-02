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
$condaEnv = [string]$config['conda_env']
$solver = Get-Section -Config $config -Name 'turnstile_solver'

$workDir = Resolve-RepoPath -RepoRoot $repoRoot -PathValue 'TurnstileSolver'
$condaArgs = @(
    'run', '-n', $condaEnv,
    'python', 'api_solver.py',
    '--host', ([string]$solver['host']),
    '--port', ([string]$solver['port']),
    '--thread', ([string]$solver['thread']),
    '--browser_type', ([string]$solver['browser_type'])
)

if (-not [bool]$solver['headless']) {
    $condaArgs += '--no-headless'
}
if ([bool]$solver['debug']) {
    $condaArgs += '--debug'
}
if ([bool]$solver['proxy']) {
    $condaArgs += '--proxy'
}
if ([bool]$solver['random']) {
    $condaArgs += '--random'
}

Write-Host "Config file: $resolvedConfigPath"
Write-Host "TurnstileSolver workdir: $workDir"
Write-Host ("Command: conda " + (Format-CommandPreview -Parts $condaArgs))

if ($DryRun) {
    return
}

Set-Location -LiteralPath $workDir
& conda @condaArgs
