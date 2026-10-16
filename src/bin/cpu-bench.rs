//! Synthetic CPU/export benchmark. Never enumerates or opens USB.
#![forbid(unsafe_code)]
use camera_lab::{Frame, Metadata};
use std::{hint::black_box, sync::Arc, time::Instant};
fn main() {
    if std::env::args().nth(1).as_deref()==Some("--verify-export-bundle") {
        let path=std::env::args().nth(2).expect("PNG bundle path");
        let meta:serde_json::Value=serde_json::from_slice(&std::fs::read(format!("{path}.json")).unwrap()).unwrap();
        let frame=Frame {raw:Arc::new(std::fs::read(format!("{path}.raw")).unwrap()),metadata:serde_json::from_value(meta["frame"].clone()).unwrap()};
        let recipe:camera_lab::display_recipe::Recipe=serde_json::from_value(meta["display_recipe"].clone()).unwrap();
        let (w,h,pixels)=recipe.render(&frame,None).unwrap();
        let image=image::open(&path).unwrap().to_rgb8();
        assert_eq!(image.dimensions(),(w,h));assert_eq!(image.as_raw(),&pixels);
        println!("Saved RAW/recipe replay matches all {} RGB pixels ({w}x{h})",w*h);
        return;
    }
    if std::env::args().nth(1).as_deref()==Some("--recipe-v2") {
        for (w,h) in [(1024,768),(2048,1536)] {
            let f=Frame {raw:Arc::new((0..w*h).map(|i|(i%251) as u8).collect()),metadata:Metadata {acquisition:None,source:"Synthetic recipe benchmark; no camera".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:w,height:h,format:"BayerGRBG8".into()}};
            let recipe=camera_lab::display_recipe::Recipe::default();
            let compiled=recipe.compile().unwrap();
            let start=Instant::now();
            for _ in 0..10_000 { black_box(recipe.compile().unwrap()); }
            println!("{w}x{h} recipe/v2 compile-only: {:.3} us/recipe (10,000 iterations)",start.elapsed().as_secs_f64()*100.);
            for _ in 0..3 {
                let (pw,ph,rgb)=recipe.render(&f,Some(1024)).unwrap();
                black_box(eframe::egui::ColorImage::from_rgb([pw as usize,ph as usize],&rgb));
                black_box(recipe.render_color_image(&f,Some(1024)).unwrap());
                black_box(compiled.render_color_image(&f,Some(1024)).unwrap());
            }
            for sample in 0..5 {
                let start=Instant::now();
                for _ in 0..10 {
                    let (pw,ph,rgb)=recipe.render(&f,Some(1024)).unwrap();
                    black_box(eframe::egui::ColorImage::from_rgb([pw as usize,ph as usize],&rgb));
                }
                let export_path=start.elapsed();
                let start=Instant::now();
                for _ in 0..10 {black_box(recipe.render_color_image(&f,Some(1024)).unwrap());}
                let direct=start.elapsed();
                let start=Instant::now();
                for _ in 0..10 {black_box(compiled.render_color_image(&f,Some(1024)).unwrap());}
                let cached=start.elapsed();
                println!("{w}x{h} recipe/v2 preview sample {sample}: render+ColorImage={:.3} ms/frame direct-ColorImage={:.3} ms/frame compiled-ColorImage={:.3} ms/frame",export_path.as_secs_f64()*100.,direct.as_secs_f64()*100.,cached.as_secs_f64()*100.);
            }
        }
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
