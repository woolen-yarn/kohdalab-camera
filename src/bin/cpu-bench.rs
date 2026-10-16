//! Synthetic CPU/export benchmark. Never enumerates or opens USB.
#![forbid(unsafe_code)]
use camera_lab::{Frame, Metadata};
use std::{hint::black_box, sync::Arc, time::Instant};
fn main() {
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
                measurement: None, unix_millis: 0,
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
                measurement: None, unix_millis: 0,
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
