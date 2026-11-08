# Phase 3 - Convert the merged HF model to GGUF and quantize it (Windows PowerShell).
# Usage: .\quantize.ps1 -LlamaCpp C:\src\llama.cpp [-Quant Q4_K_M]
param(
    [Parameter(Mandatory = $true)][string]$LlamaCpp,
    [string]$Merged = "..\output\merged_model",
    [string]$OutDir = "..\output\gguf",
    [ValidateSet("Q4_K_M", "Q5_K_M", "Q8_0")][string]$Quant = "Q4_K_M"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$f16 = Join-Path $OutDir "model_f16.gguf"
$out = Join-Path $OutDir ("custom_assistant_" + $Quant.ToLower() + ".gguf")

Write-Host "==> Converting to GGUF (f16)"
python (Join-Path $LlamaCpp "convert_hf_to_gguf.py") $Merged --outfile $f16 --outtype f16

Write-Host "==> Quantizing to $Quant"
& (Join-Path $LlamaCpp "build\bin\Release\llama-quantize.exe") $f16 $out $Quant

Write-Host "==> Done: $out"
Get-Item $out | Select-Object Name, @{n = "SizeGB"; e = { [math]::Round($_.Length / 1GB, 2) } }
