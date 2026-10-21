# Provenance and redistribution scope

The application started from an independently written Rust implementation in an
isolated workspace. Legacy application source, manufacturer binaries,
disassembly output, private USB traffic captures, and manufacturer artwork were
not incorporated into the source or release. The icon and Windows manifest are
project assets. The current camera icon artwork was generated for this project
with OpenAI image generation; its original PNG, derived Windows ICO, and
conversion script are included under the project license.

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

---

# 来歴と再配布範囲 / Provenance and redistribution scope（日本語）

アプリケーションは isolated workspace で独自に記述した Rust 実装から開始しました。旧 application source、manufacturer binary、disassembly output、private USB traffic capture、manufacturer artwork は source/release に取り込んでいません。icon と Windows manifest は project asset です。現行 camera icon artwork は KohdaLab Camera 用に OpenAI image generation で生成し、original PNG、派生 Windows ICO、conversion script を project license で含めています。

sensor dimension、public register specification、timing は public Micron MT9T001 datasheet から調査しました。USB communication format を調べる研究入力として public ScopeTek implementation（JohnDMcMaster/uvscopetek）と public Linux camera implementation を参照しました。公開されていることは manufacturer certification や redistribution permission と同義ではありません。GPL research source、array、initialization-sequence code、datasheet contents はこの repository/release に同梱していません。public implementation が reverse-engineering origin を持たないと断言することもできません。本記述は入力と過程の文書であり、legal clean-room certification ではありません。

- [Public ScopeTek research reference](https://github.com/JohnDMcMaster/uvscopetek)
- [Public Micron MT9T001 datasheet reference](https://datasheet.octopart.com/MT9T001P12STC-Micron-datasheet-136597.pdf)
- Rust registry dependencies: exact checksum は `Cargo.lock` に記録します。`THIRD_PARTY_NOTICES.md` と `third-party-licenses/` を保持してください。
- local `platform` crate は project-authored MIT code です。

配布 executable は Windows x64 向けです。Apple-only `dispatch` dependency の missing upstream license text を含む macOS distribution compliance はこの release の qualification scope 外です。旧 application version を削除しても references と third-party license notices は必要です。将来の camera backend は明確な provenance と redistribution basis を持つ documented interface を使用しなければなりません。manufacturer binary reverse engineering、private traffic capture、proprietary initialization data は導入禁止です。現 backend の public-research provenance は開示し続け、platform roadmap を公式 vendor API の事後的根拠にしてはいけません。
