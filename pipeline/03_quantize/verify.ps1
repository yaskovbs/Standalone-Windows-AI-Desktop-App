# Phase 3 - Smoke test: measure tokens/sec and memory using llama-cli.exe.
# Usage: .\verify.ps1 -LlamaCpp C:\src\llama.cpp -Model ..\output\gguf\custom_assistant_q4_k_m.gguf
param(
    [Parameter(Mandatory = $true)][string]$LlamaCpp,
    [Parameter(Mandatory = $true)][string]$Model,
    [int]$GpuLayers = 99
)

$ErrorActionPreference = "Stop"
$cli = Join-Path $LlamaCpp "build\bin\Release\llama-cli.exe"
$prompt = "Think step by step: what is 17 * 24?"

$proc = Start-Process -FilePath $cli -NoNewWindow -PassThru -RedirectStandardOutput "verify_out.txt" `
    -RedirectStandardError "verify_err.txt" `
    -ArgumentList @("-m", $Model, "-p", "`"$prompt`"", "-n", "128", "-ngl", $GpuLayers, "--no-display-prompt", "-no-cnv")

$peak = 0
while (-not $proc.HasExited) {
    $proc.Refresh()
    $peak = [math]::Max($peak, $proc.WorkingSet64)
    Start-Sleep -Milliseconds 250
}

Write-Host "Peak working set: $([math]::Round($peak / 1MB)) MB"
Select-String -Path "verify_err.txt" -Pattern "eval time|tokens per second" | ForEach-Object { $_.Line }
Write-Host "--- model output ---"
Get-Content "verify_out.txt"
