# RelAudio — Windows derleme betiği.
# Kaynağı Windows'a kopyaladıktan sonra bu dosyayı proje kökünde çalıştır.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
#
# Gerekenler: Rust (rustup, msvc), Node.js, Visual C++ Build Tools + Windows SDK.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "== gereksinim kontrolu ==" -ForegroundColor Cyan
foreach ($c in @("cargo", "node", "npm")) {
    if (-not (Get-Command $c -ErrorAction SilentlyContinue)) {
        throw "$c bulunamadi. Kurulum icin docs/11-calistirma.md"
    }
    Write-Host ("  {0,-6} {1}" -f $c, (& $c --version | Select-Object -First 1))
}

# MSVC linker'i ve Windows SDK yollarini kur. Rust bunlari kendi bulmaya
# calisir ama VS Insiders gibi onizleme surumlerinde basarisiz olabiliyor.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vs = & $vswhere -prerelease -latest -products * `
          -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
          -property installationPath 2>$null | Select-Object -First 1
    if ($vs) {
        $devShell = Join-Path $vs "Common7\Tools\Launch-VsDevShell.ps1"
        if (Test-Path $devShell) {
            Write-Host "== VS gelistirme ortami yukleniyor ==" -ForegroundColor Cyan
            & $devShell -Arch amd64 -HostArch amd64 | Out-Null
            Set-Location $root
        }
    }
}

Write-Host "== komut satiri araci (teshis icin) ==" -ForegroundColor Cyan
cargo build --release --bin relaudio-cli
if ($LASTEXITCODE -ne 0) { throw "CLI build failed (exit $LASTEXITCODE)." }

Write-Host "== npm bagimliliklari ==" -ForegroundColor Cyan
Set-Location (Join-Path $root "app")
npm install

Write-Host "== derleme ==" -ForegroundColor Cyan
npx tauri build --no-bundle
# Eski bir exe duruyorsa Test-Path başarılı görünüyordu; asıl ölçüt çıkış kodu.
if ($LASTEXITCODE -ne 0) { throw "Build failed (exit $LASTEXITCODE)." }

$exe = Join-Path $root "app\src-tauri\target\release\relaudio-app.exe"
$cli = Join-Path $root "target\release\relaudio-cli.exe"
if (Test-Path $exe) {
    Write-Host ""
    Write-Host "TAMAM" -ForegroundColor Green
    Write-Host "  Uygulama : $exe"
    if (Test-Path $cli) { Write-Host "  CLI      : $cli" }
    Write-Host ""
    Write-Host "Cikis aygitini sinamak icin (agdan bagimsiz):" -ForegroundColor Cyan
    Write-Host "  & '$cli' devices"
    Write-Host "  & '$cli' tone"
} else {
    throw "Derleme bitti ama exe bulunamadi."
}
