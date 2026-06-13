# ==========================================================================
# File    : ascope.ps1
# Project : AuraScope
# Layer   : Shell
# Purpose : PowerShell wrapper that captures the final AuraScope directory and
#           applies it to the caller session.
#
# Author  : Ahmed Ashour
# Created : 2026-06-13
# ==========================================================================

function Invoke-AuraScopeWarp {
    $tmpFile = [System.IO.Path]::GetTempFileName()

    try {
        & ascope.exe --export-target $tmpFile @args
        if (Test-Path $tmpFile) {
            $targetDir = (Get-Content -Raw $tmpFile).Trim()
            if ($targetDir -and (Test-Path $targetDir -PathType Container)) {
                Set-Location $targetDir
            }
        }
    }
    finally {
        if (Test-Path $tmpFile) {
            Remove-Item $tmpFile -Force
        }
    }
}

Set-Alias asc Invoke-AuraScopeWarp
