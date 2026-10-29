param(
    [Parameter(Mandatory=$true)][string]$DistributionDirectory,
    [Parameter(Mandatory=$true)][string]$SigningInputsDirectory
)
$ErrorActionPreference = 'Stop'
$distribution = (Resolve-Path $DistributionDirectory).Path
$inputs = (Resolve-Path $SigningInputsDirectory).Path
$manifest = Get-Content "$distribution/windows/RELEASE-MANIFEST.json" -Raw | ConvertFrom-Json
$version = $manifest.version
$setup = "$distribution/KohdaLab-Camera-$version-Setup.exe"
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run this disposable-machine validation in elevated PowerShell.' }
$install = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Kohdalab\KohdaLab Camera'
$productKey = 'HKLM:\Software\KohdaLab\Camera'
$uninstallKey = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KohdaLab Camera'
if ((Test-Path -LiteralPath $install) -or (Test-Path -LiteralPath $productKey) -or (Test-Path -LiteralPath $uninstallKey)) { throw 'Refusing to validate over an existing Camera installation.' }

function Assert-Metadata([string]$Path) {
    $file = Get-Item -LiteralPath $Path
    if ($file.VersionInfo.ProductName -ne 'KohdaLab Camera' -or
        $file.VersionInfo.ProductVersion -ne $version -or
        $file.VersionInfo.FileVersion -ne $version) {
        throw "Executable metadata mismatch: $($file.Name)"
    }
    $signature = (Get-AuthenticodeSignature -LiteralPath $Path).Status.ToString()
    if ($signature -ne 'NotSigned') { throw "Unexpected unsigned-build signature status: $signature" }
    [pscustomobject]@{name=$file.Name;product=$file.VersionInfo.ProductName;version=$file.VersionInfo.ProductVersion;signature=$signature;sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
}

$metadata = @(
    Assert-Metadata "$distribution/windows/KohdaLab Camera.exe"
    Assert-Metadata "$distribution/windows/continuous-helper.exe"
    Assert-Metadata "$inputs/Uninstall KohdaLab Camera.exe"
    Assert-Metadata $setup
)
$p = Start-Process -FilePath $setup -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "Install exit $($p.ExitCode)" }
foreach ($entry in $manifest.runtime.PSObject.Properties) {
    if ((Get-FileHash -LiteralPath (Join-Path $install $entry.Name) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.Value) { throw 'Installed runtime mismatch' }
}
if ((Get-FileHash "$install/Uninstall KohdaLab Camera.exe").Hash -ne (Get-FileHash "$inputs/Uninstall KohdaLab Camera.exe").Hash) { throw 'Exported uninstaller differs from installed bytes' }
$startup = Join-Path $distribution 'startup-report.json'
$p = Start-Process -FilePath "$install/KohdaLab Camera.exe" -ArgumentList @('--startup-report',"`"$startup`"") -Wait -PassThru
if ($p.ExitCode -ne 0) { throw 'Startup report failed' }
$report = Get-Content $startup -Raw | ConvertFrom-Json
if ($report.version -ne $version -or !$report.helper_present -or $report.usb_opens -ne 0 -or $report.helper_starts -ne 0) { throw 'Unexpected startup activity' }
Set-Content "$install/user-sentinel.txt" 'preserve-user-file'
$p = Start-Process -FilePath $setup -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0 -or (Get-Content "$install/user-sentinel.txt") -ne 'preserve-user-file') { throw 'Update failed' }
$p = Start-Process -FilePath "$install/Uninstall KohdaLab Camera.exe" -ArgumentList '/S' -Wait -PassThru
# NSIS starts a temporary uninstaller process. Wait for its actual file removal.
$deadline = [DateTime]::UtcNow.AddSeconds(60)
while ((Test-Path "$install/KohdaLab Camera.exe") -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 500 }
if ((Test-Path "$install/KohdaLab Camera.exe") -or !(Test-Path "$install/user-sentinel.txt")) { throw 'Uninstall or user-file preservation failed' }
$validation = [ordered]@{version=$version;scope='Disposable Windows CI installation and enumeration only; no physical capture';metadata=$metadata;runtime_hashes_match=$true;exported_uninstaller_matches=$true;fresh_install=$true;update=$true;uninstall=$true;unrelated_file_preserved=$true;usb_opens=0;helper_starts=0}
$validation | ConvertTo-Json -Depth 6 | Set-Content "$distribution/ci-validation.json" -Encoding utf8
