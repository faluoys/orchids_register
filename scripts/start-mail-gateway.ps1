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
$mailGateway = Get-Section -Config $config -Name 'mail_gateway'

$workDir = Resolve-RepoPath -RepoRoot $repoRoot -PathValue 'mail-gateway'
$dbPath = Resolve-RepoPath -RepoRoot $repoRoot -PathValue ([string]$mailGateway['database_path'])
$dbDir = Split-Path -Parent $dbPath
if (-not (Test-Path -LiteralPath $dbDir)) {
    New-Item -ItemType Directory -Force -Path $dbDir | Out-Null
}

$env:MAIL_GATEWAY_DB = $dbPath
$env:LUCKMAIL_BASE_URL = [string]$mailGateway['luckmail_base_url']
$env:LUCKMAIL_API_KEY = [string]$mailGateway['luckmail_api_key']

$condaArgs = @(
    'run', '-n', $condaEnv,
    'python', '-m', 'uvicorn', 'mail_gateway.app:app',
    '--host', ([string]$mailGateway['host']),
    '--port', ([string]$mailGateway['port'])
)

Write-Host "Config file: $resolvedConfigPath"
Write-Host "mail-gateway workdir: $workDir"
Write-Host "MAIL_GATEWAY_DB=$dbPath"
if ($env:LUCKMAIL_API_KEY -match 'REPLACE_WITH_REAL|your-real-key') {
    Write-Warning 'luckmail_api_key is still a placeholder. /health will likely not show enabled.'
}
Write-Host ("Command: conda " + (Format-CommandPreview -Parts $condaArgs))

if ($DryRun) {
    return
}

Set-Location -LiteralPath $workDir
& conda @condaArgs
