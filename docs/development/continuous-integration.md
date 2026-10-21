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
- The NSIS 3.12 archive is hash-verified before extraction. Setup is per-user and
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
