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
