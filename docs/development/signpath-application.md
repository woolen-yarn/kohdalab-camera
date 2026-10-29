# SignPath application preparation

Status: **draft; not submitted**. Current releases remain unsigned. This file is
an application brief and readiness checklist, not a claim of eligibility or an
enabled signing pipeline. No account, GitHub App permission, secret or signing
request was created as part of preparation.

## Project brief for the application

- Project: KohdaLab Camera.
- Repository/home page: https://github.com/woolen-yarn/kohdalab-camera
- Releases: https://github.com/woolen-yarn/kohdalab-camera/releases
- Existing release: v0.1.11, Windows x64 portable ZIP and per-user EXE installer.
- Project license: MIT; dependency and toolchain notices are retained in
  `THIRD_PARTY_NOTICES.md` and `third-party-licenses/`.
- Repository owner: https://github.com/woolen-yarn
- Code signing policy: [code-signing-policy.md](../reference/code-signing.md).
- Contact: the maintainer must supply an email in the application form; no
  contact address or university representation is inferred from repository ownership.

Suggested project description:

> KohdaLab Camera is an MIT-licensed Rust desktop application for laboratory
> camera acquisition, live image adjustment and reproducible image export. Its
> long-term goal is a common workflow for multiple camera backends. Current
> physical support is limited to one tested SHODENSHA GR300 configuration on
> Windows x64 with an existing WinUSB binding. Simulation is available without
> hardware. The application preserves existing drivers and does not install
> drivers, services or security-policy changes. Manufacturer SDK support is
> deferred; no proprietary manufacturer SDK or DLL is bundled.

Suggested reason for requesting signing:

> We distribute a Windows application and acquisition helper through GitHub
> Releases. We would like users to verify the origin and integrity of releases
> without purchasing an organization code-signing certificate. We intend to
> build from public source in CI, keep release approval with a human maintainer,
> and sign the application, helper, uninstaller and installer using a provenance-
> verified pipeline accepted by SignPath. Current releases are explicitly unsigned.

The unsigned build implementation is now available; successful run evidence is
recorded below after qualification. No SignPath integration is active.
Use [the official application page](https://signpath.org/apply); adapt this brief
to the fields shown there rather than treating it as a fixed form schema.

## Evidence and remaining conditions

| Condition | Current evidence / required action |
| --- | --- |
| Public OSS project | Public repository and MIT root license confirmed. Check the Windows-linked dependency license closure and installer/toolchain exceptions before asserting full license compliance. OSI-approved alternative license choices are not commercial dual-licensing. |
| Maintained and released | Public source and v0.1.11 release exist. Do not imply a large user base or longstanding reputation; project reputation remains subject to Foundation review. |
| Documented function | README describes supported camera, limitations, exports and installation. |
| No proprietary bundled SDK | Official SDK integration is deferred. Disclose the public ScopeTek-family protocol implementation and existing provenance notices; do not describe the backend as manufacturer-certified. |
| Repository/source control | Owner account is identified. Human must confirm authority, team roles and MFA on both services. |
| Policy and privacy | Draft policy is linked from README. Provider attribution is conditional until acceptance/use. Signed release pages will need the policy link. |
| Verified automated build | Implemented in `.github/workflows/windows-unsigned.yml`; qualified by successful run `39187983298` for commit `14d7cc156e496b6f2e8ea65c00414e05b4cf8cdb`. GitHub build evidence does not itself mean SignPath-verified origin. |
| PE metadata | GUI/helper resources now derive product/version from the root package. CI verifies GUI, helper, exported uninstaller and Setup metadata against the same version. |
| Uninstaller signing | NSIS exports the exact unsigned uninstaller for a separate signing artifact. Provider enrollment and a reviewed signed-input/import stage remain necessary before a fully signed release; disclose this in the application. |
| Installation behavior | v0.1.11 CI install/update/uninstall and unrelated-file preservation passed. The exact CI runtime was installed and hash/metadata/startup-checked on the workstation. See `validation-0.1.11.json`; no new physical capture test is claimed. |
| Application approval | Not submitted. The human maintainer must review provider terms and Foundation publisher identity before enrollment. |

## Qualified build evidence for the application

- [Successful Windows run 39187983298](https://github.com/woolen-yarn/kohdalab-camera/actions/runs/39187983298).
- Source commit: `14d7cc156e496b6f2e8ea65c00414e05b4cf8cdb`.
- Unsigned distribution GitHub artifact ID: `12159300328`.
- Initial signing-input GitHub artifact ID: `12159116912`.
- [Scoped validation and artifact digests](../validation/validation-0.1.11.json).

Both artifact file checksum manifests and ZIP CRCs were verified after download.
GitHub artifact retention is 30 days; rerun the workflow if artifacts have expired
when applying. Release assets are retained separately, but do not substitute a
release URL for the original GitHub artifact ID in a future signing request.
There is enough technical evidence to request eligibility/enrollment review;
provider acceptance, MFA/role confirmation and production signing setup remain
external prerequisites. The initial portability/test-setup failures are retained
in Actions history; the linked run is the successful corrected implementation.

## Questions to resolve with SignPath

1. Confirm eligibility of this newly published research-camera project; disclose
   its actual release history without inventing popularity or adoption.
2. Confirm whether one human maintainer may hold the proposed committer,
   reviewer and approver roles, and what repository protections are required.
3. Confirm acceptable build-origin verification for Rust GNU Windows builds and
   the unmodified NSIS stubs/System plugin and toolchain runtime components.
4. Confirm a supported sequence for signing nested executable files, the NSIS
   generated uninstaller, and final Setup with manual release approval.

## Implemented unsigned build and proposed signing integration

See [continuous integration](continuous-integration.md) for the workflow's inputs,
checks, artifact structure and limits. It uses read-only repository permissions,
pinned Actions and Rust, checks the source identity, builds both executables,
compiles Setup with a hash-verified NSIS, and exports the unsigned uninstaller.
It does not require SignPath credentials or access to a physical camera.

The draft [artifact configuration](../../.signpath/first-party-files.draft.xml) and
[enrollment notes](../../.signpath/README.md) describe the initial three-file signing
request. The draft is well-formed XML, not a provider-validated configuration.

After enrollment, ask SignPath to accept the original GitHub artifact ID with
origin verification, source-branch restrictions and manual human approval. Then
qualify signed-input packaging/uninstaller import and final Setup signing before
publishing any signed release. Confirm the supported nested sequence with the
provider; unsigned Setup must not be described as fully signed because its loose
input executables were signed separately. Keep existing unsigned release hashes
and publish future signed bytes as a new version.

The exact provider identifiers, GitHub App authorization and credentials are not
invented or configured in advance. Neither a signing request nor enrollment is
part of this preparation.

## Maintainer-only enrollment steps

- Confirm the Foundation publisher identity is acceptable and identify the
  human contact/approver; confirm MFA rather than sharing recovery codes.
- Review current [Foundation conditions](https://signpath.org/terms.html) and
  the application page. Resolve the readiness items above before representing
  the pipeline as operational.
- Complete account/terms/application submission personally. Preparation does not
  authorize enrollment or acceptance of terms on the maintainer's behalf.

Prepared against official conditions on 2026-10-19. Acceptance and review time
are not guaranteed. No camera run or new binary release is needed for these
application preparation. The CI/metadata update has its own unsigned qualification;
physical capture evidence remains scoped to its original version.

---

# SignPath 申請準備 / SignPath application preparation（日本語）

状態は **draft; not submitted** です。current release は unsigned のままで、本書は application brief/readiness checklist であり eligibility や enabled signing pipeline の主張ではありません。preparation の一部として account、GitHub App permission、secret、signing request は作成していません。

## 申請用 project brief / Project brief for the application

- Project: KohdaLab Camera。repository/home page: https://github.com/woolen-yarn/kohdalab-camera 。releases: https://github.com/woolen-yarn/kohdalab-camera/releases 。
- Existing release: v0.1.11、Windows x64 portable ZIP と per-user EXE installer。
- Project license: MIT。dependency/toolchain notice は `THIRD_PARTY_NOTICES.md` と `third-party-licenses/` に保持します。repository owner: https://github.com/woolen-yarn 。code signing policy: [code-signing-policy.md](../reference/code-signing.md)。
- Contact は application form に maintainer が email を入力し、repository ownership から contact/university representation を推測しません。

推奨 project description は、MIT-licensed Rust desktop application が laboratory camera acquisition/live image adjustment/reproducible export を行い、長期目標は multiple backend の common workflow であること、physical support は既存 WinUSB binding の Windows x64 上で検証した SHODENSHA GR300 一構成に限ること、Simulation は hardware なしで使えること、application は existing driver/service/security policy を install/change せず official SDK/manufacturer DLL を同梱しないこと、です。

推奨 signing request 理由は、GitHub Releases で Windows application/acquisition helper を配布し organization code-signing certificate を購入せず origin/integrity を確認できるようにしたいこと、public source から CI build し human maintainer が release approval を保持して SignPath が受け入れる provenance-verified pipeline で application/helper/uninstaller/installer を sign する意図、current release が明示的に unsigned であることです。unsigned build implementation は available で successful run evidence は qualification 後に記録しますが SignPath integration は active ではありません。実際の field に合わせ [official application page](https://signpath.org/apply) を使い、本書を固定 form schema としません。

## 根拠と未完了条件 / Evidence and remaining conditions

public OSS/MIT root license、public source/v0.1.11 release、README の supported camera/limitation/export/install 記述、official SDK integration deferred、owner account、draft policy、`.github/workflows/windows-unsigned.yml` の verified automated build、root package 由来の PE metadata、unsigned uninstaller export、v0.1.11 install/update/uninstall/unrelated-file preservation は準備済み根拠です。ただし Windows-linked dependency license closure/installer-toolchain exception、project reputation、ScopeTek-family protocol provenance、human authority/team role/MFA、provider attribution、provider enrollment/approved signed-input/import stage、application submission はそれぞれ未確認または未実施です。successful run `39187983298`（commit `14d7cc156e496b6f2e8ea65c00414e05b4cf8cdb`）、unsigned distribution artifact ID `12159300328`、initial signing-input artifact ID `12159116912`、[scoped validation and artifact digests](../validation/validation-0.1.11.json) を参照してください。GitHub build evidence は SignPath-verified origin ではありません。

artifact checksum manifest/ZIP CRC は download 後 verify 済みです。GitHub artifact retention は 30 days で、expired 時は workflow を rerun します。release asset は別に保持しますが将来 signing request で original GitHub artifact ID の代わりに release URL を使いません。eligibility/enrollment review を求める technical evidence はありますが、provider acceptance、MFA/role confirmation、production signing setup は外部 prerequisite です。Actions history には初期 portability/test-setup failure を残し、リンクした run は corrected implementation の成功記録です。

## SignPath と解決する質問と実装済み unsigned build

新規 research-camera project の eligibility、actual release history、一人の human maintainer の committer/reviewer/approver role と必要 protection、Rust GNU Windows build/unaltered NSIS stub/System plugin/toolchain runtime の build-origin verification、nested executable/NSIS generated uninstaller/final Setup の human-approved signing sequence を確認します。[continuous integration](continuous-integration.md) は read-only permission、pinned Actions/Rust、source identity check、両 executable build、hash-verified NSIS、unsigned uninstaller export を使い SignPath credential/physical camera を必要としません。[artifact configuration](../../.signpath/first-party-files.draft.xml) と [enrollment notes](../../.signpath/README.md) は initial three-file signing request を記載しますが、draft XML は well-formed でも provider-validated configuration ではありません。

enrollment 後は original GitHub artifact ID を provider が origin verify し、source-branch restriction と manual human approval を適用できるよう依頼します。その後 signed-input packaging/uninstaller import/final Setup signing を qualify してから signed release を publish します。nested sequence は provider と確認し、unsigned Setup を fully signed と表現しません。provider identifier/GitHub App authorization/credential を事前に捏造・設定せず、signing request/enrollment は本準備に含みません。

既存 unsigned release の hash は保持し、将来の signed byte は **new version** として publish します。既存 release asset を置換したり、過去の unsigned byte を signed と表現したりしません。

## Maintainer-only enrollment steps

Foundation publisher identity を確認し human contact/approver と MFA を確認します（recovery code は共有しません）。current [Foundation conditions](https://signpath.org/terms.html) と application page を読み、pipeline operational と表現する前に readiness item を解決します。account/terms/application submission は maintainer 本人が行い、preparation は代理 enrollment/terms acceptance を許可しません。2026-10-19 の official condition を基に準備し acceptance/review time は保証されません。application preparation に camera run/new binary release は不要で、CI/metadata update は独自 unsigned qualification、physical capture evidence は元 version の scope に留まります。
