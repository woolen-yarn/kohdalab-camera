# Windows installation, drivers and signing

KohdaLab Camera separates application installation from camera driver setup.
The application runs as the current user. Neither startup nor Setup changes a
Windows driver, service, security setting or PowerShell execution policy.

## Installed driver policy

KohdaLab IV prefers an installed official VISA runtime and does not silently
fall back to WinUSB. Camera follows the same preservation principle, but a
camera SDK is a separate interface from VISA and requires its own documented
backend.

For the tested SHODENSHA GR300, the current acquisition backend requires an
existing WinUSB binding. If a different driver is bound, Camera preserves it,
explains that a compatible vendor SDK backend is not available, and disables
Load. The acquisition helper checks the binding again before opening USB.
Unknown, missing and composite bindings are not treated as WinUSB by guesswork.
The presence of a driver package somewhere on disk does not make it accessible
through the active binding.

Official SDK integration is deferred at the owner's request. Using the official
GR300 driver for acquisition still requires manufacturer API
specifications, supported architecture, SDK access and redistribution terms.
No manufacturer DLL is probed by guessed entry points or reverse engineered.
The manufacturer's available [GR300-family setup instructions](https://www.shodensha-inc.co.jp/microscope/gr300bcm3_20180219.pdf)
explain the supplied viewer; they do not establish a redistributable API for the
tested unit. Other GR300 revisions must not be assumed compatible.

[Microsoft's USB desktop guidance](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/windows-desktop-app-for-a-usb-device)
explains that a WinUSB application uses Winusb.sys as the device driver. A vendor
driver requires the interface that driver exposes; these transports are not
interchangeable merely because the USB ID is the same.

## Startup

Startup loads bounded saved preferences and enumerates cameras asynchronously.
A single matching device is selected automatically; acquisition starts only
when Load is pressed. No elevation, installer, driver replacement, network
fetch, certificate-store update or security bypass runs at startup.

The helper is resolved beside the executable, without searching PATH. A missing
helper is rejected before creating a physical-session interlock. Protective
locks and uncertain cleanup continue to block reconnection; the installer does
not remove them. Keep the complete application folder together.

For a support report without opening a camera or launching the helper:

```powershell
& '.\KohdaLab Camera.exe' --startup-report '.\startup-report.json'
```

The output path must be new. The report includes application version, enumerated
matching camera bindings and helper presence. It is an enumeration/readiness
report, not a successful capture or signature certificate.

## Installer and portable ZIP

The NSIS installer targets Windows x64 and installs per user under
`%ProgramFiles%\Kohdalab\KohdaLab Camera`. It creates desktop/Start Menu shortcuts
and an Apps uninstall entry under HKCU. It installs the application, helper and
license notices, without driver installation or automatically starting Live.
User settings, captures and device interlocks are separate from program files.

The portable ZIP remains an option. Obtain Setup or ZIP from the
[official releases](https://github.com/woolen-yarn/kohdalab-camera/releases), verify
its SHA-256, and retain the helper and license files. A checksum verifies byte
identity against the published reference; it does not replace publisher signing.

Build from a verified `package_windows.py` payload using an explicit NSIS compiler:

```sh
python scripts/build_windows_installer.py --payload dist/windows \
  --output dist/KohdaLab-Camera-Setup.exe --makensis /path/to/makensis
```

The build tool verifies runtime hashes and license presence. It does not download
or install a compiler. [NSIS](https://nsis.sourceforge.io/Docs/Chapter4.html)
`RequestExecutionLevel user` avoids an administrative setup requirement.

## Verified installation

Version 0.1.11 additionally passed [public Windows CI](https://github.com/woolen-yarn/kohdalab-camera/actions/runs/39187983298),
including metadata/unsigned-status checks on all executable types, matching
exported/installed uninstaller bytes, fresh install, update, uninstall and
unrelated-file preservation. The exact CI runtime was installed and startup-
checked on the workstation. See [v0.1.11 validation](validation-0.1.11.json).
This remains an unsigned release without new physical capture qualification.

Version 0.1.10 Setup was compiled with NSIS 3.12 using zlib after checking all
687 build inputs. Fresh per-user install, same-version update and uninstall
passed on Windows; an unrelated test file survived both update and uninstall.
The final per-user installation matched both runtime hashes and its desktop
shortcut targeted the installed application. The installed startup report found
the GR300 WinUSB binding and sibling helper, without starting acquisition.
See the [scoped installation record](validation-0.1.10.json). This does not extend
the version 0.1.9 physical Live validation to new binaries.

## Signature audit and execution warnings

The inspected version 0.1.10 GUI, helper, uninstaller and Setup are **NotSigned**. Neither the user nor
machine personal certificate store contained a code-signing certificate with a
private key. The checked files had no Zone.Identifier stream, so this local test
cannot establish downloaded-file SmartScreen behavior.

The latest inspected 200 events per available log within seven days contained no
Camera match in Code Integrity, AppLocker or Defender logs. SmartScreen Debug
was disabled. Missing matching events do not prove that a warning cannot occur.
Do not change those log settings or protection policies to make a release pass.

Audit a prepared payload or installed application with:

```powershell
.\scripts\audit_windows_distribution.ps1 -PayloadDirectory '.\windows' `
  -OutputJson '.\signature-report.json' -RequireSigned
```

The test machine blocks `.ps1` files under its existing execution policy; that
policy was preserved. On such systems use the standard read-only command
`Get-AuthenticodeSignature -LiteralPath ".\KohdaLab Camera.exe"` interactively,
or run the audit script in an organization-approved signed-script environment.
Setup does not require PowerShell.

`-RequireSigned` fails unless every executable/DLL has a Valid Authenticode
signature. Inspect the publisher identity and timestamp as well. Without this
switch, the report records unsigned status without claiming signing readiness.

For a signed release, use a publisher-owned certificate or signing service:

1. Build and sign both application executables and any future bundled DLLs.
2. Verify signatures, publisher identity and timestamps, then generate the runtime
   manifest from those final signed bytes.
3. Compile the installer with an uninstaller-signing hook; sign and verify the
   generated uninstaller and final Setup executable.
4. Publish checksums only after all signing steps. Keep private keys and service
   credentials outside source, logs and distribution archives.

Current releases remain unsigned. The owner has requested preparation for the
free SignPath Foundation program, without submitting an application yet. Its
certificate publisher would be SignPath Foundation, not the laboratory or
university. See the [Code signing policy](code-signing-policy.md) and
[application preparation](signpath-application.md). A laboratory/organization
certificate remains a separate alternative requiring organization validation
and enrollment. No enrollment, purchase or credential change was performed.
The current installer does not itself provide a signing identity. A signed
release must qualify the full signing pipeline, including the NSIS uninstaller;
an unsigned build must remain labelled unsigned.

[Microsoft's signing comparison](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
describes Store/MSIX, certificate and Artifact Signing options and geographic
eligibility. [SmartScreen reputation guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
explains that even valid signatures do not guarantee immediate reputation, and
EV certificates no longer provide an automatic bypass. Smart App Control and
enterprise policy can impose additional blocks. The application cannot repair
a block that prevents it from starting. Preserve the exact warning, file hash
and signature report for diagnosis; do not disable protection or silently remove
Mark of the Web as an installation workaround.
