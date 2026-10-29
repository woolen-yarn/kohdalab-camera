# Third-party notices

The optional Windows Setup uses unmodified NSIS 3.12 bootstrap/uninstaller code
and its zlib compression module (zlib/libpng license). Upstream notices are in
`third-party-licenses/nsis-3.12/COPYING`; that file also documents other NSIS
compression modules not selected by this installer. The NSIS compiler and its
archive are development tools and are not bundled in Setup.

The local `capability-platform` crate is part of this independently rebuilt
application, licensed under MIT (see `crates/capability-platform/LICENSE`). Its source and manifest
must accompany source distributions. It is not a registry-vendored dependency
and has no dependency on retained evidence directories. Its registry dependency
is the checksum-pinned `windows-sys` 0.61.2 and `windows-link` 0.2.1 already listed
below. Qualification-only executable source is included, but its executable is
not a camera application or a hardware-qualified distribution.

Compiler/runtime additions: GCC runtime exception and GPLv3 texts are in gcc-runtime (public gcc-mirror/gcc COPYING.RUNTIME and COPYING3); MinGW-w64/runtime contains the upstream combined runtime contributor notices, in addition to root COPYING/AUTHORS. Rust standard-library COPYRIGHT and MIT/Apache texts are retained separately. These compiler/runtime texts are notices, not application code adopted under GPL.

Engineering candidate v002 repairs omitted upstream license texts using exact public VCS revisions, with per-file public URLs and SHA-256 in the affected directories' RECOVERY.tsv. Windows dependency expressions were checked for substantive license clauses, including OR/AND obligations and bundled fonts. Khronos registry attribution is retained in khronos_api-3.1.0/REGISTRY-NOTICES.txt. This check is not proof of every nested attribution. The Apple-only dispatch 0.2.0 package declares MIT but supplies no license text in its public archive or upstream tree; that unresolved upstream notice defect prevents claiming complete macOS distribution coverage. No macOS executable or dependency source is shipped in this Windows candidate. Full metadata rows below include dependencies for other platforms, whether or not linked on Windows.

Resolved public crates, including target-specific dependencies. Declared license expressions are from Cargo metadata. Original license/notice files follow in third-party-licenses. Cargo.lock contains exact registry checksums.

## 日本語の説明 / Japanese explanation

optional Windows Setup は変更していない NSIS 3.12 の bootstrap/uninstaller code と zlib compression module（zlib/libpng license）を使います。upstream notice は `third-party-licenses/nsis-3.12/COPYING` にあり、この installer が選択しない他の NSIS compression module も記載しています。NSIS compiler と archive は開発 tool であり Setup には同梱しません。

local `capability-platform` crate は独自に再構築した application の一部で MIT license です（`crates/capability-platform/LICENSE`）。source distribution にはその source/manifest を同梱しなければなりません。registry-vendored dependency ではなく retained evidence directory への dependency もありません。registry dependency は下表にも記載する checksum-pinned `windows-sys` 0.61.2 と `windows-link` 0.2.1 です。qualification-only executable source は含みますが、その executable は camera application でも hardware-qualified distribution でもありません。

compiler/runtime addition として、GCC runtime exception/GPLv3 text は gcc-runtime（public gcc-mirror/gcc の COPYING.RUNTIME と COPYING3）に、MinGW-w64/runtime は root COPYING/AUTHORS に加えて upstream combined runtime contributor notice を含みます。Rust standard-library COPYRIGHT/MIT/Apache text も別に保持します。これら compiler/runtime text は notice であり、GPL に基づいて採用した application code ではありません。

engineering candidate v002 は、影響する directory の `RECOVERY.tsv` に各 file の public URL/SHA-256 とともに exact public VCS revision で不足 upstream license text を修復したことを記録します。Windows dependency expression は OR/AND obligation と bundled font を含む substantive license clause を確認し、Khronos registry attribution は `khronos_api-3.1.0/REGISTRY-NOTICES.txt` に保持します。この確認は全 nested attribution の証明ではありません。Apple-only dispatch 0.2.0 package は MIT を宣言しますが public archive/upstream tree に license text がないため、未解決の upstream notice defect により macOS distribution coverage が完全とは主張できません。この Windows candidate は macOS executable/dependency source を配布しません。下の full metadata row は Windows で link しない場合も含め他 platform dependency を記載します。

target-specific dependency を含む解決済み public crate を以下に示します。license expression は Cargo metadata による宣言です。原文の license/notice file は `third-party-licenses` に続き、`Cargo.lock` は exact registry checksum を含みます。以下の表、upstream notice、license text は原文のまま保持します。

| Crate | Version | License | Source |
|---|---|---|---|
| ab_glyph | 0.2.32 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| ab_glyph_rasterizer | 0.1.10 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| ahash | 0.8.12 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| android-activity | 0.6.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| android-properties | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| arboard | 3.6.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| bitflags | 1.3.2 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| block2 | 0.5.1 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| block2 | 0.6.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| bytes | 1.12.1 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| calloop | 0.13.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| calloop | 0.14.5 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| calloop-wayland-source | 0.4.1 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| cc | 1.8.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| cfg_aliases | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| cgl | 0.3.2 | MIT / Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| clipboard-win | 5.4.1 | BSL-1.0 | registry+https://github.com/rust-lang/crates.io-index |
| combine | 4.6.8 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| core-foundation | 0.9.4 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| core-graphics | 0.23.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| core-graphics-types | 0.1.3 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| crunchy | 0.2.4 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib | registry+https://github.com/rust-lang/crates.io-index |
| dispatch | 0.2.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| dlib | 0.5.3 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| document-features | 0.2.12 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| downcast-rs | 1.2.1 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| dpi | 0.1.2 | Apache-2.0 AND MIT | registry+https://github.com/rust-lang/crates.io-index |
| ecolor | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| eframe | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| egui | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| egui-winit | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| egui_glow | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| emath | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| epaint | 0.31.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| epaint_default_fonts | 0.31.1 | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 | registry+https://github.com/rust-lang/crates.io-index |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| errno | 0.3.14 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| error-code | 3.4.0 | BSL-1.0 | registry+https://github.com/rust-lang/crates.io-index |
| fax | 0.2.7 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| find-msvc-tools | 0.1.14 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| foreign-types | 0.5.0 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| gethostname | 1.1.0 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| gl_generator | 0.14.0 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| glow | 0.16.0 | MIT OR Apache-2.0 OR Zlib | registry+https://github.com/rust-lang/crates.io-index |
| glutin | 0.32.3 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| glutin-winit | 0.5.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| glutin_egl_sys | 0.7.1 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| glutin_wgl_sys | 0.6.1 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| half | 2.7.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_collections | 2.3.0 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_locale_core | 2.3.0 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_normalizer | 2.3.0 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_properties | 2.3.0 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_properties_data | 2.3.0 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| icu_provider | 2.3.1 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| idna | 1.1.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| image | 0.25.10 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| io-kit-sys | 0.5.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| itoa | 1.0.18 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| jni | 0.22.4 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| jni-sys | 0.3.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| jobserver | 0.1.35 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| js-sys | 0.3.106 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| khronos_api | 3.1.0 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| libc | 0.2.190 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| libloading | 0.8.9 | ISC | registry+https://github.com/rust-lang/crates.io-index |
| libredox | 0.1.25 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| linux-raw-sys | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| litemap | 0.8.3 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| litrs | 1.0.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| log | 0.4.34 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| mach2 | 0.5.0 | BSD-2-Clause OR MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| memchr | 2.8.3 | Unlicense OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| memmap2 | 0.9.11 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| memoffset | 0.9.1 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| ndk | 0.9.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| ndk-context | 0.1.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| ndk-sys | 0.6.0+11769913 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| nohash-hasher | 0.2.0 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| nusb | 0.2.9 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc-sys | 0.3.5 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2 | 0.5.3 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2 | 0.6.5 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-app-kit | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-cloud-kit | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-contacts | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-core-data | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-core-image | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-core-location | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-encode | 4.1.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-foundation | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-foundation | 0.3.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-link-presentation | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-metal | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-quartz-core | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-symbols | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-ui-kit | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-uniform-type-identifiers | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| objc2-user-notifications | 0.2.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| orbclient | 0.3.55 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| owned_ttf_parser | 0.25.1 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| pin-project | 1.1.13 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| pin-project-internal | 1.1.13 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| plain | 0.2.3 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| png | 0.18.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| polling | 3.11.0 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| pollster | 0.4.0 | Apache-2.0/MIT | registry+https://github.com/rust-lang/crates.io-index |
| potential_utf | 0.1.6 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| profiling | 1.0.18 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| quick-error | 2.0.1 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| quick-xml | 0.41.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| quote | 1.0.47 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | registry+https://github.com/rust-lang/crates.io-index |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | registry+https://github.com/rust-lang/crates.io-index |
| redox_syscall | 0.4.1 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| redox_syscall | 0.5.18 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| redox_syscall | 0.9.4 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| rfd | 0.17.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| rustix | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| same-file | 1.0.6 | Unlicense/MIT | registry+https://github.com/rust-lang/crates.io-index |
| scoped-tls | 1.0.1 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| semver | 1.0.28 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| serde | 1.0.229 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| shlex | 2.0.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| simd-adler32 | 0.3.10 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| slab | 0.4.12 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| slotmap | 1.1.1 | Zlib | registry+https://github.com/rust-lang/crates.io-index |
| smallvec | 1.16.3 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| smithay-client-toolkit | 0.20.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| smithay-clipboard | 0.7.3 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| smol_str | 0.2.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| syn | 2.0.119 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| syn | 3.0.9 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| synstructure | 0.14.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| tiff | 0.11.3 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| tinystr | 0.8.4 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| tokio | 1.53.3 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| tracing | 0.1.44 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| tracing-core | 0.1.36 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| url | 2.5.8 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| version_check | 0.9.5 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| walkdir | 2.5.0 | Unlicense/MIT | registry+https://github.com/rust-lang/crates.io-index |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| wayland-backend | 0.3.17 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-client | 0.31.15 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-csd-frame | 0.3.0 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-cursor | 0.31.14 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-protocols | 0.32.13 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-protocols-experimental | 20250721.0.1 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-protocols-misc | 0.3.12 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-protocols-wlr | 0.3.12 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-scanner | 0.31.11 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| wayland-sys | 0.31.11 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| web-sys | 0.3.106 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| web-time | 1.1.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| webbrowser | 1.2.4 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| weezl | 0.1.12 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| winapi | 0.3.9 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| winapi-util | 0.1.11 | Unlicense OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_aarch64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_aarch64_msvc | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_i686_gnu | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_i686_gnullvm | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_i686_msvc | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_x86_64_gnu | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_x86_64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| winit | 0.30.13 | Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| winnow | 1.0.4 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| writeable | 0.6.4 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| x11rb | 0.13.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| x11rb-protocol | 0.13.2 | MIT OR Apache-2.0 | registry+https://github.com/rust-lang/crates.io-index |
| xcursor | 0.3.11 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| xkbcommon-dl | 0.4.2 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| xkeysym | 0.2.1 | MIT OR Apache-2.0 OR Zlib | registry+https://github.com/rust-lang/crates.io-index |
| xml-rs | 0.8.29 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| yoke | 0.8.3 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| yoke-derive | 0.8.4 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | registry+https://github.com/rust-lang/crates.io-index |
| zerofrom | 0.1.8 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| zerotrie | 0.2.5 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| zerovec | 0.11.8 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| zerovec-derive | 0.11.6 | Unicode-3.0 | registry+https://github.com/rust-lang/crates.io-index |
| zlib-rs | 0.6.8 | Zlib | registry+https://github.com/rust-lang/crates.io-index |
| zmij | 1.0.23 | MIT | registry+https://github.com/rust-lang/crates.io-index |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | registry+https://github.com/rust-lang/crates.io-index |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib | registry+https://github.com/rust-lang/crates.io-index |

Bundled default fonts retain their own licenses in epaint_default_fonts-0.31.1/fonts: Ubuntu Font License, SIL Open Font License, Hack license, and emoji icon MIT license. The Windows candidate also links MinGW-w64 runtime support; its ZPL and contributor notices are in mingw-w64/. Rust standard library uses MIT OR Apache-2.0; corresponding notices are in rust-standard-library/.
