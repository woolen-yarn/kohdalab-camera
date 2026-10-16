# Provenance and redistribution scope

The application started from an independently written Rust implementation in an
isolated workspace. Legacy application source, manufacturer binaries,
disassembly output, private USB traffic captures, and manufacturer artwork were
not incorporated into the source or release. The icon and Windows manifest are
project-authored assets.

Sensor dimensions, public register specifications, and timing were studied from
the public Micron MT9T001 datasheet. Public ScopeTek implementations
(JohnDMcMaster/uvscopetek) and public Linux camera implementations were consulted
as research inputs when investigating functional USB communication formats.
Public availability is not equivalent to manufacturer certification or
redistribution permission. GPL research source, arrays, initialization-sequence
code, and datasheet contents are not bundled in this repository or release.
A public implementation cannot categorically be described as free of
reverse-engineering origins. This statement documents inputs and process; it is
not a legal clean-room certification.

- [Public ScopeTek research reference](https://github.com/JohnDMcMaster/uvscopetek)
- [Public Micron MT9T001 datasheet reference](https://datasheet.octopart.com/MT9T001P12STC-Micron-datasheet-136597.pdf)
- Rust registry dependencies: exact checksums are recorded in `Cargo.lock`.
  Preserve `THIRD_PARTY_NOTICES.md` and `third-party-licenses/`.
- The local `platform` crate is project-authored MIT code.

The distributed executable targets Windows x64. macOS distribution compliance,
including the missing upstream license text for the Apple-only `dispatch`
dependency, is outside this release's qualification scope. References and
third-party license notices remain necessary even when old application versions
are removed.

Future camera backends must use documented interfaces with a clear provenance
and redistribution basis. Manufacturer binary reverse engineering, private
traffic captures, and proprietary initialization data must not be introduced.
The current backend's public-research provenance above must remain disclosed;
the platform roadmap does not retroactively establish an official vendor API.
