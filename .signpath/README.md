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
