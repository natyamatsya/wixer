param
(
    [string]$s,  # stage
    [string]$p,  # property value
    [string]$v,  # version
    [string]$l   # install location
)

Push-Location $PSScriptRoot

$fileName = "InstallationStages.txt"
$time = Get-Date -Format "HH:mm:ss.fff"
$l = $l.Trim()

Add-Content -Path $fileName -Value "$time - $l - $v - $p - $s"

Pop-Location