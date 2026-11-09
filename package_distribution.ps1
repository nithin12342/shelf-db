# package_distribution.ps1 - Automated build and package script for SheafDB
$ErrorActionPreference = "Stop"
Write-Host "Invoking SheafDB packaging pipeline..."
py -3 "$PSScriptRoot\package_distribution.py"
