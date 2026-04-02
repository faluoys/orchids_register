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
$orchids = Get-Section -Config $config -Name 'orchids'

$workDir = $repoRoot
$resultJson = 'register_result.json'
if ($orchids.ContainsKey('result_json')) {
    $resultJson = [string]$orchids['result_json']
}

$cargoArgs = @(
    'run', '--bin', 'orchids-auto-register', '--',
    '--mail-mode', ([string]$orchids['mail_mode']),
    '--mail-gateway-base-url', ([string]$orchids['mail_gateway_base_url']),
    '--mail-provider', ([string]$orchids['mail_provider']),
    '--mail-provider-mode', ([string]$orchids['mail_provider_mode']),
    '--mail-project-code', ([string]$orchids['mail_project_code']),
    '--use-capmonster',
    '--captcha-api-url', ([string]$orchids['captcha_api_url']),
    '--poll-timeout', ([string]$orchids['poll_timeout']),
    '--poll-interval', ([string]$orchids['poll_interval']),
    '--result-json', $resultJson
)

Write-Host "Config file: $resolvedConfigPath"
Write-Host "CLI workdir: $workDir"
Write-Host ("Command: cargo " + (Format-CommandPreview -Parts $cargoArgs))

if ($DryRun) {
    return
}

Set-Location -LiteralPath $workDir
& cargo @cargoArgs
