# KohdaLab Camera 0.1.0

The first official development baseline for a Rust laboratory-camera platform.
The current physical backend is limited to the tested ScopeTek-family USB
`0547:4d33` / Micron MT9T001 unit on Windows x64. Multi-vendor camera support is a
long-term goal, not a capability of this release.

The portable release contains the application, a separate acquisition helper,
MIT project source, and dependency license notices. Features include camera
selection, five control tabs, default brightness of 1.20×, per-pixel GRBG
demosaicing, image adjustments, processed PNG/TIFF and original RAW exports,
full-sensor capture with Live restoration, separate acquisition/preview FPS,
and host UTC frame-acceptance timestamps.

Pre-publication feature code was checked for physical Live acquisition, exports
at both resolutions, pixel agreement, and normal shutdown. The build with
publication names and diagnostic paths has different hashes and has not been
physically retested. Ordinary mouse interaction, color/field-of-view calibration,
and arbitrary window-size persistence remain incomplete. Long-duration testing
is deferred. This release does not represent full product qualification.

Repository documentation may evolve after the tag; the existing 0.1.0 download
archives retain their original contents and checksums.
