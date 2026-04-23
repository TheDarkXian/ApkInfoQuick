$ErrorActionPreference = "SilentlyContinue"

function Test-Command {
    param([string]$Name)
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    return $null -ne $cmd
}

function Write-Status {
    param(
        [string]$Label,
        [bool]$Ok,
        [string]$Detail
    )
    if ($Ok) {
        Write-Output ("[OK]   {0} - {1}" -f $Label, $Detail)
    }
    else {
        Write-Output ("[MISS] {0} - {1}" -f $Label, $Detail)
    }
}

Write-Output "ApkInfoQuick Frontend Doctor"
Write-Output "----------------------------"

$nodeOk = Test-Command "node"
$npmOk = Test-Command "npm.cmd"
$tauriOk = Test-Command "tauri"
$cargoOk = Test-Command "cargo"
$rustupOk = Test-Command "rustup"

$nodeVersion = if ($nodeOk) { (node -v) } else { "not found" }
$npmVersion = if ($npmOk) { (npm.cmd -v) } else { "not found" }
$tauriVersion = if ($tauriOk) { (tauri -V) } else { "not found" }
$cargoVersion = if ($cargoOk) { (cargo --version) } else { "not found" }
$rustupVersion = if ($rustupOk) { (rustup --version) } else { "not found" }

Write-Status "Node.js" $nodeOk $nodeVersion
Write-Status "npm" $npmOk $npmVersion
Write-Status "Tauri CLI" $tauriOk $tauriVersion
Write-Status "cargo" $cargoOk $cargoVersion
Write-Status "rustup" $rustupOk $rustupVersion

$webview2Version = $null
$webviewKeys = @(
    "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F1A4E5E3-93D1-4F9A-B2A6-E7D1C9A0E3AA}",
    "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F1A4E5E3-93D1-4F9A-B2A6-E7D1C9A0E3AA}",
    "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)

foreach ($key in $webviewKeys) {
    try {
        $pv = Get-ItemPropertyValue -Path $key -Name "pv"
        if ($pv) {
            $webview2Version = $pv
            break
        }
    }
    catch {}
}

$webviewOk = -not [string]::IsNullOrWhiteSpace($webview2Version)
Write-Status "WebView2 Runtime" $webviewOk ($(if ($webviewOk) { $webview2Version } else { "not detected" }))

Write-Output ""
if (-not $cargoOk -or -not $rustupOk) {
    Write-Output "Rust toolchain is missing. Install with:"
    Write-Output "  winget install Rustlang.Rustup"
    Write-Output "Then restart terminal and run:"
    Write-Output "  npm.cmd run doctor"
    Write-Output "  npm.cmd run tauri:dev"
}
else {
    Write-Output "Environment looks ready for Tauri development."
    Write-Output "Next: npm.cmd run tauri:dev"
}
