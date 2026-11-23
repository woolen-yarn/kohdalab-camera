# KohdaLab Control Protocol 1 / 共通GUI API

This additive API standardizes local GUI discovery, authentication, ownership,
status, job receipts and errors. Scientific settings and operation arguments
remain application-specific; ask `describe()` before invoking an operation.
Existing native clients and their safety defaults remain available.

## Python / Notebook

Install the separate dependency-free SDK from `bindings/control` in the source
or the common SDK directory in the package. This is the same distribution in all
five applications, not five overlapping Python modules.

```sh
python -m pip install ./bindings/control
```

```python
from kohdalab_control import ControlClient

app = ControlClient("kohdalab-camera")
print(app.describe())
print(app.status())
```

For multiple instances choose an explicit private descriptor with `endpoint=`.
Read `describe/status/config/result` without a context manager; context entry
acquires the lease. Do not substitute internal camera helper IPC for a public
GUI API. A Windows save dialog can keep the GUI from handling a request within
the two-second deadline; query status again after closing the dialog.

Windows の保存ダイアログを開いている間は、GUI が2秒の期限内に要求を処理できない
場合があります。ダイアログを閉じてから状態を再取得してください。

### Camera operations / Camera の操作

```python
with ControlClient("kohdalab-camera") as cam:          # acquire + heartbeat
    cam.wait(cam.invoke("start_live", source="simulation")["job_id"])
    cam.wait(cam.invoke("capture", path="/data/roi.png")["job_id"])
    cam.stop_and_wait()
# release: the window's Stop, then the release job's own cleanup evidence
```

| Method | Same GUI action | Terminal receipt only after |
| --- | --- | --- |
| `acquire` / `heartbeat` / `release` | — | Lease 1 s heartbeat, 5 s timeout; one holder |
| `start_live` `source` | **Load camera** with the window's selection, driver gate, startup intent and draft settings | A valid frame for that request (`completed`) |
| `stop` | **Stop / Disconnect** | Worker inactive; cleanup from lifecycle evidence |
| `capture` `path` | **Capture image…** after its dialog (displayed frame and recipe, current Live profile) | Save pipeline result plus primary, RAW, metadata and sidecars on disk |
| `capture_full` `path` | **Capture full sensor image…** after its dialog | File saved **and** the existing verified return to the applied Live settings |

- `source` must be `simulation` or `usb` and match the window selection; the
  API cannot pick a device or bypass the picker, driver gate or Simulation-window
  interlock. Admission uses the same readiness checks as the buttons.
- `path` must be absolute, `.png/.tif/.tiff`, in an existing folder, with no
  existing primary or sidecar. It is checked before acquisition changes and again
  atomically when files are written. Existing files and the window's default
  save folder and filename sequence are preserved.
- Simulation frames carry no acquisition metadata, so `capture_full` is refused
  (`busy`) there. With a verified Live signal from a selected camera, call
  `cam.wait(cam.invoke("capture_full", path=full_path)["job_id"])`.
  Recording is not implemented (`unsupported`).
- Mutations need a UUID `client_id`, the current `lease` and a `request_id`.
  Repeating a `request_id` returns the original decision (no second action);
  reusing it for different parameters is `request_id_conflict`. A request that
  expired before the GUI handled it is refused with `timeout`; nothing ran.
- Stale or wrong leases are refused (`invalid_lease`, or `revoked` with the
  reason).

| Event | Effect |
| --- | --- |
| `release` | Lease ends; if acquisition is present the window's Stop is requested and a `release` job is returned for the SDK to follow |
| Heartbeat expiry (also how a lost client is detected) | Same as release, with a `lease_expiry` job; late calls get `revoked` |
| Window **Take control** | Lease ends; acquisition continues under the window |
| Window **Stop** | Lease ends and the window's Stop is requested |
| Window **Stop / Disconnect** while leased | Always available; running receipts become `stopped`; the lease remains |

Cleanup is `verified` only when the Worker lifecycle reports collection without
unproved cleanup, `not_required` for software Simulation, and otherwise
`unknown`. Unknown or failed cleanup, or a Worker that disabled reconnection,
reports `phase: "error"` and refuses `acquire`, `start_live` and captures
(`cleanup_unconfirmed`) until the application is restarted after the recovery
guidance. Stop and release stay available.

With `--gui-api` the toolbar and camera picker show **External control ·
Available**. While a client holds the lease they show **External control ·
Active** with **Stop** and **Take control**. Source selection, Load, Reconnect,
Change camera, settings, the software recipe, Reset and captures are disabled
in the window; Stop / Disconnect, Pause and view zoom stay available.

`--gui-api` 起動時は **External control · Available** を表示し、リース保持中は
**External control · Active** と「Stop」「Take control」を表示します。保持中は窓の
カメラ選択・設定・画像処理・撮影を無効化し、停止・切断は常に使えます。外部操作は
窓のボタンと同じ処理と判定を使い、装置選択の迂回はありません。解放・心拍切れでは
窓の停止を要求し、後片付けを確認できない場合は再起動まで外部操作を受け付けません。

### Camera `control.config`

The top-level `mode`, `exposure_rows` and `gain_eighths` keep their earlier
meaning: the GUI draft, which may not be applied yet. Additional fields come
only from the Worker snapshot the GUI holds for that update:

| Field | Meaning |
| --- | --- |
| `draft` | The same GUI draft as the top-level fields |
| `requested` | Settings and `generation` last sent to the Worker, or `null` |
| `applied` | Settings and `generation` that the Worker has checked as live for the current request, or `null` if this is not confirmed |
| `applied_generation` | `applied.generation`, or `null` |
| `apply_state` | `none`, `pending`, `applied`, `unconfirmed` (streaming, but settings not checked, e.g. Simulation), `stopping` or `failed` |

While a new Apply is pending, and after Stop, disconnection, an error or a
disabled restart, `applied` is `null`. Earlier settings are never reported as
the current ones. During Full capture, `applied.mode` is the Full profile
while the draft keeps the Live profile.

`control.config` の最上位 `mode/exposure_rows/gain_eighths` は従来どおり GUI の下書き
（未適用の場合あり）です。`draft` は同じ下書きです。`applied` には、Worker が現在の要求を
確認して適用した値と世代が入ります。確認できない場合は `null` です。Apply の処理中、停止後、
切断時、異常時には、以前の適用値を現在の値として返しません。

### Framing and refusals / 応答形式と拒否

Every response is exactly one newline-terminated JSON record. This includes
early refusals made before the GUI is involved: malformed JSON, a non-object
request, an incomplete request and a wrong token. Malformed, non-object and
incomplete requests keep the legacy `{ok:false,error_code,error}` shape.
A wrong token on a `control.*` request returns the common error with
`error.code: "authentication"`, and the request never reaches the GUI. The
SDK therefore reports `authentication`, not `transport`. Authenticated
operations that Camera does not implement (for example `control.record`) reach
the GUI and return `error.code: "unsupported"` from the running application. The SDK's local
refusal for operations missing from `describe()` is a separate check.

すべての応答は改行で終わる1行のJSONです。GUIに渡す前の拒否も同じです。誤ったトークンを
送ると、SDK は `transport` ではなく `authentication` として報告します。認証後の未対応の
操作には、実行中のアプリが `unsupported` を返します。

## Contract / 通信仕様

| Area | Common behavior |
| --- | --- |
| Discovery | One atomic per-instance descriptor in `~/.kohdalab/<app>/run` (Windows `%USERPROFILE%\.kohdalab\<app>\run`); app, instance UUID, PID, loopback host/port, private token, protocol version |
| Wire | Local TCP JSON Lines, `control_protocol: 1`, `method: control.<name>` |
| Success | `ok: true`, `request_id`, `result`, `error: null` |
| Error | `ok: false`, `request_id`, `error: {code, application_code, message, retryable}` |
| Ownership | `client_id` UUID plus opaque `lease`; heartbeat interval and timeout come from `describe()`/`acquire()` |
| Status | `phase`, actual admission `busy`, `ownership`, `cleanup`, app snapshot under `native` |
| Job receipt | `job_id`, `kind`, `status`, `terminal`, `cleanup`, optional `linked_job_id`, original receipt under `native` |
| Completion | Only `completed/stopped/error` are terminal; a command ACK can link to a separate acquisition job |
| Cleanup | `verified/pending/not_required/unknown`; ACK or output command alone is not verified cleanup |

Read endpoints are token-authenticated without acquiring ownership. Mutations
use fresh UUID request IDs and the app's existing admission/duplicate tracking.
A transport timeout means the outcome may be unknown: inspect status and the
specific receipt before retrying. The SDK never replays mutations automatically,
chooses a process by recency, or opens a second instrument connection.

`wait(job_id)` follows explicit ACK-to-job links, never the latest job.
`stop_and_wait()` requires actual cleanup confirmation. `release()` waits for
confirmed release and cleanup; a timeout is reported rather than hidden. Finish
or explicitly stop work before releasing when required by the app policy.

## Application boundaries / アプリ別の動作

| App | Public scope | Safety policy retained |
| --- | --- | --- |
| IV | Measurement, configuration, source/gate operations through GUI | Existing protections and fresh output-OFF confirmation on release/takeover |
| TRKR | Measurement, configuration and supported motion through GUI | Existing continuation options and motion/stop admission rules |
| PLE | Measurement/configuration through GUI; explicit confirmations retained | Optical cleanup barrier and command-ACK/run-job distinction |
| Shutter | Supported channel operations through GUI | Hold last state; commanded position is not measured optical transmission |
| Camera | Lease, Live start/stop, current-ROI and full-sensor capture through GUI | Window source selection and interlocks; Stop on release/expiry; unknown cleanup blocks; helper stays private |

The common contract does not make device stop policies identical. Read the
advertised methods and safety fields from the running release. Native API
versions, app versions and common protocol version are separate.

GUI経由の共通窓口を追加しました。既存API・測定・CSV・停止方針は維持します。
対応操作は `describe()` で確認し、複数起動時は対象を明示してください。
通信失敗時に操作を自動再送せず、受付応答を停止・測定の完了とは判断しません。
Cameraは窓と同じ処理で開始・停止・撮影し、Shutterは状態保持など、アプリごとの違いを維持します。
