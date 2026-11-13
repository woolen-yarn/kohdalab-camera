# SignPath enrollment inputs

No SignPath account, certificate, GitHub App or API token is configured. The
unsigned workflow uploads `signpath-inputs-<commit>` as a separate GitHub artifact
containing the GUI, helper, exact exported NSIS uninstaller and build-origin
record. GitHub's artifact ID and run URL, not this self-authored JSON alone, are
the future trusted-build-system inputs.

`first-party-files.draft.xml` is an enrollment-review draft based on the official
[artifact examples](https://docs.signpath.io/artifact-configuration/examples).
It is not deployed or provider-validated. Submit the signing-input artifact with
its original GitHub artifact ID after enrollment. The configuration deliberately
does not sign the unsigned Setup or claim to repackage its nested executables.

After approval, qualify two stages with SignPath:

1. Sign the three files in the signing-input artifact, checking product/version,
   approved source branch and commit origin, and requiring a human approver.
2. Rebuild the payload manifest/ZIP from returned signed runtime bytes, import
   the signed uninstaller using a reviewed NSIS path, then sign and verify final
   Setup. Current tooling exports the uninstaller but does not yet import signed
   replacements. Do not publish a partially signed installer as fully signed.

The second stage requires provider integration and cannot be validated before
enrollment. It is not a prerequisite for requesting eligibility review: disclose
this plan in the application rather than claiming operational signing.

Do not place API tokens, certificate keys, private images, or hardware-session
records here. See [application preparation](../docs/development/signpath-application.md).

---

# SignPath enrollment inputs（日本語）

SignPath account、certificate、GitHub App、API token は設定していません。unsigned workflow は GUI、helper、正確に export した NSIS uninstaller、build-origin record を含む `signpath-inputs-<commit>` を別の GitHub artifact として upload します。将来の trusted-build-system input は、この自己作成 JSON だけではなく GitHub artifact ID と run URL です。

`first-party-files.draft.xml` は公式 [artifact examples](https://docs.signpath.io/artifact-configuration/examples) に基づく enrollment review 用 draft です。deploy 済みでも provider 検証済みでもありません。enrollment 後、元の GitHub artifact ID を持つ signing-input artifact を提出します。この設定は unsigned Setup を署名せず、nested executable の再 packaging を主張しません。

承認後は SignPath で二段階を適格化します。

1. signing-input artifact の 3 ファイルを署名し、product/version、承認済み source branch と commit origin を確認し、人間の approver を必須にします。
2. 返却された signed runtime bytes から payload manifest/ZIP を再構築し、review 済み NSIS path で signed uninstaller を import してから final Setup を署名・検証します。現行 tooling は uninstaller を export しますが signed replacement の import はまだ行いません。部分署名 installer を fully signed として公開してはいけません。

第二段階には provider integration が必要で、enrollment 前には検証できません。ただし eligibility review を求める前提ではありません。運用中の署名を主張せず、この計画を application に開示します。

ここに API token、certificate key、private image、hardware-session record を置かないでください。[application preparation](../docs/development/signpath-application.md) を参照してください。
