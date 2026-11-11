# Contributing

Use English for project documentation, user-facing interface text, issue and
pull-request descriptions, and new code comments. Preserve third-party license
texts in their original form. Non-English test fixtures may remain when needed
to exercise Unicode handling.

## Adding camera support

KohdaLab Camera aims to provide one capture workflow across multiple camera
families. Before implementing a backend, document:

- Manufacturer, commercial model, USB/device identifiers, sensor, and firmware scope.
- The public API, protocol documentation, or redistributable SDK to be used,
  including provenance and dependency licenses.
- Supported operating systems, setup requirements, acquisition modes, controls,
  and pixel formats.
- Timestamp and frame-rate semantics, original-data preservation, ownership,
  shutdown, and failure behavior.
- A bounded hardware-validation plan and the exact limits of any resulting claims.

Do not incorporate manufacturer binary reverse-engineering results, private USB
traffic captures, proprietary initialization data, or legacy application assets.
Keep vendor-specific details behind backend boundaries. Simulation and unit
tests help development but do not qualify physical devices. Update
[the support matrix](docs/reference/hardware.md) only to the extent supported by
actual validation.

## Changes and validation

See [code structure and maintenance boundaries](docs/development/architecture.md) before
moving responsibilities or shared defaults between modules.

Read the relevant implementation before editing. Use targeted tests appropriate
to the change; documentation-only edits do not require physical camera runs.
Do not change recorded results, thresholds, or historical checksums to make a
check pass. Preserve license notices in source and portable distributions.

## Dependency updates

Dependabot checks the Rust workspace weekly and GitHub Actions monthly.
Dependency updates require review; automatic merging is not configured.
Apply the reviewed Cargo manifest and lockfile changes to a maintainer branch
based on `main`, then run this command from the repository root.

```sh
python3 scripts/check_third_party_licenses.py --update
```

The command reads locked Cargo metadata for all targets and checks the directories
under `third-party-licenses/`. For each missing crate version, add the exact
upstream license and notice files, including bundled attributions where applicable.
Record the upstream URL and SHA-256 for each added file using the existing
`RECOVERY.tsv` format. Review and remove directories for versions no longer in the
lockfile. Keep the tool and runtime licenses. Rerun the same command after making
these corrections.

Once the inventory passes, the command regenerates the crate table in
`THIRD_PARTY_NOTICES.md` and refreshes `RECORDER-SOURCE-ID`. Review and commit the
dependency changes, notices, license files, and source ID together. Keep
`nusb` at 0.2.7. A separate update to that dependency requires physical-camera
validation.

Run the Workspace tests and Windows unsigned build workflows on the maintainer
branch before merging. Windows unsigned build also supports manual dispatch.
Both the license and source-ID checks run in CI without modifying the checkout.

## Native input diagnostics

For a bounded, simulation-only investigation of input delivery, build the
application with `--features input-diagnostics`. Set `KOHDALAB_INPUT_TRACE` to a
new writable file before launch. The diagnostic build disables physical camera
selection and the physical-adapter CLI path, and requests normal closure after
180 seconds. Do not use this feature in release distributions.

Only pointer, focus, and navigation-key metadata are recorded. Text and paste
contents are omitted; each frame inspects at most 16 events and the file is
capped at 256 records of at most 4096 characters. A diagnostic trace can establish
which events reached the application; it cannot establish that an external
remote-input tool actually sent a click.

After changing build inputs, run `python3 scripts/update_source_identity.py`
before the release build. The diagnostic marker is supplementary; release
manifest commit and binary hashes identify the distributed artifacts.

---

# 貢献ガイド / Contributing（日本語）

プロジェクト文書、ユーザー向け UI 文言、Issue/PR の説明、新しいコードコメントは英語を基準にし、このリポジトリでは日本語の対訳も維持します。第三者ライセンス文は原文のまま保持してください。Unicode の扱いを検証する必要がある非英語テスト fixture は残せます。

## カメラ対応の追加 / Adding camera support

KohdaLab Camera は複数のカメラ系列に共通の取得ワークフローを提供することを目標にしています。バックエンドを実装する前に、製造元・製品名・USB/device ID・センサー・firmware 範囲、利用する公開 API/プロトコル/再配布可能 SDK と来歴・依存ライセンス、対象 OS・セットアップ・取得モード・制御・pixel format、timestamp/frame rate の意味・元データ保持・所有権・終了・失敗時挙動、そして限定された実機検証計画と主張可能な範囲を文書化してください。

メーカー binary の reverse engineering 結果、非公開 USB traffic capture、専有初期化データ、旧アプリ資産は取り込まないでください。ベンダー固有の詳細は backend 境界の内側に置きます。Simulation と unit test は開発を助けますが物理装置の適格性を示しません。[support matrix](docs/reference/hardware.md) の更新は実際の検証で裏付けられる範囲に限ります。

## 変更と検証 / Changes and validation

責務や共有 default を移す前に [code structure and maintenance boundaries](docs/development/architecture.md) を読んでください。編集前に関連実装を読み、変更に見合う対象テストを実行します。文書のみの変更に物理カメラ実行は不要です。チェックを通すために記録済み結果、閾値、履歴 checksum を変更してはいけません。ソースと portable distribution のライセンス通知を保持してください。

## 依存関係更新 / Dependency updates

Dependabot は Rust workspace を毎週、GitHub Actions を毎月確認します。自動マージは設定されていません。レビュー済みの Cargo manifest と lockfile の変更を `main` ベースのメンテナーブランチに適用し、リポジトリ直下で次を実行します。

```sh
python3 scripts/check_third_party_licenses.py --update
```

全ターゲットの locked Cargo metadata を読み、`third-party-licenses/` を照合します。不足する crate の版には、対応する上流のライセンス文と通知を追加し、同梱素材の帰属表示も必要に応じて収録します。追加したファイルごとに上流 URL と SHA-256 を既存の `RECOVERY.tsv` 形式で記録します。lockfile から外れた版のディレクトリは確認して削除し、ツールとランタイムのライセンスは保持します。修正後に同じコマンドを再実行します。

照合に通ると、`THIRD_PARTY_NOTICES.md` の crate 表と `RECORDER-SOURCE-ID` が更新されます。依存関係の差分、通知、ライセンス文、ソース ID を確認し、まとめてコミットします。`nusb` は 0.2.7 を維持します。この依存の更新は実機検証を伴う別作業です。

マージ前にメンテナーブランチで Workspace tests と Windows unsigned build を実行します。Windows unsigned build は手動実行にも対応しています。CI のライセンス照合とソース ID チェックはチェックアウトを書き換えません。

## Native input diagnostics

入力配送を限定的に Simulation のみで調べる場合は `--features input-diagnostics` でビルドし、起動前に新しい書込み可能ファイルへ `KOHDALAB_INPUT_TRACE` を設定します。この診断 build は物理カメラ選択と physical-adapter CLI path を無効化し、180 秒後に通常終了を要求します。配布物には使用しません。

記録するのは pointer、focus、navigation-key の metadata のみで、text/paste 内容は記録しません。各 frame は最大 16 events、ファイルは最大 256 records・各 4096 characters です。trace はどの event がアプリに届いたかを示せますが、外部 remote-input tool が click を送ったことの証明にはなりません。build input 変更後は release build 前に `python3 scripts/update_source_identity.py` を実行してください。診断 marker は補助情報であり、配布成果物の同定には release manifest の commit と binary hash を用います。
