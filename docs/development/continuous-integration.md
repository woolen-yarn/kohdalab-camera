# Unsigned Windows continuous integration

The [Windows unsigned build](https://github.com/woolen-yarn/kohdalab-camera/actions/workflows/windows-unsigned.yml)
builds public source on a disposable GitHub-hosted Windows 2025 runner. Relevant
pushes to `main` and an explicit manual workflow dispatch can start it. It does
not sign, create a GitHub release, install drivers, or use a physical camera.

## Inputs and checks

- Rust 1.99.1 and the Windows GNU target are selected explicitly. The hosted
  image's GNU linker/resource compiler versions are recorded in the run log.
- Cargo uses the committed lockfile. First-party Actions use fixed commit SHAs;
  checkout credentials are not retained and the workflow token is read-only.
- `.gitattributes` keeps source checkout line endings consistent. Source identity
  uses platform-independent relative-path ordering and is verified, not rewritten
  by CI. A stale marker stops the build.
- Camera driver-policy and headless GUI unit tests run without hardware. Both
  first-party executables embed the root package version and product name.
- The NSIS 3.12 archive is hash-verified before extraction. Setup is machine-wide and
  does not start acquisition. The compiler exports the exact unsigned uninstaller
  without running Setup to obtain it.
- A disposable install checks all four executable types' metadata and unsigned
  status, runtime hashes, exported/installed uninstaller equality, startup report,
  same-version update, normal uninstall and unrelated-file preservation.

The test installer creates shortcuts and HKCU application entries on the
disposable runner, then removes them through normal uninstall. Never run this
CI validation script against a user's existing installation: it is designed for
an isolated build runner. Local workstation updates use the normal Setup path.

## Artifacts and evidence

Successful runs upload two artifacts for 30 days:

| Artifact | Contents |
| --- | --- |
| `windows-unsigned-<commit>` | Portable/source ZIPs, Setup, complete portable payload, startup/CI validation records, build origin and SHA-256 checksums |
| `signpath-inputs-<commit>` | GUI, helper, exported uninstaller, build origin and SHA-256 checksums; enrollment preparation only |

`BUILD-ORIGIN.json` records the public source commit, workflow/run identity,
source marker and Rust tools. It is descriptive evidence, not an attestation or
SignPath-verified origin. After enrollment, SignPath must retrieve the original
GitHub artifact using the provider's supported integration and enforce its
source/approval policies. Copying files elsewhere does not preserve that trusted
artifact identity.

The workflow does not consume `.local/`, camera images, credentials or private
validation data. The only uploaded paths are generated `dist/` and
`signing-inputs/`. No SignPath API call, account integration, signing secret or
automatic release approval exists.

## From CI to a public release

Check the successful run's commit and artifact digests, download its artifacts,
and verify individual checksums before distributing them. Retain the run URL and
scoped validation record with the release. Upload exact verified bytes as a new
version; never overwrite an old release to attach changed binaries. CI checks do
not prove physical camera capture, optical quality or Windows download-reputation
behavior. Keep the last physical validation identified by its own version/hash.

See [SignPath application preparation](signpath-application.md) for the remaining
human enrollment steps and provider-dependent signed-installer work.

---

# unsigned Windows 継続的インテグレーション / Unsigned Windows continuous integration（日本語）

[Windows unsigned build](https://github.com/woolen-yarn/kohdalab-camera/actions/workflows/windows-unsigned.yml) は disposable GitHub-hosted Windows 2025 runner で public source を build します。`main` への対象 push と明示的 manual workflow dispatch で開始でき、署名、GitHub release 作成、driver install、physical camera 使用は行いません。

## 入力と確認 / Inputs and checks

Rust **1.99.1** と Windows GNU target を明示選択し、hosted image の GNU linker/resource compiler version を run log に記録します。Cargo は committed lockfile、first-party Actions は fixed commit SHA を使い、checkout credential は保持せず workflow token は read-only です。`.gitattributes` は checkout line ending を揃え、source identity は platform-independent relative-path order を用いて verification のみ行い CI が書換えません。stale marker は build を停止します。camera driver-policy/headless GUI unit test は hardware なしで実行し、両 first-party executable は root package version/product name を埋込みます。NSIS **3.12** archive は hash-verify 後に extract し、Setup は machine-wide で acquisition を始めません。compiler は Setup を実行せず exact unsigned uninstaller を export します。disposable install は 4 executable type の metadata/unsigned status、runtime hash、exported/installed uninstaller equality、startup report、same-version update、normal uninstall、unrelated-file preservation を確認します。installer が作る shortcut/HKCU entry は runner の normal uninstall で除去し、この script を user の既存 installation に対して実行してはいけません。

## Artifact と根拠 / Artifacts and evidence

successful run は 30 日間、`windows-unsigned-<commit>`（portable/source ZIP、Setup、complete portable payload、startup/CI validation record、build origin、SHA-256 checksum）と `signpath-inputs-<commit>`（GUI、helper、exported uninstaller、build origin、SHA-256 checksum、enrollment preparation のみ）を upload します。`BUILD-ORIGIN.json` は public source commit、workflow/run identity、source marker、Rust tool を記録する descriptive evidence であり attestation/SignPath-verified origin ではありません。enrollment 後も provider の supported integration で original GitHub artifact を取得し source/approval policy を強制する必要があり、別場所への copy は trusted artifact identity を保存しません。workflow は `.local/`、camera image、credential、private validation data を消費せず、upload path は generated `dist/` と `signing-inputs/` のみです。SignPath API call/account integration/signing secret/automatic release approval はありません。

## CI から public release へ / From CI to a public release

successful run の commit/artifact digest を確認し artifact を download、個別 checksum を verify してから distribute します。run URL と scoped validation record を release と共に保持し、exact verified byte は新 version として upload します。変更 binary を添付するために古い release を上書きしません。CI check は physical camera capture、optical quality、Windows download-reputation behavior を証明しないため、最後の physical validation は独自 version/hash で示します。残る human enrollment/provider-dependent signed-installer work は [SignPath application preparation](signpath-application.md) を参照してください。
