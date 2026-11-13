# Security policy / セキュリティ方針

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/woolen-yarn/kohdalab-camera/security/advisories) when **Report a vulnerability** is available. If it is unavailable, use [GitHub Issues](https://github.com/woolen-yarn/kohdalab-camera/issues/new) to request a private contact channel. Keep that public request limited to asking a maintainer to arrange private contact.

In the private report, include the affected version or commit, operating system,
distribution type, and whether the issue occurred in Simulation or with a physical
camera. Describe the observed behavior and the conditions needed to reproduce it.
Share sanitized logs if they help. Remove GUI API tokens, device identifiers,
local paths, and private measurement data before sharing.

Reports can cover issues in the Windows installer or portable package, local
configuration or capture files, device ownership and cleanup, or the
authentication boundary of the opt-in read-only GUI API. See the
[hardware support matrix](docs/reference/hardware.md) for the qualified camera
configuration.

## 脆弱性の報告

**Report a vulnerability** が表示される場合は、[GitHub の非公開脆弱性報告](https://github.com/woolen-yarn/kohdalab-camera/security/advisories)を使ってください。利用できない場合は、[GitHub Issues](https://github.com/woolen-yarn/kohdalab-camera/issues/new) で保守担当者に非公開の連絡経路を尋ねてください。公開する Issue には連絡経路の依頼だけを書いてください。

非公開の報告には、対象の版またはコミット、OS、配布形式、Simulation と実機のどちらで起きたかを記載してください。観測した挙動と再現条件を記載し、必要に応じてログを添えてください。共有前に GUI API のトークン、機器識別子、ローカルパス、非公開の測定データを取り除いてください。

Windows のインストーラーや portable 版、ローカル設定や撮影ファイル、機器の所有権と終了処理、任意で有効化する読み取り専用 GUI API の認証境界に関する問題が対象です。検証済みの機器構成は[対応機器一覧](docs/reference/hardware.md)を参照してください。
