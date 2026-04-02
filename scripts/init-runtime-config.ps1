param(
    [switch]$Force,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'common.ps1')

$repoRoot = Get-RepoRoot
$paths = Get-DefaultRuntimePaths -RepoRoot $repoRoot
$localPath = [string]$paths['local_path']
$examplePath = [string]$paths['example_path']

Write-Host "Template config: $examplePath"
Write-Host "Local config: $localPath"

if ((Test-Path -LiteralPath $localPath) -and -not $Force) {
    Write-Host 'runtime.local.yaml already exists. Use -Force to overwrite it from the template.'
    if ($DryRun) {
        return
    }
}

$result = Initialize-RuntimeLocalConfig -RepoRoot $repoRoot -Force:$Force -DryRun:$DryRun

if ($DryRun) {
    if ($Force) {
        Write-Host 'DryRun: runtime.local.yaml would be overwritten from runtime.example.yaml'
    } elseif ([bool]$result['created']) {
        Write-Host 'DryRun: runtime.local.yaml would be created from runtime.example.yaml'
    } else {
        Write-Host 'DryRun: no file change'
    }
    return
}

if ($Force) {
    Write-Host 'runtime.local.yaml overwritten from runtime.example.yaml'
} elseif ([bool]$result['created']) {
    Write-Host 'runtime.local.yaml created from runtime.example.yaml'
} else {
    Write-Host 'No file change'
}
