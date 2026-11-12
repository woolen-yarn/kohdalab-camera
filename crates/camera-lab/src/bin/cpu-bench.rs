//! Synthetic CPU/export benchmark. Never enumerates or opens USB.
#![forbid(unsafe_code)]
use camera_lab::{Frame, Metadata};
use std::{hint::black_box, sync::Arc, time::Instant};
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--verify-export-bundle") {
        let path = std::env::args().nth(2).expect("PNG bundle path");
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(format!("{path}.json")).unwrap()).unwrap();
        let frame = Frame {
            raw: Arc::new(std::fs::read(format!("{path}.raw")).unwrap()),
            metadata: serde_json::from_value(meta["frame"].clone()).unwrap(),
        };
        let recipe: camera_lab::display_recipe::Recipe =
            serde_json::from_value(meta["display_recipe"].clone()).unwrap();
        let (w, h, pixels) = recipe.render(&frame, None).unwrap();
        let image = image::open(&path).unwrap().to_rgb8();
        assert_eq!(image.dimensions(), (w, h));
        assert_eq!(image.as_raw(), &pixels);
        println!(
            "Saved RAW/recipe replay matches all {} RGB pixels ({w}x{h})",
            w * h
        );
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--recipe-v2") {
        for (w, h) in [(1024, 768), (2048, 1536)] {
            let f = Frame {
                raw: Arc::new((0..w * h).map(|i| (i % 251) as u8).collect()),
                metadata: Metadata {
                    acquisition: None,
                    source: "Synthetic recipe benchmark; no camera".into(),
                    simulated: true,
                    sequence: 1,
                    measurement: None,
                    unix_millis: 0,
                    width: w,
                    height: h,
                    format: "BayerGRBG8".into(),
                },
            };
            let recipe = camera_lab::display_recipe::Recipe::default();
            let compiled = recipe.compile().unwrap();
            let start = Instant::now();
            for _ in 0..10_000 {
                black_box(recipe.compile().unwrap());
            }
            println!(
                "{w}x{h} recipe/v2 compile-only: {:.3} us/recipe (10,000 iterations)",
                start.elapsed().as_secs_f64() * 100.
            );
            for _ in 0..3 {
                let (pw, ph, rgb) = recipe.render(&f, Some(1024)).unwrap();
                black_box(eframe::egui::ColorImage::from_rgb(
                    [pw as usize, ph as usize],
                    &rgb,
                ));
                black_box(recipe.render_color_image(&f, Some(1024)).unwrap());
                black_box(compiled.render_color_image(&f, Some(1024)).unwrap());
            }
            for sample in 0..5 {
                let start = Instant::now();
                for _ in 0..10 {
                    let (pw, ph, rgb) = recipe.render(&f, Some(1024)).unwrap();
                    black_box(eframe::egui::ColorImage::from_rgb(
                        [pw as usize, ph as usize],
                        &rgb,
                    ));
                }
                let export_path = start.elapsed();
                let start = Instant::now();
                for _ in 0..10 {
                    black_box(recipe.render_color_image(&f, Some(1024)).unwrap());
                }
                let direct = start.elapsed();
                let start = Instant::now();
                for _ in 0..10 {
                    black_box(compiled.render_color_image(&f, Some(1024)).unwrap());
                }
                let cached = start.elapsed();
                println!(
                    "{w}x{h} recipe/v2 preview sample {sample}: render+ColorImage={:.3} ms/frame direct-ColorImage={:.3} ms/frame compiled-ColorImage={:.3} ms/frame",
                    export_path.as_secs_f64() * 100.,
                    direct.as_secs_f64() * 100.,
                    cached.as_secs_f64() * 100.
                );
            }
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--recipe-v2-orientations") {
        recipe_v2_orientations();
        return;
    }
    if let Some(mode) = std::env::args().nth(1) {
        assert!(matches!(
            mode.as_str(),
            "--preview-memory-baseline" | "--preview-memory-bounded"
        ));
        let f = Frame {
            raw: Arc::new(vec![127; 2048 * 1536]),
            metadata: Metadata {
                acquisition: None,
                source: "Synthetic preview memory".into(),
                simulated: true,
                sequence: 1,
                measurement: None,
                unix_millis: 0,
                width: 2048,
                height: 1536,
                format: "BayerGRBG8".into(),
            },
        };
        let (w, h, pixels) = if mode == "--preview-memory-baseline" {
            (2048, 1536, f.display_rgb().into_owned())
        } else {
            f.preview_rgb(1024).unwrap()
        };
        let image = eframe::egui::ColorImage::from_rgb([w as usize, h as usize], &pixels);
        println!(
            "{mode}: raw={} rgb={} color32={} bytes retained; no GPU allocation",
            f.raw.len(),
            pixels.len(),
            image.pixels.len() * 4
        );
        // Allow an external OS sampler to observe the same retained pipeline.
        std::thread::sleep(std::time::Duration::from_secs(2));
        black_box((&f, &pixels, &image));
        return;
    }
    for (w, h) in [(640, 480), (2048, 1536)] {
        let f = Frame {
            raw: Arc::new((0..w * h).map(|i| (i % 251) as u8).collect()),
            metadata: Metadata {
                acquisition: None,
                source: "Synthetic CPU benchmark; no camera".into(),
                simulated: true,
                sequence: 1,
                measurement: None,
                unix_millis: 0,
                width: w,
                height: h,
                format: "BayerGRBG8".into(),
            },
        };
        let reference = || {
            let mut out = Vec::with_capacity((w * h * 3) as usize);
            for y in 0..h {
                for x in 0..w {
                    out.extend_from_slice(&f.rgb(x, y).unwrap());
                }
            }
            out
        };
        assert_eq!(reference(), f.display_rgb().as_ref());
        for (name, old) in [("baseline", true), ("cell_once", false)] {
            let start = Instant::now();
            for _ in 0..20 {
                if old {
                    black_box(reference());
                } else {
                    black_box(f.display_rgb());
                }
            }
            println!(
                "{w}x{h} {name}: {:.3} ms/conversion (20 iterations)",
                start.elapsed().as_secs_f64() * 1000. / 20.
            );
        }
        let root =
            std::env::temp_dir().join(format!("camera-cpu-bench-{}-{w}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for ext in ["png", "tiff"] {
            let start = Instant::now();
            camera_lab::export(&f, &root.join(format!("synthetic.{ext}"))).unwrap();
            println!(
                "{w}x{h} {ext} bundle export: {:.3} ms",
                start.elapsed().as_secs_f64() * 1000.
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// SHA-256 of the output pixels as RGBA bytes in display order, prefixed by
/// the little-endian `u32` width and height so dimension changes are visible.
fn color_image_sha256(image: &eframe::egui::ColorImage) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update((image.size[0] as u32).to_le_bytes());
    hasher.update((image.size[1] as u32).to_le_bytes());
    for pixel in &image.pixels {
        hasher.update(pixel.to_array());
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Untimed verification for all 16 `quarter_turns × mirror × flip` flag
/// combinations (each with and without monochrome) on a small non-square
/// Bayer frame, at stride 1 and at bounds whose stride leaves partial
/// terminal boxes on both axes. The combined hash compares old and new builds;
/// the independent pixel reference is covered by the renderer's unit tests.
fn recipe_v2_flag_verification() {
    use camera_lab::display_recipe::Recipe;
    use sha2::{Digest, Sha256};
    let (w, h) = (38_u32, 26_u32);
    let mut state = 0x9E37_79B9_u32;
    let f = Frame {
        raw: Arc::new(
            (0..w * h)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    (state >> 24) as u8
                })
                .collect(),
        ),
        metadata: Metadata {
            acquisition: None,
            source: "Synthetic recipe flag verification; no camera".into(),
            simulated: true,
            sequence: 1,
            measurement: None,
            unix_millis: 0,
            width: w,
            height: h,
            format: "BayerGRBG8".into(),
        },
    };
    // None and Some(38) give stride 1; the others give strides 4, 6 and 8,
    // none of which divides 38 or 26.
    let bounds = [None, Some(38), Some(11), Some(7), Some(5)];
    for bound in bounds.into_iter().flatten() {
        let stride = w.max(h).div_ceil(bound);
        assert!(
            stride == 1 || (!w.is_multiple_of(stride) && !h.is_multiple_of(stride)),
            "bound={bound} stride={stride} has no partial box"
        );
    }
    let mut hasher = Sha256::new();
    let mut cases = 0_u32;
    for quarter_turns in 0..4_u8 {
        for mirror in [false, true] {
            for flip in [false, true] {
                for monochrome in [false, true] {
                    let recipe = Recipe {
                        ev: 0.37,
                        gamma: 1.73,
                        wb: [0.63, 1.24, 1.81],
                        quarter_turns,
                        mirror,
                        flip,
                        monochrome,
                        ..Recipe::neutral()
                    };
                    let compiled = recipe.compile().unwrap();
                    for bound in bounds {
                        let image = compiled.render_color_image(&f, bound).unwrap();
                        hasher.update([quarter_turns, mirror as u8, flip as u8, monochrome as u8]);
                        hasher.update(bound.unwrap_or(0).to_le_bytes());
                        hasher.update(color_image_sha256(&image).as_bytes());
                        cases += 1;
                    }
                }
            }
        }
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    println!(
        "{w}x{h} recipe/v2 flag verification: {cases} cases (16 flags x mono x bounds {bounds:?}); sha256={digest}"
    );
}

/// Compiled v2 preview cost and deterministic output hash for all eight
/// display orientations (four clockwise turns, each with and without mirror).
/// Uses only the public API so the same source can be built against both the
/// previous and the current renderer. Hashing happens outside timed regions.
fn recipe_v2_orientations() {
    const SAMPLES: usize = 5;
    const FRAMES: u32 = 10;
    // Untimed correctness gate before any measurement.
    recipe_v2_flag_verification();
    for (w, h) in [(1024_u32, 768_u32), (2048, 1536)] {
        let f = Frame {
            raw: Arc::new((0..w * h).map(|i| (i % 251) as u8).collect()),
            metadata: Metadata {
                acquisition: None,
                source: "Synthetic recipe orientation benchmark; no camera".into(),
                simulated: true,
                sequence: 1,
                measurement: None,
                unix_millis: 0,
                width: w,
                height: h,
                format: "BayerGRBG8".into(),
            },
        };
        for quarter_turns in 0..4 {
            for mirror in [false, true] {
                let recipe = camera_lab::display_recipe::Recipe {
                    quarter_turns,
                    mirror,
                    flip: false,
                    ..camera_lab::display_recipe::Recipe::default()
                };
                let compiled = recipe.compile().unwrap();
                // Warm-up; the first output is also the hash reference.
                let reference = compiled.render_color_image(&f, Some(1024)).unwrap();
                for _ in 0..2 {
                    black_box(compiled.render_color_image(&f, Some(1024)).unwrap());
                }
                let mut ms = Vec::with_capacity(SAMPLES);
                for _ in 0..SAMPLES {
                    let start = Instant::now();
                    for _ in 0..FRAMES {
                        black_box(compiled.render_color_image(&f, Some(1024)).unwrap());
                    }
                    ms.push(start.elapsed().as_secs_f64() * 1000. / f64::from(FRAMES));
                }
                // Untimed: hash the warm-up output and confirm a fresh render
                // after timing is byte-identical (determinism).
                let hash = color_image_sha256(&reference);
                let after = compiled.render_color_image(&f, Some(1024)).unwrap();
                assert_eq!(after, reference, "non-deterministic render");
                let mut sorted = ms.clone();
                sorted.sort_by(f64::total_cmp);
                println!(
                    "{w}x{h} recipe/v2 orientation turn={quarter_turns} mirror={mirror} out={}x{} compiled-ColorImage median={:.3} min={:.3} ms/frame samples=[{}] ({SAMPLES}x{FRAMES} frames) sha256={hash}",
                    reference.size[0],
                    reference.size[1],
                    sorted[SAMPLES / 2],
                    sorted[0],
                    ms.iter()
                        .map(|v| format!("{v:.3}"))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
    }
}
