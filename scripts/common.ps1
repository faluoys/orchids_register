Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RepoRoot {
    return [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
}

function Resolve-RepoPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [Parameter(Mandatory = $true)]
        [string]$PathValue
    )

    if ([System.IO.Path]::IsPathRooted($PathValue)) {
        return [System.IO.Path]::GetFullPath($PathValue)
    }

    return [System.IO.Path]::GetFullPath((Join-Path $RepoRoot $PathValue))
}

function Get-DefaultRuntimePaths {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $configDir = Join-Path $RepoRoot 'config'
    return @{
        config_dir = $configDir
        local_path = Join-Path $configDir 'runtime.local.yaml'
        example_path = Join-Path $configDir 'runtime.example.yaml'
    }
}

function Initialize-RuntimeLocalConfig {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [switch]$Force,
        [switch]$DryRun
    )

    $paths = Get-DefaultRuntimePaths -RepoRoot $RepoRoot
    $configDir = [string]$paths['config_dir']
    $localPath = [string]$paths['local_path']
    $examplePath = [string]$paths['example_path']

    if (-not (Test-Path -LiteralPath $examplePath)) {
        throw "Missing template config: $examplePath"
    }

    if ((Test-Path -LiteralPath $localPath) -and -not $Force) {
        return @{
            created = $false
            overwritten = $false
            path = $localPath
            source = $examplePath
        }
    }

    if (-not (Test-Path -LiteralPath $configDir)) {
        if (-not $DryRun) {
            New-Item -ItemType Directory -Force -Path $configDir | Out-Null
        }
    }

    if (-not $DryRun) {
        Copy-Item -LiteralPath $examplePath -Destination $localPath -Force:$Force
    }

    return @{
        created = $true
        overwritten = [bool](Test-Path -LiteralPath $localPath)
        path = $localPath
        source = $examplePath
    }
}

function Convert-YamlScalar {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$RawValue
    )

    $value = $RawValue.Trim()
    if ($value.Length -ge 2) {
        if (($value.StartsWith('"') -and $value.EndsWith('"')) -or ($value.StartsWith("'") -and $value.EndsWith("'"))) {
            $value = $value.Substring(1, $value.Length - 2)
        }
    }

    if ($value -match '^(true|false)$') {
        return [System.Convert]::ToBoolean($value)
    }

    if ($value -match '^-?\d+$') {
        return [int]$value
    }

    return $value
}

function Read-SimpleYaml {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "YAML file not found: $Path"
    }

    $config = @{}
    $currentSection = $null

    foreach ($rawLine in Get-Content -LiteralPath $Path) {
        $line = $rawLine.TrimEnd()
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        if ($line.TrimStart().StartsWith('#')) {
            continue
        }

        if ($line -match '^\s{2}(?<key>[A-Za-z0-9_-]+):\s*(?<value>.*)$') {
            if ($null -eq $currentSection) {
                throw "Nested key found without section: $rawLine"
            }
            $config[$currentSection][$matches['key']] = Convert-YamlScalar $matches['value']
            continue
        }

        if ($line -match '^(?<key>[A-Za-z0-9_-]+):\s*(?<value>.*)$') {
            $key = $matches['key']
            $valueText = $matches['value']
            if ([string]::IsNullOrWhiteSpace($valueText)) {
                $currentSection = $key
                if (-not $config.ContainsKey($key)) {
                    $config[$key] = @{}
                }
            } else {
                $config[$key] = Convert-YamlScalar $valueText
                $currentSection = $null
            }
            continue
        }

        throw "Unsupported YAML line: $rawLine"
    }

    return $config
}

function Get-RuntimeConfig {
    param(
        [string]$ConfigPath
    )

    $repoRoot = Get-RepoRoot

    if (-not [string]::IsNullOrWhiteSpace($ConfigPath)) {
        $resolvedPath = Resolve-RepoPath -RepoRoot $repoRoot -PathValue $ConfigPath
        if (-not (Test-Path -LiteralPath $resolvedPath)) {
            throw "Runtime config not found: $resolvedPath"
        }

        $config = Read-SimpleYaml -Path $resolvedPath
        $config['_meta'] = @{
            repo_root = $repoRoot
            config_path = $resolvedPath
        }
        return $config
    }

    $paths = Get-DefaultRuntimePaths -RepoRoot $repoRoot
    $localPath = [string]$paths['local_path']
    $examplePath = [string]$paths['example_path']

    if (-not (Test-Path -LiteralPath $localPath)) {
        $result = Initialize-RuntimeLocalConfig -RepoRoot $repoRoot
        if ([bool]$result['created']) {
            Write-Host "Created local runtime config from template: $localPath"
        }
    }

    $selectedPath = $localPath
    if (-not (Test-Path -LiteralPath $selectedPath)) {
        if (Test-Path -LiteralPath $examplePath) {
            $selectedPath = $examplePath
        } else {
            throw 'No runtime config found. Expected config/runtime.local.yaml or config/runtime.example.yaml'
        }
    }

    $config = Read-SimpleYaml -Path $selectedPath
    $config['_meta'] = @{
        repo_root = $repoRoot
        config_path = $selectedPath
    }
    return $config
}

function Format-CommandPreview {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Parts
    )

    return ($Parts | ForEach-Object {
        if ($_ -match '\s') {
            '"' + $_ + '"'
        } else {
            $_
        }
    }) -join ' '
}

function Get-Section {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Config,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if (-not $Config.ContainsKey($Name)) {
        throw "Missing config section: $Name"
    }

    return $Config[$Name]
}
