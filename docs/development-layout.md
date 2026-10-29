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
`%ProgramFiles%\Kohdalab\KohdaLab Camera`, with one desktop/Start Menu launch path.
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
