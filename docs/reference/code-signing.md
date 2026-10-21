# Code signing policy

## Current status

Published Windows releases remain **unsigned**. SignPath application preparation
does not mean that an application has been submitted, accepted or connected to
the repository. No signing credentials or certificate are configured.

The project is considering SignPath Foundation's free open-source program.
Foundation certificates identify **SignPath Foundation**, not Tohoku University
or KohdaLab, as the certificate publisher. Product metadata remains KohdaLab
Camera. A laboratory/organization certificate is a separate future option.

See the [application preparation](../development/signpath-application.md) for evidence, proposed
roles and outstanding checks. Acceptance is at the Foundation's discretion.

## Intended signing scope

Only reviewed KohdaLab Camera releases built from a recorded public source
commit may be submitted. Intended first-party targets are the GUI executable,
`continuous-helper.exe`, the generated uninstaller and Windows Setup. Third-party
code and installer stubs must be disclosed and accepted under the provider's
rules; they are not presented as first-party source. Existing release assets
must not be overwritten or silently described as signed.

Signing will require a human release approver, verified build provenance,
matching product/version metadata, and verification of every resulting signature
and timestamp. Checksums and manifests must describe the final signed bytes.
Neither pull requests nor automatic tag pushes may approve a signing request.

## Roles

Repository owner: [woolen-yarn](https://github.com/woolen-yarn).
Proposed committer, reviewer and signing approver: the human maintainer operating
that account. The maintainer must confirm these assignments and any additional
members before applying; the role list is not an assertion that approval access
has already been granted. Automated coding agents are not signing approvers.

All participating humans must enable multi-factor authentication for GitHub and
SignPath. Account status and repository protection settings remain to be checked
by the maintainer; this document does not certify them.

## Privacy

The application does not automatically upload camera images, settings or
diagnostics, and does not include an automatic update or analytics client.
Acquisition telemetry in the source means local acquisition diagnostics, not a
remote analytics service. Preferences, captures and optional diagnostic reports
are written to disk. Saving to a network/shared folder chosen by the user may
transfer those files through the operating system.

Build services fetch dependencies and process public source/build artifacts;
that activity is separate from running the camera application. A future signing
integration would transfer release artifacts and provenance to SignPath only,
not camera captures or private hardware-validation records. Repository users
should avoid posting sensitive images or diagnostic contents in public issues.

## Attribution after acceptance

The following provider-required attribution is prepared for activation only after
acceptance and actual use; it is **not a claim about current releases**:

> Free code signing provided by [SignPath.io](https://signpath.io/), certificate
> by [SignPath Foundation](https://signpath.org/).

When signing begins, link this policy from the signed release/download pages,
confirm the named human roles, and update the current-status section with the
first verified signed version. Do not promise that signing eliminates all
SmartScreen or organization-policy warnings.

Reference: [SignPath Foundation conditions](https://signpath.org/terms.html),
reviewed on 2026-10-19.

---

# コード署名ポリシー / Code signing policy（日本語）

## 現在の状態 / Current status

公開済み Windows release は **unsigned** です。SignPath application preparation は application が submitted、accepted、repository に接続済みであることを意味しません。signing credential/certificate は設定していません。検討中の SignPath Foundation free open-source program の certificate publisher は **SignPath Foundation** であり、Tohoku University や KohdaLab ではありません。product metadata は KohdaLab Camera のままです。laboratory/organization certificate は別の将来選択肢です。[application preparation](../development/signpath-application.md) に根拠、提案 roles、未完了 checks を示します。acceptance は Foundation の裁量です。

## 想定する署名範囲 / Intended signing scope

記録済み public source commit から build した review 済み KohdaLab Camera release だけを submit できます。最初の first-party target は GUI executable、`continuous-helper.exe`、generated uninstaller、Windows Setup です。third-party code と installer stub は provider rule に従って開示・受諾される必要があり first-party source として提示しません。既存 release asset を上書きしたり、署名済みと黙って表現してはいけません。

署名には human release approver、verified build provenance、一致する product/version metadata、全 signature/timestamp の検証が必要です。checksum/manifest は最終 signed byte を記述します。pull request と automatic tag push は signing request を承認できません。

## 役割と privacy / Roles and privacy

repository owner は [woolen-yarn](https://github.com/woolen-yarn) です。提案する committer/reviewer/signing approver はその account を運用する human maintainer ですが、申請前に本人が assignment と追加 member を確認しなければならず、既に approval access があるという主張ではありません。automated coding agent は signing approver ではありません。参加する人は GitHub と SignPath で multi-factor authentication を有効化します。account status と repository protection は maintainer が確認する事項で、本書は証明しません。

アプリは camera image/settings/diagnostics を自動 upload せず、automatic update/analytics client も含みません。source の acquisition telemetry は local diagnostic を意味し remote analytics ではありません。preferences、captures、optional diagnostic report は disk に書かれ、ユーザーが選んだ network/shared folder では OS を通じて transfer され得ます。build service が public source/build artifact を処理することは camera application 実行とは別です。将来の signing integration は release artifact/provenance だけを SignPath に渡し、camera capture/private hardware-validation record は渡しません。public issue に sensitive image/diagnostic を投稿しないでください。

## 承認後の attribution / Attribution after acceptance

次の attribution は acceptance と actual use の後にだけ有効化し、現 release の主張にはしません: “Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).” 署名開始時は signed release/download page から本 policy を link し、human role を確認し、最初の verified signed version で current-status を更新します。署名が全 SmartScreen/organization-policy warning をなくすとは約束しません。参照: [SignPath Foundation conditions](https://signpath.org/terms.html)、2026-10-19 確認。
