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

$jobs = @(
    @{ Name = 'mail-gateway'; Script = (Join-Path $PSScriptRoot 'start-mail-gateway.ps1') },
    @{ Name = 'turnstile-solver'; Script = (Join-Path $PSScriptRoot 'start-turnstile-solver.ps1') },
    @{ Name = 'desktop-dev'; Script = (Join-Path $PSScriptRoot 'start-desktop-dev.ps1') }
)

Write-Host "Config file: $resolvedConfigPath"
Write-Host 'Will open 3 windows: mail-gateway, TurnstileSolver, desktop dev'

foreach ($job in $jobs) {
    $argList = @(
        '-NoExit',
        '-NoProfile',
        '-ExecutionPolicy', 'Bypass',
        '-File', $job.Script,
        '-ConfigPath', $resolvedConfigPath
    )

    Write-Host (("Launch [{0}] -> powershell.exe " -f $job.Name) + (Format-CommandPreview -Parts $argList))

    if (-not $DryRun) {
        Start-Process -FilePath 'powershell.exe' -WorkingDirectory $repoRoot -ArgumentList $argList | Out-Null
        Start-Sleep -Milliseconds 400
    }
}
