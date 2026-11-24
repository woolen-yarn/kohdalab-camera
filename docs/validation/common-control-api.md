# Common control API validation

> **Provenance / 来歴**
> This is a point-in-time measurement snapshot; counts below are not comparable
> across records. / これは特定時点の測定スナップショットで、各記録間で件数を比
> 較できません。
> - Date / 日付: 2026-10-29
> - Target / 対象: 0.1.13
> - Platform / 環境: macOS ARM64
> - Command / コマンド: device-free `cargo` suites and the shared Python SDK suite (see items below) / device-free な `cargo` スイートと共通 Python SDK スイート（下記項目を参照）

Date: 2026-10-29. Target: 0.1.13. Host: macOS ARM64.

- Device-free Rust tests: 188 passed across the selected application/workspace and common contract suites.
- Shared Python SDK: 9 tests passed, including authenticated loopback errors, exact job links, ambiguous timeout boundaries and pending cleanup.
- Actual macOS fake GUI: authenticated describe/status/config and unsupported mutation refusal passed. One hardware-dependent test remains ignored. Full strict clippy currently has pre-existing warnings; it is not claimed as passing.
- Physical instruments were not opened or qualified for this change. Windows packaging/CI evidence belongs to the matching PR/release; these local tests are not a Windows hardware pass.

The existing native API, scientific storage and product-specific safety policy remain separate from the additive common namespace. See [the common contract](../reference/common-control-api.md).
