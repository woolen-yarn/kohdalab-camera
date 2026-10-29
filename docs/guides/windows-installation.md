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

The NSIS installer targets Windows x64 and installs for all users under
`%ProgramFiles%\Kohdalab\KohdaLab Camera`. It creates desktop/Start Menu shortcuts
and an Apps uninstall entry under HKLM. It installs the application, helper and
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
`RequestExecutionLevel admin` requires administrator approval for Setup. The application runs as the launching user. Setup keeps the installation directory fixed; `/D` cannot redirect installation or uninstallation to another application.

## Verified installation

Version 0.1.11 additionally passed [public Windows CI](https://github.com/woolen-yarn/kohdalab-camera/actions/runs/39187983298),
including metadata/unsigned-status checks on all executable types, matching
exported/installed uninstaller bytes, fresh install, update, uninstall and
unrelated-file preservation. The exact CI runtime was installed and startup-
checked on the workstation. See [v0.1.11 validation](../validation/validation-0.1.11.json).
This remains an unsigned release without new physical capture qualification.

Version 0.1.10 Setup was compiled with NSIS 3.12 using zlib after checking all
687 build inputs. Fresh machine-wide install, same-version update and uninstall
passed on Windows; an unrelated test file survived both update and uninstall.
The final machine-wide installation matched both runtime hashes and its desktop
shortcut targeted the installed application. The installed startup report found
the GR300 WinUSB binding and sibling helper, without starting acquisition.
See the [scoped installation record](../validation/validation-0.1.10.json). This does not extend
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
university. See the [Code signing policy](../reference/code-signing.md) and
[application preparation](../development/signpath-application.md). A laboratory/organization
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

---

# Windows インストール、driver、署名 / Windows installation, drivers and signing（日本語）

KohdaLab Camera は application installation と camera driver setup を分けます。アプリは current user として動作し、startup/Setup は Windows driver、service、security setting、PowerShell execution policy を変更しません。

## Installed driver policy

KohdaLab IV が official VISA runtime を優先し WinUSB へ黙って fallback しないのと同じ preservation principle を Camera も採ります。ただし camera SDK は VISA と別 interface なので独自に文書化した backend が必要です。検証済み SHODENSHA GR300 の acquisition backend は既存 WinUSB binding を必要とします。別 driver が binding されていれば Camera はそれを保持し、互換 vendor SDK backend がないことを説明して Load を無効化します。helper は USB を開く前に binding を再確認します。unknown/missing/composite binding を推測で WinUSB と扱いません。disk 上に driver package があるだけでは active binding から利用可能になりません。

official SDK integration は owner の要請により deferred です。official GR300 driver を取得に用いるには manufacturer API specification、supported architecture、SDK access、redistribution term が必要です。manufacturer DLL を推測した entry point で probe したり reverse engineer したりしません。公開済み [GR300-family setup instructions](https://www.shodensha-inc.co.jp/microscope/gr300bcm3_20180219.pdf) は supplied viewer を説明しますが、tested unit 向け再配布可能 API を示しません。別 GR300 revision は互換と仮定できません。[Microsoft USB desktop guidance](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/windows-desktop-app-for-a-usb-device) の通り WinUSB app は Winusb.sys を device driver に使い、vendor driver はその driver が公開する interface を必要とします。同じ USB ID でも transport は置換可能ではありません。

## Startup

startup は bounded saved preference を load して camera を asynchronous に enumerate します。一致 device が一台なら自動選択しますが、acquisition は **Load** が押されて初めて開始します。startup で elevation、installer、driver replacement、network fetch、certificate-store update、security bypass は実行しません。helper は PATH を検索せず executable の隣から解決し、missing helper は physical-session interlock 作成前に拒否します。protective lock と不確かな cleanup は reconnection を block し続け、installer はそれを消しません。application folder は完全なまま保ってください。

camera を開かず helper を起動しない support report は次です。

```powershell
& '.\KohdaLab Camera.exe' --startup-report '.\startup-report.json'
```

output path は新規でなければなりません。report は application version、enumerated matching camera binding、helper presence を含み、successful capture/certificate の証明ではありません。

## Installer と portable ZIP

NSIS installer は Windows x64 向けで、`%ProgramFiles%\Kohdalab\KohdaLab Camera` に machine-wide install します。desktop/Start Menu shortcut と HKLM Apps uninstall entry を作成し、application/helper/license notice を install しますが driver install や Live 自動開始はしません。user setting、capture、device interlock は program file と別です。portable ZIP も利用できます。公式 [releases](https://github.com/woolen-yarn/kohdalab-camera/releases) から Setup/ZIP を取得し SHA-256 を確認、helper/license file を保持してください。checksum は published reference との byte identity を確認しますが publisher signing の代替ではありません。

verified `package_windows.py` payload から明示的 NSIS compiler を使う build は次です。

```sh
python scripts/build_windows_installer.py --payload dist/windows \
  --output dist/KohdaLab-Camera-Setup.exe --makensis /path/to/makensis
```

build tool は runtime hash/license presence を確認し compiler を download/install しません。[NSIS](https://nsis.sourceforge.io/Docs/Chapter4.html) の `RequestExecutionLevel admin` により Setup に管理者承認を要求します。

## 検証済み installation と署名 / Verified installation and signing

0.1.11 は [public Windows CI](https://github.com/woolen-yarn/kohdalab-camera/actions/runs/39187983298) で全 executable type の metadata/unsigned status、exported/installed uninstaller byte 一致、fresh install/update/uninstall/unrelated-file preservation を pass しました。exact CI runtime は workstation に install し startup-check 済みです（[v0.1.11 validation](../validation/validation-0.1.11.json)）。これは unsigned release であり new physical capture qualification はありません。0.1.10 では NSIS 3.12/zlib と全 **687 build inputs** を確認し、fresh machine-wide install、same-version update、uninstall と unrelated test file の保持を確認しました。installed runtime hash/desktop shortcut も一致し、startup report は acquisition を始めず GR300 WinUSB binding と sibling helper を検出しました（[scoped installation record](../validation/validation-0.1.10.json)）。これは 0.1.9 physical Live validation を新 binary へ延長するものではありません。

調査した 0.1.10 GUI/helper/uninstaller/Setup は **NotSigned** です。user/machine personal certificate store に private key を持つ code-signing certificate はなく、checked file に Zone.Identifier stream もなかったため download-file SmartScreen behavior は示せません。7 日以内の available log の最新 200 event に Camera match はなく、SmartScreen Debug は disabled でした。matching event がないことは warning が起きない証明ではありません。release を通すため log setting/protection policy を変えないでください。

prepared payload/installed app の audit は次です。

```powershell
.\scripts\audit_windows_distribution.ps1 -PayloadDirectory '.\windows' `
  -OutputJson '.\signature-report.json' -RequireSigned
```

既存 execution policy が `.ps1` を block する machine では policy を保ち、標準 read-only command `Get-AuthenticodeSignature -LiteralPath ".\KohdaLab Camera.exe"` を interactively 実行するか、organization-approved signed-script environment を使います。Setup は PowerShell を必要としません。`-RequireSigned` は全 executable/DLL が Valid Authenticode signature を持たなければ fail し、publisher identity/timestamp も確認します。switch なしの report は unsigned status を記録しますが signing readiness を主張しません。

signed release では publisher-owned certificate/signing service を使い、application executable と将来の bundled DLL を build/sign、signature/publisher/timestamp を確認して final signed bytes から runtime manifest を生成し、uninstaller-signing hook で installer を compile、generated uninstaller/final Setup を sign/verify します。checksum は全 signing step 後に publish し、private key/service credential は source/log/distribution archive の外に置きます。current release は unsigned です。owner は free SignPath Foundation program の準備を依頼していますが application は未提出です。certificate publisher は laboratory/university でなく SignPath Foundation です。詳細は [Code signing policy](../reference/code-signing.md) と [application preparation](../development/signpath-application.md) を参照してください。organization certificate は organization validation/enrollment を要する別選択肢です。enrollment/purchase/credential change は実施していません。signed release は NSIS uninstaller を含む全 pipeline を qualify し、unsigned build は unsigned と表記し続けます。

[Microsoft's signing comparison](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options) は Store/MSIX、certificate、Artifact Signing option と geographic eligibility を、[SmartScreen reputation guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation) は valid signature でも immediate reputation は保証されず EV certificate に automatic bypass はないことを説明します。Smart App Control/enterprise policy が追加 block を行うこともあります。起動を妨げる block はアプリが修復できません。診断時は exact warning、file hash、signature report を保持し、protection を disable したり Mark of the Web を黙って除去したりしないでください。

## Configuration location / 設定の保存先

Use `%USERPROFILE%\.kohdalab\camera\config.json` for each user. Earlier settings are preserved at their old location and are not imported automatically. Before moving from the earlier per-user installer, close the application and uninstall that installation, then run the new all-users Setup. Settings and captures remain intact.

各ユーザーの設定は `%USERPROFILE%\.kohdalab\camera\config.json` に保存します。旧設定は元の場所に保持し、自動移行しません。旧ユーザー別インストールから移行する場合はアプリを終了し、旧版をアンインストールしてから新版の全ユーザー向けSetupを実行してください。設定と保存画像は保持します。

The legacy-install check uses the elevated installer's account. When a standard user supplies a different administrator account at UAC, manually uninstall that user's previous version before starting Setup; the elevated check cannot inspect every user's installation. The new configuration starts with defaults and leaves the old preference file untouched.

旧版検出は昇格後のアカウントを対象にします。UAC で別の管理者アカウントを指定する場合は、Setup 起動前に元ユーザーの旧版を手動でアンインストールしてください。全ユーザーの旧版を一括検出するものではありません。新保存先では既定設定で開始し、旧設定ファイルは変更しません。
