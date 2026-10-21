# Development and installation layout

The public repository is the source of truth. Build outputs, local captures and
historical validation are not source dependencies and must not be uploaded in
source or signing artifacts.

## Mac development checkout

- Repository root: current tracked source and public documentation only.
- `.local/toolchain/` and `.local/target/`: reusable local tools and build cache.
- `.local/evidence/`: retained private validation data.
- `.local/release-<current-version>/`: current downloaded or packaged distribution.
- `.local/archive/releases/`, `.local/archive/candidates/`: older engineering outputs.
- `.local/logs/historical/`: previous build/test logs.

`.local/` is ignored in git. Organization moves preserve historical bytes and
results; they do not make old outputs current or authorize reusing a consumed
physical-session plan. Cache and tool locations stay stable for existing builds.

## Windows workstation

The supported installed application lives under
`%LOCALAPPDATA%\Programs\KohdaLab Camera`, with one desktop/Start Menu launch path.
The workspace holds `source/` for the current public source snapshot, `windows/`
for the current portable payload, and `release/` for the current downloaded
distribution. These are distinct from the installed application's user data.

Within the project workspace, `archive/legacy-validation/` holds older numbered
validation folders, and `archive/project-history/` holds previous update staging
and obsolete validation shortcuts. Existing evidence, captures, recovery archives
and interlocks are preserved. Never move an active session or clear a protective
lock to make organization or an update succeed.

Source synchronization must record the commit and verify archive/remote hashes.
An installed update must be checked against its own manifest and executable
version, rather than trusting a shortcut name. Do not replace historic release
assets or write new results into old validation records.

---

# 開発・インストール構造 / Development and installation layout（日本語）

public repository が source of truth です。build output、local capture、historical validation は source dependency ではなく、source/signing artifact に upload してはいけません。

## Mac development checkout

- repository root: 現在追跡している source と public documentation のみ。
- `.local/toolchain/` と `.local/target/`: 再利用する local tool と build cache。
- `.local/evidence/`: 保持する private validation data。
- `.local/release-<current-version>/`: 現在 download/package した distribution。
- `.local/archive/releases/`、`.local/archive/candidates/`: 古い engineering output。
- `.local/logs/historical/`: 過去の build/test log。

`.local/` は git ignore です。organization move は historical byte/result を保持しますが、古い output を current にしたり、消費済み physical-session plan の再使用を許可したりしません。既存 build のため cache/tool location は安定させます。

## Windows workstation

対応する installed application は `%LOCALAPPDATA%\Programs\KohdaLab Camera` にあり desktop/Start Menu の launch path を一つ持ちます。workspace の `source/` は current public source snapshot、`windows/` は current portable payload、`release/` は current downloaded distribution で、installed application の user data とは別です。project workspace の `archive/legacy-validation/` は古い番号付き validation folder、`archive/project-history/` は previous update staging/obsolete validation shortcut を置きます。evidence、capture、recovery archive、interlock は保持し、organization/update のため active session を移動したり protective lock を消したりしません。

source synchronization は commit を記録し archive/remote hash を確認します。installed update は shortcut name を信頼せず自身の manifest/executable version と照合します。historic release asset を置換したり、新しい result を古い validation record に書き込んだりしません。
