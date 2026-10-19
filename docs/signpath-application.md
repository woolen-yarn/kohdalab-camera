# SignPath application preparation

Status: **draft; not submitted**. Current releases remain unsigned. This file is
an application brief and readiness checklist, not a claim of eligibility or an
enabled signing pipeline. No account, GitHub App permission, secret or signing
request was created as part of preparation.

## Project brief for the application

- Project: KohdaLab Camera.
- Repository/home page: https://github.com/woolen-yarn/kohdalab-camera
- Releases: https://github.com/woolen-yarn/kohdalab-camera/releases
- Existing release: v0.1.10, Windows x64 portable ZIP and per-user EXE installer.
- Project license: MIT; dependency and toolchain notices are retained in
  `THIRD_PARTY_NOTICES.md` and `third-party-licenses/`.
- Repository owner: https://github.com/woolen-yarn
- Code signing policy: [code-signing-policy.md](code-signing-policy.md).
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

These statements describe the intended application, not already-completed CI.
Use [the official application page](https://signpath.org/apply); adapt this brief
to the fields shown there rather than treating it as a fixed form schema.

## Evidence and remaining conditions

| Condition | Current evidence / required action |
| --- | --- |
| Public OSS project | Public repository and MIT root license confirmed. Check the Windows-linked dependency license closure and installer/toolchain exceptions before asserting full license compliance. OSI-approved alternative license choices are not commercial dual-licensing. |
| Maintained and released | Public source and v0.1.10 release exist. Do not imply a large user base or longstanding reputation; project reputation remains subject to Foundation review. |
| Documented function | README describes supported camera, limitations, exports and installation. |
| No proprietary bundled SDK | Official SDK integration is deferred. Disclose the public ScopeTek-family protocol implementation and existing provenance notices; do not describe the backend as manufacturer-certified. |
| Repository/source control | Owner account is identified. Human must confirm authority, team roles and MFA on both services. |
| Policy and privacy | Draft policy is linked from README. Provider attribution is conditional until acceptance/use. Signed release pages will need the policy link. |
| Verified automated build | **Not ready:** no `.github/workflows` pipeline exists at preparation time. Current releases were built outside GitHub Actions. Existing binary hashes are evidence of byte identity, not provider-verified CI origin. |
| PE metadata | GUI and Setup have product/version resources. **Not ready:** helper has no dedicated Windows resource build step; generated uninstaller metadata also needs verification. Both must satisfy SignPath product/version restrictions. |
| Uninstaller signing | **Not ready:** current NSIS tooling has no qualified uninstaller-signing hook. Confirm the supported multi-stage process before claiming all executable types can be signed. |
| Installation behavior | v0.1.10 install/update/uninstall and unrelated-file preservation were checked. See `validation-0.1.10.json`; these are not new physical capture results. |
| Application approval | Not submitted. The human maintainer must review provider terms and Foundation publisher identity before enrollment. |

## Questions to resolve with SignPath

1. Confirm eligibility of this newly published research-camera project; disclose
   its actual release history without inventing popularity or adoption.
2. Confirm whether one human maintainer may hold the proposed committer,
   reviewer and approver roles, and what repository protections are required.
3. Confirm acceptable build-origin verification for Rust GNU Windows builds and
   the unmodified NSIS stubs/System plugin and toolchain runtime components.
4. Confirm a supported sequence for signing nested executable files, the NSIS
   generated uninstaller, and final Setup with manual release approval.

## Proposed CI integration, after prerequisites are settled

1. Add a manually triggered unsigned build that checks out a public commit and
   uses reviewed, pinned actions and toolchain versions. Use read-only repository
   permissions, no physical camera, no private `.local` files and no release write
   token in the build job. Keep signing unavailable to pull requests/forks.
2. Build both Windows GNU executables with the public commands in README and
   `Cargo.lock` enforced. Qualify resource-compiler discovery on the chosen runner;
   current `build.rs` explicitly invokes `x86_64-w64-mingw32-windres`.
3. Verify source identity without silently rewriting committed inputs; run scoped
   tests, package notices and record source commit, compiler versions, hashes and
   the CI artifact/run identity. Pin and verify the NSIS download as well.
4. After enrollment, configure the exact organization/project/policy/artifact
   identifiers assigned by SignPath. Install its GitHub integration and credentials
   only with the maintainer's explicit authorization. Do not check tokens into git.
5. Submit artifacts using the provider-supported provenance integration, require
   human approval, and verify Authenticode signatures, identity, timestamps and
   consistent product/version resources on returned files.
6. Package/sign in the provider-approved nested order, then generate final
   manifests/checksums. The current packager requires a clean committed tree and
   hashes unsigned build outputs; qualify a signed-input packaging path first.
7. Test a new signed candidate's install/update/uninstall and normal launch, then
   publish a new version. Never replace v0.1.10 assets with changed signed bytes.

This design deliberately does not install a dormant signing workflow with fake
credentials or an untested artifact configuration. It leaves the application,
current binaries, historical hashes and Windows security settings unchanged.

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
documentation-only changes.
