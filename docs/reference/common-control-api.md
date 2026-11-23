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
Camera currently offers read-only `describe/status/config`; inspect it without a
context manager because context entry requests ownership. Camera capture/start/
stop/ownership are not exposed by this adapter. Do not substitute internal camera
helper IPC for a public GUI API. A Windows save dialog can keep the GUI from
handling `control.status` within the two-second request deadline. If status
times out while the dialog is open, query it again after closing the dialog.

Windows の保存ダイアログを開いている間は、GUI が2秒の期限内に
`control.status` を処理できない場合があります。ダイアログを閉じてから
状態を再取得してください。

When started with `--gui-api`, the toolbar and the camera picker show
**External control · Read only**. KohdaLab SLM uses the same wording. There
is no Take control or Stop button because the adapter has no writer lease.
The window keeps full control.

`--gui-api` で起動すると、ツールバーとカメラ選択画面に **External control · Read only**
を表示します（SLM と同じ表記）。書き込み用のリースがないため、「制御を取得」や
「停止」ボタンはありません。

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
operations that Camera does not implement reach the GUI and return
`error.code: "unsupported"` from the running application. The SDK's local
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
| Camera | Read-only GUI status/configuration | Acquisition helper remains private; no new device commands |

The common contract does not make device stop policies identical. Read the
advertised methods and safety fields from the running release. Native API
versions, app versions and common protocol version are separate.

GUI経由の共通窓口を追加しました。既存API・測定・CSV・停止方針は維持します。
対応操作は `describe()` で確認し、複数起動時は対象を明示してください。
通信失敗時に操作を自動再送せず、受付応答を停止・測定の完了とは判断しません。
Cameraは読み取り専用、Shutterは状態保持など、アプリごとの違いを維持します。
