# KohdaLab Camera 0.1.11

- Add public, read-only GitHub Actions Windows GNU builds with pinned Rust and
  Actions, a hash-verified NSIS compiler, source identity checks and unsigned
  distribution/signing-input artifacts.
- Derive GUI/helper PE product/version resources from the root package version;
  verify matching metadata on GUI, helper, uninstaller and Setup.
- Export the exact unsigned NSIS uninstaller for future SignPath enrollment.
- Qualify synthetic-frame GUI tests, fresh install, update, uninstall, unrelated
  file preservation and no-acquisition startup diagnostics in Windows CI.
- Install the exact CI runtime on the workstation and verify hashes, version,
  shortcut target and startup report. Organize current source/output paths and
  archive earlier validation/staging data without deleting it.

[CI and installation validation](docs/validation/validation-0.1.11.json) is scoped to build,
metadata and installation. Physical capture validation remains v0.1.9. This
release remains unsigned; no SignPath application or account integration was
submitted. See [application preparation](docs/development/signpath-application.md).

# KohdaLab Camera 0.1.10

- Preserve existing Windows driver bindings. Reuse WinUSB only when it is the
  active binding; a different or missing driver receives an explanation instead
  of a silent replacement or unsupported acquisition attempt.
- Recheck the binding in the physical helper. Detect a missing helper before
  creating a physical-session interlock.
- Add `--startup-report new-file.json`, an enumeration-only support report that
  does not open USB or launch the acquisition helper.
- Add per-user Windows Setup tooling and a signature audit, with documented
  organization-signing prerequisites and honest unsigned-release status.

Official GR300 SDK integration is deferred at the owner's request. The existing
WinUSB backend remains the only implemented physical acquisition backend.
Version 0.1.9 retains the last five-minute physical validation; new 0.1.10 checks
cover driver policy, UI regression and Windows startup/installation behavior.
Fresh install, update, uninstall and unrelated-file preservation passed. The
installed runtime hashes and shortcut were verified; all four executable types
remain unsigned. See [installation validation](docs/validation/validation-0.1.10.json).

# KohdaLab Camera 0.1.9

- Distinguish the whole-image **Fit** view from magnification such as
  **2.0× Fit**, with an explanation of how it differs from 100%.
- Add a quick start with current Simulation screenshots and a responsiveness
  report separating CPU timings from UI-tool observation overhead.

The targeted GUI check, native Simulation image adjustment/export/normal close,
and exact saved-image replay passed. Windows x64 built successfully. Subsequent
physical validation of the exact published binaries completed 399.4 seconds of
GR300 Live with 15,701 accepted frames, image adjustment and Fit controls,
PNG/RAW/Bayer TIFF export, and normal Disconnect/Close. All saved pixels matched
independent RAW reconstruction; both processes exited normally without a
remaining protective lock. See [validation](docs/validation/validation-0.1.9.json) and
[observed responsiveness](docs/validation/responsiveness.md).

The initial zero-connection attempt was blocked by Windows App input delivery.
The owner opened the picker, and held mouse clicks then allowed the finite run
to proceed. Validation is supplemental to the original release archives; their
bytes and original manifests remain unchanged.

# KohdaLab Camera 0.1.8

- Show the installed application version in the camera-selection window title,
  Live window title, and Live toolbar.

This release did not repeat physical-camera acquisition. It was built and
published from the current Windows x64 source baseline.

# KohdaLab Camera 0.1.7

- Refine the dark interface: make the image the visual focus, use a restrained
  accent for available primary actions, and allow a selected toolbar tab to
  close its inspector.
- Make Fit the minimum zoom, add visible Fit / minus / magnification / plus /
  100% controls, and keep 100% unavailable when it would be below Fit or beyond
  the supported zoom range.
- Remove the reserved blank area before Cursor in the status bar and simplify
  Advanced details and duplicated display controls.
- Avoid idle repaint polling and reuse compiled display recipes until image
  controls change. Image and export semantics remain unchanged.

Validation included GUI, renderer, and preview-pipeline tests; native Simulation
interaction checks; and replay against a saved GR300 endpoint image. This
release did not repeat physical-camera acquisition, and its local CPU
measurements do not establish a physical capture-FPS improvement.

# KohdaLab Camera 0.1.6

- Remember camera/image settings, Live window size, save folder and capture
  numbering. Restore defaults from Advanced without automatically reconnecting
  or applying camera controls.
- Add pointer-centered wheel zoom, drag panning, double-click to fit, and Fit /
  100% buttons in Image. View transforms do not change exported pixels.
- Suggest numbered PNG names and skip occupied image/sidecar names.
- Render preview pixels directly into ColorImage and remove the unused retained
  image clone. This eliminates an intermediate RGB allocation and pixel copy;
  measured CPU timings vary, so no additional capture-FPS increase is claimed.

Validation includes exact direct/indirect pixel equivalence, bounded pipeline
checks, saved physical-image replay, preferences overwrite/retry, view geometry,
input isolation, no-auto-connect/reset tests, independent review and native Mac
Simulation interaction/restart/reset checks. The last physical Live validation
remains the scoped 0.1.4 run.

# KohdaLab Camera 0.1.5

- Set the default exposure to 120 sensor rows. Gamma remains 2.2 and brightness
  remains 1.20×.
- Record the owner's visual confirmation of acceptable color and field of view;
  this does not imply optical/color calibration.

The five-minute physical validation applies to 0.1.4. This default-setting update
was checked with the UI/integration suite and a Windows release build.

# KohdaLab Camera 0.1.4

- Automatically select a single supported camera after enumeration; open it with
  **Load camera**. Multiple cameras still require a choice, and a missing selected
  camera is never silently replaced by a different device.
- Shorten the opening screen instructions.

Validation: 26 UI tests and independent review passed. The exact Windows build
completed 336.2 seconds of real GR300 Live acquisition, a PNG/RAW/Bayer TIFF export
with independent pixel agreement, and normal GUI/helper exit. No process or
protective lock remained. The owner confirmed manual resizing and size retention.
Color/optical qualification and picker round-trip resizing remain outside this
run. See [the scoped validation record](docs/validation/validation-0.1.4.json).

# KohdaLab Camera 0.1.3

- Simplify the Live status bar and export panel. Move technical counters and
  startup diagnostics into a collapsed Diagnostics section.
- Keep capture FPS, simulation/retained-image labels and actionable errors visible.

- Reconstruct only the color neighbors required at each Bayer site, with layout
  validation once per render. RGB pixels, RAW bytes and recipe semantics remain
  unchanged. The synthetic Mac CPU benchmark reduced rendering time by about
  31–35%; this is not a Windows capture-FPS claim.
- Verify all pixels against the preceding real-camera export and retain exact
  orientation, scaling, color and malformed-input regression coverage.

Validation: 26 UI tests passed; rendering tests passed; simulation Image/Export
panels were visually checked with mouse clicks. The 0.1.3 Windows build has not
had a new physical-camera run. See [performance verification](docs/validation/rendering-performance.md).

## Additional 0.1.2 validation

The published build subsequently completed 333.2 seconds of Live acquisition on
SHODENSHA GR300, with a pixel-checked endpoint export and normal native shutdown.
Manual mouse operation was confirmed by the owner. The planned start export was
canceled, so the full two-endpoint export plan remains incomplete. See
[the later validation record](docs/validation/validation-0.1.2-five-minute.json).

The earlier observations below retain their original candidate scope.

# KohdaLab Camera 0.1.2

A focused stability update for the SHODENSHA GR300 backend.

- Remember valid Live window sizes as they change, and preserve them when native
  transition reports are missing or still describe the smaller camera picker.
- Keep the SIMULATION label visible on retained images after disconnect.
- Add a regression test for normal mouse press/release on all five Live tabs.
- Add optional, simulation-only input diagnostics with bounded pointer/navigation
  records, no typed/pasted text, and a normal-close deadline. This feature is
  disabled in the distributed application.

A short physical run observed Live acquisition, image and sidecar export,
independent pixel agreement, and native GUI/helper exit code 0. Postflight found
no remaining camera process or protective interlock and normal WinUSB status.
The 60-second preflight-to-Connect window was not proven, so this is not an
acceptance PASS. The final GUI binary additionally includes the retained-
simulation label fix and has not been physically retested. See
`docs/validation/validation-0.1.2.json` for the exact scope.

---

# 変更履歴 / Changelog（日本語）

以下の英語の各 entry が canonical historical record です。日本語は同じ release scope を読みやすくする対訳で、記録済み validation、hash、release asset を変更しません。

## 0.1.11

公開・read-only の GitHub Actions Windows GNU build（pinned Rust/Actions、hash-verified NSIS、source identity、unsigned distribution/signing-input artifact）を追加しました。GUI/helper PE product/version resource を root package version から導出し、GUI、helper、uninstaller、Setup の metadata 一致を検証します。将来の SignPath enrollment 用に exact unsigned NSIS uninstaller を export し、synthetic-frame GUI、fresh install/update/uninstall、unrelated file preservation、no-acquisition startup diagnostic を Windows CI で qualify しました。workstation に exact CI runtime を install して hash/version/shortcut/startup report を確認し、source/output path と過去 validation/staging data を削除せず整理しました。[CI and installation validation](docs/validation/validation-0.1.11.json) は build/metadata/install の範囲で、physical capture validation は 0.1.9 のままです。release は unsigned で SignPath application/account integration は提出していません（[application preparation](docs/development/signpath-application.md)）。

## 0.1.10

既存 Windows driver binding を保持し、WinUSB は active binding のときのみ再利用します。異なる/missing driver は silent replacement/unsupported acquisition でなく説明を返します。physical helper でも binding を再確認し、physical-session interlock の前に missing helper を検出します。`--startup-report new-file.json` は USB を開かず helper も起動しない enumeration-only support report です。per-user Windows Setup/signature audit と organization-signing prerequisite/unsigned status を追加しました。owner request により official GR300 SDK integration は deferred で、existing WinUSB backend が唯一の physical backend です。0.1.9 が five-minute physical validation、0.1.10 は driver policy/UI regression/Windows startup-install behavior を対象とし、fresh install/update/uninstall/unrelated file preservation、runtime hash/shortcut を確認しました。4 executable type は unsigned です（[installation validation](docs/validation/validation-0.1.10.json)）。

## 0.1.9

whole-image **Fit** と **2.0× Fit** のような magnification を区別し、100% との差を説明しました。Simulation screenshot 付き quick start と CPU timing/UI-tool observation overhead を分けた responsiveness report を追加しました。targeted GUI/native Simulation image adjustment/export/normal close、exact saved-image replay、Windows x64 build を確認し、その後 published binary が GR300 Live **399.4 seconds**、**15,701 accepted frames**、image adjustment/Fit、PNG/RAW/Bayer TIFF export、normal Disconnect/Close を完了しました。saved pixel は independent RAW reconstruction と一致し、protective lock は残りませんでした。owner の input delivery 制約により held mouse click を用いた run であり、original release archive を補う validation ですが byte/original manifest は不変です（[validation](docs/validation/validation-0.1.9.json)、[responsiveness](docs/validation/responsiveness.md)）。

## 0.1.8

- camera-selection window title、Live window title、Live toolbar に installed application version を表示します。

この release では physical-camera acquisition を再実施していません。current Windows x64 source baseline から build/publish しました。

## 0.1.7

- dark interface を改善し、image を visual focus に、available primary action に抑制した accent を使い、selected toolbar tab で inspector を閉じられるようにします。
- Fit を minimum zoom とし、visible な Fit / minus / magnification / plus / 100% control を追加します。100% が Fit 未満または supported zoom range 外になるときは利用不可に保ちます。
- status bar の Cursor 前に予約していた blank area を除き、Advanced detail と重複した display control を簡潔にします。
- idle repaint polling を避け、image control が変わるまで compiled display recipe を再利用します。image/export semantics は変更しません。

validation には GUI、renderer、preview-pipeline test、native Simulation interaction check、保存済み GR300 endpoint image の replay を含みます。この release は physical-camera acquisition を再実施しておらず、local CPU measurement は physical capture-FPS improvement の根拠ではありません。

## 0.1.6

- camera/image setting、Live window size、save folder、capture numbering を記憶します。Advanced から default を復元しても自動 reconnect や camera control の適用はしません。
- pointer 中心の wheel zoom、drag pan、double-click Fit、Image の Fit / 100% button を追加します。view transform は exported pixel を変更しません。
- 番号付き PNG name を提案し、使用済み image/sidecar name を避けます。
- preview pixel を ColorImage に直接 render し、不要な retained image clone を除きます。intermediate RGB allocation/pixel copy をなくしますが、測定 CPU timing は変動するため追加 capture-FPS increase は主張しません。

validation には exact direct/indirect pixel equivalence、bounded pipeline check、saved physical-image replay、preference overwrite/retry、view geometry、input isolation、no-auto-connect/reset test、independent review、native Mac Simulation interaction/restart/reset check を含みます。最後の physical Live validation は scope を限定した 0.1.4 run のままです。

## 0.1.5

- default exposure を **120 sensor rows** に設定します。gamma は **2.2**、brightness は **1.20×** のままです。
- owner による acceptable color/field of view の visual confirmation を記録しますが、optical/color calibration を意味しません。

five-minute physical validation は 0.1.4 に適用されます。この default-setting update は UI/integration suite と Windows release build で確認しました。

## 0.1.4

- enumeration 後、supported camera が一台なら自動選択し、**Load camera** で開きます。複数 camera では選択が必要で、missing selected camera を別 device に黙って置換しません。
- opening screen instruction を短縮します。

validation: **26 UI tests** と independent review が pass しました。exact Windows build は実 GR300 Live acquisition **336.2 seconds**、independent pixel agreement を伴う PNG/RAW/Bayer TIFF export、normal GUI/helper exit を完了しました。process/protective lock は残りませんでした。owner は manual resize と size retention を確認しました。color/optical qualification と picker round-trip resizing はこの run の範囲外です。[scoped validation record](docs/validation/validation-0.1.4.json) を参照してください。

## 0.1.3

- Live status bar/export panel を簡潔にし、technical counter/startup diagnostic を折り畳んだ Diagnostics section に移します。
- capture FPS、simulation/retained-image label、actionable error を可視のままにします。
- 各 Bayer site で必要な color neighbor だけを再構成し、layout validation は render ごとに一度にします。RGB pixel、RAW byte、recipe semantics は変更しません。synthetic Mac CPU benchmark は rendering time を約 **31–35%** 削減しましたが、Windows capture-FPS の主張ではありません。
- preceding real-camera export に対して全 pixel を検証し、orientation、scaling、color、malformed-input の exact regression coverage を保ちます。

validation: **26 UI tests** と rendering test が pass し、simulation Image/Export panel は mouse click で visual check しました。0.1.3 Windows build は新しい physical-camera run を受けていません。[performance verification](docs/validation/rendering-performance.md) を参照してください。

## Additional 0.1.2 validation / 0.1.2 の追加検証

公開 build はその後 SHODENSHA GR300 で **333.2 seconds** の Live acquisition、pixel-checked endpoint export、normal native shutdown を完了しました。owner は manual mouse operation を確認しました。planned start export は canceled されたため、full two-endpoint export plan は未完です。[later validation record](docs/validation/validation-0.1.2-five-minute.json) を参照してください。

以下の earlier observation は original candidate scope を保ちます。

## 0.1.2

SHODENSHA GR300 backend 向けの focused stability update です。

- valid Live window size を変化時に記憶し、native transition report がない場合や、より小さい camera picker を記述したままの場合にも保持します。
- disconnect 後の retained image に **SIMULATION** label を表示し続けます。
- 5 個すべての Live tab で通常の mouse press/release を行う regression test を追加します。
- bounded pointer/navigation record、typed/pasted text を含まないこと、normal-close deadline を持つ optional simulation-only input diagnostic を追加します。この feature は distributed application では無効です。

短い physical run では Live acquisition、image/sidecar export、independent pixel agreement、native GUI/helper exit code 0 を観測しました。postflight は camera process/protective interlock が残らず、WinUSB status が正常であることを確認しました。**60-second preflight-to-Connect window は証明されていないため、acceptance PASS ではありません**。final GUI binary には retained-simulation label fix も含まれますが、物理的に再試験していません。正確な scope は `docs/validation/validation-0.1.2.json` を参照してください。

Windows App を通じた ordinary mouse input、physical arbitrary-resize persistence、color/field-of-view calibration は未解決です。long-duration testing は deferred です。acquisition/image-processing algorithm は変更していません。
