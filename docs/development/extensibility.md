# Extending the camera platform

The next priority is support for cameras from other manufacturers. Only the
SHODENSHA GR300 configuration described in the compatibility documentation is
currently supported. This document distinguishes existing boundaries from work
that must precede a second physical backend; it does not announce new support.

## Current foundation and remaining coupling

| Area | Existing foundation | Next boundary to establish |
| --- | --- | --- |
| Discovery | `device_catalog.rs` shares USB candidate identification between the picker and startup report | Per-backend discovery, including devices that do not enumerate through USB |
| Connection | Installed-driver checks and strict physical identity checks | Select a documented backend compatible with the installed driver |
| Acquisition | `session::Factory` and `Transport`, bounded workers and lifecycle ownership | Backend-specific settings and supported operations |
| Controls | GR300 modes, exposure in sensor rows and gain in eighths | Explicit units, ranges, steps and availability for each backend |
| Frames | Validated RAW frames, acquisition identity and display recipes | Explicit format, dimensions, stride, bit depth and color interpretation for each backend |
| Timing | Host-side frame observations and acquisition-rate diagnostics | Explicit clock domains and documented device timestamp semantics |
| Presentation | Shared picker, image view, image adjustments and export flow | Show controls supported by the selected backend without exposing transport details |

The catalog is deliberately a discovery list. A matching USB ID does not grant
permission to open a device or prove compatibility. Existing sensor, firmware,
driver and physical-location checks remain in the backend. Adding a catalog
entry alone is insufficient to add a camera.

## First additional camera: a vertical implementation

Choose one actual camera with a documented acquisition interface and acceptable
SDK and redistribution terms. Record the manufacturer, model, operating system,
driver/SDK version and documentation provenance before implementing transport.
Do not infer protocols from manufacturer binaries or private USB captures.
Keep SDK integration deferred when the necessary API and license are unavailable.

Implement discovery, connection, one validated frame format, live view, saving
and normal disconnect for that device before designing a general plugin system.
Use the experience of two working backends to define the shared contract. Avoid
loading arbitrary vendor DLLs or allocating hardware resources merely to build
the picker. Preserve existing drivers; a driver incompatible with a backend
must produce a clear explanation rather than a silent replacement or fallback.

### Settings and capabilities

Keep device commands and encodings inside the backend. GR300 `exposure_rows`
and `gain_eighths` are not universal camera settings. Do not relabel rows as
milliseconds or reuse a numeric range for another sensor.

A future capability description should report supported acquisition modes,
frame formats, control units/ranges/steps, read-only values, and whether a change
requires stopping acquisition. Validate settings inside the backend as well as
in the GUI. Store preferences under a stable backend/model identity with a
schema version; never apply another model's settings just because a USB product
string looks similar. Migration must retain current GR300 defaults and behavior.

### Ownership, data flow and responsiveness

Retain a single session owner and the current shutdown guardian. A new backend
must honor stop/disconnect deadlines, reject stale frame generations and keep
driver calls off the UI thread. Vendor callbacks must not update GUI state
directly. Treat borrowed SDK buffers as valid only for their documented lifetime;
copy or transfer ownership before the next SDK operation can reuse them.

Keep preview work bounded and allow obsolete preview work to be superseded.
Image export and future analysis must use an immutable frame snapshot with its
matching settings and recipe. Do not extend preview dropping rules to a future
lossless recording feature without an explicit recording contract.

### Acceptance for each backend

- Test settings validation, unsupported controls, frame layout and conversion
  using deterministic fixtures without opening hardware.
- Test connection failures, stale callbacks, disconnect during acquisition and
  shutdown through a backend substitute.
- Qualify the actual model/driver combination with finite hardware runs covering
  live view, applying supported settings, export, disconnect and reconnect.
  Record image dimensions, pixel integrity and normal cleanup separately from
  optical quality and performance observations.
- Publish a compatibility matrix with tested versions, known limitations and
  driver requirements. Compilation or a matching USB ID is not qualification.

## Later measurement and instrument extensions

The current `measurement.rs` records capture observations such as host timing
and acquisition rate. Keep that purpose separate from distance, area, intensity
or external-instrument measurements.

For image measurements, introduce a versioned result record associated with an
immutable frame identity (session, settings generation and frame sequence).
Record algorithm version, ROI coordinates, units and calibration identity.
Define mappings between image coordinates and zoom/pan/display coordinates;
measurements must not change when the user zooms. Physical units require a valid
calibration for the optical configuration and resolution. A display adjustment
recipe is not a calibration. Make the chosen analysis input explicit: sensor RAW
or a documented derived image, with its processing provenance.

External devices should have their own lifecycle and typed samples. Associate
samples with frames through explicit time/trigger information, including clock
domain, uncertainty and missing-data status. Host arrival time alone does not
prove simultaneous exposure or hardware synchronization. Consider injectable
clock sources when implementing that work, preserving existing timestamp
semantics until a separately tested migration is available.

Export these results as separate versioned metadata linked to the original
frame and calibration. Preserve existing RAW, image and recipe export semantics.
Keep the normal GUI concise: supported controls and essential state first,
diagnostic details on demand.

## Delivery order

1. **Implemented:** share the current discovery candidate definition without
   broadening connection eligibility or changing the GUI.
2. **Next:** select one documented additional camera and establish its driver,
   API, licensing and frame-format requirements.
3. **Then:** add backend-specific settings and capabilities while retaining GR300
   regression coverage; implement and qualify that second backend end to end.
4. **Later:** add calibrated image measurements and external-instrument timing
   as separate, reproducible data products.

No additional physical backend, generic capability-driven controls or scientific
measurement engine is implemented by the discovery refactor.

---

# camera platform の拡張 / Extending the camera platform（日本語）

次の優先事項は他 manufacturer の camera 対応です。現在対応するのは compatibility documentation の SHODENSHA GR300 configuration だけで、本書は既存 boundary と二つ目の physical backend 前に必要な作業を区別し、新対応を発表するものではありません。

## 現在の基盤と残る coupling / Current foundation and remaining coupling

`device_catalog.rs` は picker/startup report の USB candidate identification を共有しますが、次は USB enumerate しない device も含む backend ごとの discovery です。connection は installed-driver/strict physical identity check を持ち、next は installed driver と互換の documented backend の選択です。`session::Factory`/`Transport`、bounded worker、lifecycle ownership は acquisition の基盤で、next は backend-specific setting/operation です。GR300 mode、sensor row の exposure、eighth の gain を一般設定として扱わず、backend ごとに unit/range/step/availability を明示します。frame は validated RAW/acquisition identity/display recipe から format/dimension/stride/bit depth/color interpretation へ、timing は host-side observation/acquisition-rate diagnostics から clock domain/device timestamp semantics へ拡張します。shared picker/image view/image adjustment/export flow は transport detail を露出せず selected backend が対応する control を示します。catalog は discovery list であり、USB ID 一致だけでは device open の許可や互換性の証明になりません。sensor/firmware/driver/physical-location check は backend に残し、catalog entry だけで camera を追加できません。

## 最初の追加 camera: vertical implementation

documented acquisition interface と許容できる SDK/redistribution term を持つ実在 camera 一台を選び、transport 実装前に manufacturer/model/OS/driver/SDK version/documentation provenance を記録します。manufacturer binary/private USB capture から protocol を推測せず、必要 API/license がなければ SDK integration を deferred に保ちます。general plugin system より先に、その device の discovery、connection、検証済み frame format 一種、live view、save、normal disconnect を実装します。二つの backend の経験から shared contract を定義し、arbitrary vendor DLL を load したり picker のために hardware resource を allocate したりしません。existing driver を保持し、backend と非互換の driver は silent replacement/fallback ではなく明確な説明を返します。

### Settings/capabilities、ownership/data flow、acceptance

device command/encoding は backend 内に閉じます。GR300 `exposure_rows`/`gain_eighths` を universal setting にせず、row を millisecond と改名したり別 sensor に同じ numeric range を流用しません。future capability description は acquisition mode、frame format、control unit/range/step、read-only value、変更時の acquisition stop 要否を報告し、GUI と backend の両方で validate します。preference は stable backend/model identity と schema version の下に保存し、USB product string が似ているだけで別 model setting を適用しません。migration は GR300 default/behavior を保持します。

single session owner/current shutdown guardian を保持し、新 backend は stop/disconnect deadline、stale frame generation の拒否、driver call の UI thread 外実行を守ります。vendor callback が GUI state を直接更新せず、borrowed SDK buffer は文書化された lifetime 内だけ有効として、次 SDK operation 前に copy/ownership transfer します。preview work は bounded にし obsolete work を supersede 可能にし、export/future analysis は matching setting/recipe を持つ immutable frame snapshot を使用します。明示的 recording contract なしに preview dropping rule を lossless recording へ拡張しません。

各 backend では deterministic fixture で setting validation/unsupported control/frame layout/conversion、backend substitute で connection failure/stale callback/acquisition 中 disconnect/shutdown を test し、実 model/driver で live view、supported setting apply、export、disconnect/reconnect を含む有限 hardware run を qualify します。image dimension/pixel integrity/normal cleanup を optical quality/performance observation から別に記録し、tested version/known limitation/driver requirement を持つ compatibility matrix を公開します。compile や USB ID match は qualification ではありません。

## 後の measurement/instrument 拡張と順序 / Later extensions and delivery order

current `measurement.rs` は host timing/acquisition rate の capture observation を記録し、distance/area/intensity/external instrument measurement とは分離します。image measurement では immutable frame identity（session、settings generation、frame sequence）に結ぶ versioned result record、algorithm version/ROI coordinate/unit/calibration identity、image coordinate と zoom/pan/display coordinate の mapping を導入します。measurement は zoom で変わらず、physical unit には optical configuration/resolution に有効な calibration が必要です。display adjustment recipe は calibration ではありません。analysis input は sensor RAW か来歴付き documented derived image かを明示します。

external device は固有 lifecycle/typed sample を持ち、clock domain/uncertainty/missing-data status を含む explicit time/trigger で frame と結びます。host arrival time は simultaneous exposure/hardware synchronization を示しません。実装時は injectable clock source を検討し、別途検証済み migration まで既存 timestamp semantics を保ちます。result は original frame/calibration に結ぶ別 versioned metadata として export し、既存 RAW/image/recipe export semantics と簡潔な GUI（supported control/essential state を先、diagnostic は必要時）を保ちます。

順序は、(1) 実装済み: connection eligibility/GUI を広げず discovery candidate definition を共有、(2) 次: documented additional camera と driver/API/license/frame-format requirement の選択、(3) 次に GR300 regression を保つ backend-specific setting/capability と二つ目 backend の end-to-end qualification、(4) 後に calibrated image measurement と external-instrument timing を別の再現可能 data product として追加、です。discovery refactor は additional physical backend、generic capability-driven control、scientific measurement engine を実装していません。
