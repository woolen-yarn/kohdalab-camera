#![forbid(unsafe_code)]
pub mod acceptance_summary;
pub mod acquisition_recipe;
#[cfg(test)]
mod acquisition_recipe_tests;
pub mod acquisition_telemetry;
pub mod tail_provenance;
pub mod continuous;
pub mod continuous_runtime;
pub mod continuous_session;
pub mod controls;
pub mod physical;
pub mod physical_preview;
pub mod preview_pipeline;
pub mod session;
pub mod sensor_contract;
pub mod startup;
pub mod driver_policy;
pub mod device_catalog;
pub mod product;
pub mod measurement;
pub mod image_export;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    /// Absent means legacy/fixture metadata, never inferred to be full-field skip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<sensor_contract::Acquisition>,
    pub source: String,
    pub simulated: bool,
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<measurement::Observation>,
    pub unix_millis: u128,
    pub width: u32,
    pub height: u32,
    pub format: String,
}
#[derive(Clone)]
pub struct Frame {
    pub raw: Arc<Vec<u8>>,
    pub metadata: Metadata,
}
/// Storage dimensions proved valid once for a rendering pass.
///
/// This is crate-private so callers cannot bypass `Frame`'s public validation
/// contract.  It intentionally performs the same constant-time checks as the
/// public `Frame::rgb` entry point.
pub(crate) struct RgbView<'a> {
    raw: &'a [u8],
    width: usize,
    height: usize,
    channels: usize,
}
impl RgbView<'_> {
    /// Rendering-only reconstruction after frame validation and coordinate
    /// mapping have established the layout and bounds. Safe slice indexing is
    /// still used; this never uses unchecked memory access.
    #[inline]
    pub(crate) fn render_rgb(&self, x: usize, y: usize) -> [u8; 3] {
        debug_assert!(x < self.width && y < self.height);
        if self.channels == 3 {
            let i = (y * self.width + x) * 3;
            return [self.raw[i], self.raw[i + 1], self.raw[i + 2]];
        }
        let l = if x == 0 { 1 } else { x - 1 };
        let r = if x + 1 == self.width { self.width - 2 } else { x + 1 };
        let u = if y == 0 { 1 } else { y - 1 };
        let d = if y + 1 == self.height { self.height - 2 } else { y + 1 };
        let at = |a: usize, b: usize| u16::from(self.raw[b * self.width + a]);
        let c = at(x, y) as u8;
        match (y % 2, x % 2) {
            // Green sites need only their horizontal and vertical neighbours.
            (0, 0) => [
                ((at(l, y) + at(r, y) + 1) / 2) as u8,
                c,
                ((at(x, u) + at(x, d) + 1) / 2) as u8,
            ],
            // Red and blue sites need the axial green and diagonal opposite
            // colour neighbours, but never the horizontal/vertical pair.
            (0, 1) => [
                c,
                ((at(l, y) + at(r, y) + at(x, u) + at(x, d) + 2) / 4) as u8,
                ((at(l, u) + at(r, u) + at(l, d) + at(r, d) + 2) / 4) as u8,
            ],
            (1, 0) => [
                ((at(l, u) + at(r, u) + at(l, d) + at(r, d) + 2) / 4) as u8,
                ((at(l, y) + at(r, y) + at(x, u) + at(x, d) + 2) / 4) as u8,
                c,
            ],
            _ => [
                ((at(x, u) + at(x, d) + 1) / 2) as u8,
                c,
                ((at(l, y) + at(r, y) + 1) / 2) as u8,
            ],
        }
    }
}
impl Frame {
    pub fn validate(&self) -> Result<(), String> {
        let m = &self.metadata;
        if let Some(a) = &m.acquisition {
            a.settings()?;
            if m.format != "BayerGRBG8" || m.sequence != a.sequence
                || [m.width,m.height] != a.geometry.output.map(u32::from)
                || self.raw.len() != a.geometry.payload_bytes as usize {
                return Err("Frame disagrees with acquisition contract".into());
            }
        }
        if m.width == 0
            || m.height == 0
            || m.width > 8192
            || m.height > 8192
            || !matches!(m.format.as_str(), "RGB8" | "BayerGRBG8")
            || (m.format == "BayerGRBG8"
                && (!m.width.is_multiple_of(2) || !m.height.is_multiple_of(2)))
            || self.raw.len()
                != m.width as usize * m.height as usize * if m.format == "RGB8" { 3 } else { 1 }
        {
            Err("Invalid frame".into())
        } else {
            Ok(())
        }
    }
    // Constant-time storage checks only: no acquisition-contract work or allocation.
    fn rgb_layout(&self)->Option<(usize,usize,usize)> {
        let m=&self.metadata;
        let channels=match m.format.as_str() {"RGB8"=>3,"BayerGRBG8"=>1,_=>return None};
        if m.width==0 || m.height==0 || m.width>8192 || m.height>8192
            || (channels==1 && (m.width%2!=0 || m.height%2!=0)) {return None;}
        let (w,h)=(usize::try_from(m.width).ok()?,usize::try_from(m.height).ok()?);
        let len=w.checked_mul(h)?.checked_mul(channels)?;
        if len!=self.raw.len() {return None;} Some((w,h,channels))
    }
    pub(crate) fn rgb_view(&self) -> Option<RgbView<'_>> {
        let (width, height, channels) = self.rgb_layout()?;
        Some(RgbView {
            raw: self.raw.as_slice(),
            width,
            height,
            channels,
        })
    }
    /// Independent bilinear GRBG; reflect101, u16 means rounded half up.
    /// Malformed layouts and out-of-range coordinates return None.
    pub fn rgb(&self, x:u32,y:u32)->Option<[u8;3]> {
        let (w,h,channels)=self.rgb_layout()?;
        let (x,y)=(usize::try_from(x).ok()?,usize::try_from(y).ok()?);
        if x>=w || y>=h {return None;}
        // Layout bounds prove all following index arithmetic fits usize.
        if channels==3 {let i=(y*w+x)*3;return self.raw.get(i..i+3)?.try_into().ok();}
        let l=if x==0 {1}else{x-1};let r=if x+1==w {w-2}else{x+1};
        let u=if y==0 {1}else{y-1};let d=if y+1==h {h-2}else{y+1};
        let at=|a:usize,b:usize|self.raw.get(b*w+a).copied().map(u16::from);
        let c=at(x,y)? as u8;
        let horizontal=((at(l,y)?+at(r,y)?+1)/2) as u8;
        let vertical=((at(x,u)?+at(x,d)?+1)/2) as u8;
        let axial=((at(l,y)?+at(r,y)?+at(x,u)?+at(x,d)?+2)/4) as u8;
        let diagonal=((at(l,u)?+at(r,u)?+at(l,d)?+at(r,d)?+2)/4) as u8;
        Some(match (y%2,x%2) {(0,0)=>[horizontal,c,vertical],(0,1)=>[c,axial,diagonal],(1,0)=>[diagonal,axial,c],_=>[vertical,c,horizontal]})
    }
    /// Malformed storage returns an empty buffer; valid RGB8 remains borrowed.
    pub fn display_rgb(&self)->std::borrow::Cow<'_,[u8]> {
        let Some((w,h,channels))=self.rgb_layout() else {return std::borrow::Cow::Borrowed(&[]);};
        if channels==3 {return std::borrow::Cow::Borrowed(&self.raw);}
        let mut out=Vec::with_capacity(w*h*3);
        for y in 0..self.metadata.height {for x in 0..self.metadata.width {
            let Some(p)=self.rgb(x,y) else {return std::borrow::Cow::Borrowed(&[]);};
            out.extend_from_slice(&p);
        }}
        std::borrow::Cow::Owned(out)
    }
    pub fn preview_rgb(&self,max_edge:u32)->Result<(u32,u32,Vec<u8>),String> {
        crate::display_recipe::Recipe::neutral().render(self,Some(max_edge))
    }
    pub fn legacy_rgb(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        self.rgb_layout()?;
        if x >= self.metadata.width || y >= self.metadata.height {
            return None;
        }
        if self.metadata.format == "BayerGRBG8" {
            // Micron datasheet p9: even row G/R, odd row B/G.
            // Each 2x2 cell becomes one RGB value repeated over the cell.
            let w = self.metadata.width as usize;
            let i = (y as usize & !1) * w + (x as usize & !1);
            return Some([
                *self.raw.get(i + 1)?,
                ((u16::from(*self.raw.get(i)?) + u16::from(*self.raw.get(i + w + 1)?)) / 2) as u8,
                *self.raw.get(i + w)?,
            ]);
        }
        let i = (y as usize * self.metadata.width as usize + x as usize) * 3;
        self.raw.get(i..i + 3)?.try_into().ok()
    }
    pub fn legacy_display_rgb(&self) -> std::borrow::Cow<'_, [u8]> {
        let Some((w,h,channels))=self.rgb_layout() else {return std::borrow::Cow::Borrowed(&[]);};
        if channels == 3 {
            return std::borrow::Cow::Borrowed(self.raw.as_slice());
        }
        let mut rgb = Vec::with_capacity(w*h*3);
        // Compute each GR/BG cell once; retain the exact original reconstruction.
        for y in (0..self.metadata.height).step_by(2) {
            let row_start = rgb.len();
            for x in (0..self.metadata.width).step_by(2) {
                let Some(pixel) = self.legacy_rgb(x, y) else {return std::borrow::Cow::Borrowed(&[]);};
                rgb.extend_from_slice(&pixel);
                rgb.extend_from_slice(&pixel);
            }
            rgb.extend_from_within(row_start..row_start + w * 3);
        }
        std::borrow::Cow::Owned(rgb)
    }
    /// Bounded nearest-cell preview. Exports always use the original dimensions.
    pub fn legacy_preview_rgb(&self, max_edge: u32) -> Result<(u32, u32, Vec<u8>), String> {
        self.validate()?;
        if max_edge == 0 {
            return Err("Preview bound must be positive".into());
        }
        let stride = self
            .metadata
            .width
            .max(self.metadata.height)
            .div_ceil(max_edge);
        let width = self.metadata.width.div_ceil(stride);
        let height = self.metadata.height.div_ceil(stride);
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(
                    &self
                        .legacy_rgb(x * stride, y * stride)
                        .ok_or("Preview coordinate")?,
                );
            }
        }
        Ok((width, height, pixels))
    }
}
pub trait CameraAdapter {
    fn next_frame(&mut self) -> Result<Frame, String>;
}
pub struct Simulation {
    pub sequence: u64,
}
impl CameraAdapter for Simulation {
    fn next_frame(&mut self) -> Result<Frame, String> {
        let mut raw = Vec::with_capacity(640 * 480 * 3);
        for y in 0..480 {
            for x in 0..640 {
                raw.extend_from_slice(&[
                    (x * 255 / 640) as u8,
                    (y * 255 / 480) as u8,
                    ((x + y + self.sequence as usize) % 256) as u8,
                ]);
            }
        }
        self.sequence += 1;
        Ok(Frame {
            raw: Arc::new(raw),
            metadata: Metadata {
                acquisition: None,
source: "Explicit software simulation".into(),
                simulated: true,
                sequence: self.sequence,
                measurement: None, unix_millis: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_millis(),
                width: 640,
                height: 480,
                format: "RGB8".into(),
            },
        })
    }
}
pub struct UnsupportedUsb {
    pub identity: String,
}
impl CameraAdapter for UnsupportedUsb {
    fn next_frame(&mut self) -> Result<Frame, String> {
        Err(format!(
            "{}: exact device/sensor protocol unvalidated; acquisition disabled",
            self.identity
        ))
    }
}
pub fn export(f: &Frame, path: &Path) -> Result<(), String> {
    f.validate()?;
    let format = match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => image::ImageFormat::Png,
        Some("tif" | "tiff") => image::ImageFormat::Tiff,
        _ => return Err("Choose .png or .tiff".into()),
    };
    let filename = path.file_name().ok_or("Missing filename")?;
    let bundle = path.with_file_name(format!("{}.capture", filename.to_string_lossy()));
    std::fs::create_dir(&bundle).map_err(|e| e.to_string())?;
    let result = (|| {
        image::save_buffer_with_format(
            bundle.join(filename),
            &f.legacy_display_rgb(),
            f.metadata.width,
            f.metadata.height,
            image::ColorType::Rgb8,
            format,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(bundle.join("raw.bin"), f.raw.as_slice()).map_err(|e| e.to_string())?;
        std::fs::write(
            bundle.join("metadata.json"),
            serde_json::to_vec_pretty(&f.metadata).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(bundle.join("COMPLETE"), b"camera-lab capture v1\n")
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(bundle);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolution_exports_preserve_original_pixels_and_metadata() {
        let root = std::env::temp_dir().join(format!("resolution-export-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for mode in controls::Mode::ALL {
            let p = mode.profile();
            let raw = (0..usize::from(p.width) * usize::from(p.height))
                .map(|i| ((i * 73 + 19) % 251) as u8)
                .collect();
            let f = Frame {
                raw: Arc::new(raw),
                metadata: Metadata {
                    acquisition: None,
source: format!("generated profile {} row20 col32", p.id),
                    simulated: true,
                    sequence: 17,
                    measurement: None, unix_millis: 123,
                    width: p.width.into(),
                    height: p.height.into(),
                    format: "BayerGRBG8".into(),
                },
            };
            // Changing a draft mode must never resize a retained frame/export.
            let retained = f.clone();
            let mut settings = controls::Settings {
                mode,
                ..Default::default()
            };
            settings.mode = controls::Mode::Full;
            assert_eq!(settings.dimensions(), (2048, 1536));
            for ext in ["png", "tiff"] {
                let filename = format!("{}.{ext}", p.id.replace('/',"_"));
                export(&retained, &root.join(&filename)).unwrap();
                let bundle = root.join(format!("{filename}.capture"));
                assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(), *f.raw);
                let metadata: Metadata =
                    serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap())
                        .unwrap();
                assert_eq!(metadata, f.metadata);
                let image = image::open(bundle.join(&filename)).unwrap().to_rgb8();
                assert_eq!(image.dimensions(), (p.width.into(), p.height.into()));
                for y in 0..u32::from(p.height) {
                    for x in 0..u32::from(p.width) {
                        assert_eq!(image.get_pixel(x, y).0, f.legacy_rgb(x, y).unwrap());
                    }
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cell_conversion_matches_per_pixel_reference() {
        for (width, height) in [(2, 2), (6, 4), (640, 480), (2048, 1536)] {
            let f = Frame {
                raw: Arc::new(
                    (0..width * height)
                        .map(|i| ((i * 73 + 19) % 251) as u8)
                        .collect(),
                ),
                metadata: Metadata {
                    acquisition: None,
source: "synthetic conversion regression".into(),
                    simulated: true,
                    sequence: 0,
                    measurement: None, unix_millis: 0,
                    width,
                    height,
                    format: "BayerGRBG8".into(),
                },
            };
            f.validate().unwrap();
            let (pw, ph, preview) = f.legacy_preview_rgb(1024).unwrap();
            assert!(pw <= 1024 && ph <= 1024);
            let stride = width.max(height).div_ceil(1024);
            for py in 0..ph {
                for px in 0..pw {
                    let i = ((py * pw + px) * 3) as usize;
                    assert_eq!(
                        &preview[i..i + 3],
                        &f.legacy_rgb(px * stride, py * stride).unwrap()
                    );
                }
            }
            assert!(f.legacy_preview_rgb(0).is_err());
            let converted = f.legacy_display_rgb();
            for y in 0..height {
                for x in 0..width {
                    let i = ((y * width + x) * 3) as usize;
                    assert_eq!(&converted[i..i + 3], &f.legacy_rgb(x, y).unwrap());
                }
            }
        }
    }
    #[test]
    fn bayer_channel_mapping_and_lossless_bundle() {
        let f = Frame {
            raw: Arc::new(vec![20, 80, 20, 80, 10, 20, 10, 20]),
            metadata: Metadata {
                acquisition: None,
source: "documented GRBG cell fixture".into(),
                simulated: false,
                sequence: 9,
                measurement: None, unix_millis: 1,
                width: 4,
                height: 2,
                format: "BayerGRBG8".into(),
            },
        };
        f.validate().unwrap();
        for y in 0..2 {
            for x in 0..4 {
                assert_eq!(f.legacy_rgb(x, y), Some([80, 20, 10]));
            }
        }
        let root =
            std::env::temp_dir().join(format!("camera-lab-bayer-test-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for extension in ["png", "tiff"] {
            let path = root.join(format!("bayer.{extension}"));
            export(&f, &path).unwrap();
            let bundle = root.join(format!("bayer.{extension}.capture"));
            assert_eq!(std::fs::read(bundle.join("raw.bin")).unwrap(), *f.raw);
            assert_eq!(
                image::open(bundle.join(path.file_name().unwrap()))
                    .unwrap()
                    .to_rgb8()
                    .as_raw(),
                f.legacy_display_rgb().as_ref()
            );
            let metadata: Metadata =
                serde_json::from_slice(&std::fs::read(bundle.join("metadata.json")).unwrap())
                    .unwrap();
            assert_eq!(metadata, f.metadata);
        }
        let mut bad = f.clone();
        bad.metadata.width = 3;
        assert!(bad.validate().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounds_and_validation() {
        let mut f = Simulation { sequence: 0 }.next_frame().unwrap();
        f.validate().unwrap();
        assert!(f.metadata.simulated);
        assert_eq!(f.legacy_rgb(0, 0), Some([0, 0, 0]));
        assert_eq!(f.legacy_rgb(640, 0), None);
        f.metadata.width = 0;
        assert!(f.validate().is_err());
    }
    #[test]
    fn hardware_disabled() {
        assert!(
            UnsupportedUsb {
                identity: "unknown".into()
            }
            .next_frame()
            .is_err()
        );
    }
    #[test]
    fn lossless_export() {
        let root = std::env::temp_dir().join(format!("camera-lab-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let f = Simulation { sequence: 0 }.next_frame().unwrap();
        for ext in ["png", "tiff"] {
            let p = root.join(format!("test.{ext}"));
            export(&f, &p).unwrap();
            let b = root.join(format!("test.{ext}.capture"));
            assert_eq!(
                image::open(b.join(p.file_name().unwrap()))
                    .unwrap()
                    .to_rgb8()
                    .as_raw(),
                f.raw.as_ref()
            );
            assert_eq!(std::fs::read(b.join("raw.bin")).unwrap(), *f.raw);
            let m: Metadata =
                serde_json::from_slice(&std::fs::read(b.join("metadata.json")).unwrap()).unwrap();
            assert_eq!(m, f.metadata);
            assert!(export(&f, &p).is_err());
            assert!(b.join("COMPLETE").exists());
        }
        assert!(export(&f, &root.join("bad.jpg")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub mod coordination;

pub mod progress;

pub mod display_recipe;

#[cfg(test)] mod correction_v031 {
 use super::*;
 fn frame(w:u32,h:u32,format:&str,raw:Vec<u8>)->Frame {Frame{raw:Arc::new(raw),metadata:Metadata{acquisition:None,source:"independent malformed fixture".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:w,height:h,format:format.into()}}}
 #[test] fn malformed_public_rgb_never_indexes_or_panics() {
  for f in [frame(2,2,"BayerGRBG8",vec![]),frame(2,2,"BayerGRBG8",vec![1,2,3]),frame(2,2,"unknown",vec![1;4]),frame(3,2,"BayerGRBG8",vec![1;6]),frame(2,3,"BayerGRBG8",vec![1;6]),frame(0,2,"BayerGRBG8",vec![]),frame(2,0,"BayerGRBG8",vec![]),frame(u32::MAX,u32::MAX,"BayerGRBG8",vec![]),frame(u32::MAX-1,u32::MAX-1,"BayerGRBG8",vec![]),frame(u32::MAX,u32::MAX,"RGB8",vec![]),frame(2,2,"RGB8",vec![1;11]),frame(2,2,"BayerGRBG8",vec![1;5])] {
   for (x,y) in [(0,0),(1,1),(u32::MAX,u32::MAX)] {assert_eq!(f.rgb(x,y),None);}
   assert!(f.display_rgb().is_empty());assert!(f.preview_rgb(1024).is_err());
  }
 }
 #[test] fn valid_small_rgb8_and_checked_bounds() {
  let f=frame(2,2,"RGB8",vec![1,2,3,4,5,6,7,8,9,10,11,12]);
  assert_eq!(f.rgb(0,0),Some([1,2,3]));assert_eq!(f.rgb(1,1),Some([10,11,12]));assert_eq!(f.rgb(2,0),None);assert_eq!(f.rgb(0,2),None);assert_eq!(f.display_rgb().as_ref(),f.raw.as_slice());
 }
}

#[cfg(test)] mod correction_v032 {
 use super::*;
 fn frame(w:u32,h:u32,fmt:&str,n:usize)->Frame {Frame{raw:Arc::new(vec![7;n]),metadata:Metadata{acquisition:None,source:"own malformed matrix".into(),simulated:true,sequence:32,measurement:None,unix_millis:0,width:w,height:h,format:fmt.into()}}}
 fn reject(f:Frame) {
  for (x,y) in [(0,0),(1,1),(u32::MAX,u32::MAX)] {assert_eq!(f.legacy_rgb(x,y),None);assert_eq!(f.rgb(x,y),None);}
  assert!(f.legacy_display_rgb().is_empty());assert!(f.display_rgb().is_empty());
  assert!(f.legacy_preview_rgb(8).is_err());assert!(f.preview_rgb(8).is_err());
 }
 #[test] fn malformed_format() {reject(frame(2,2,"unknown",12));}
 #[test] fn malformed_short_bayer() {reject(frame(2,2,"BayerGRBG8",0));}
 #[test] fn malformed_excess_bayer() {reject(frame(2,2,"BayerGRBG8",5));}
 #[test] fn malformed_short_rgb() {reject(frame(2,2,"RGB8",11));}
 #[test] fn malformed_excess_rgb() {reject(frame(2,2,"RGB8",13));}
 #[test] fn malformed_odd_bayer() {reject(frame(3,2,"BayerGRBG8",6));reject(frame(2,3,"BayerGRBG8",6));}
 #[test] fn malformed_zero() {reject(frame(0,2,"BayerGRBG8",0));reject(frame(2,0,"RGB8",0));}
 #[test] fn malformed_oversize() {for (w,h) in [(8194,2),(2,8194),(u32::MAX,u32::MAX),(u32::MAX-1,u32::MAX-1)] {for fmt in ["RGB8","BayerGRBG8"] {reject(frame(w,h,fmt,0));}}}
 #[test] fn valid_legacy_literal_and_borrowing() {
  let mut f=frame(4,2,"BayerGRBG8",8);f.raw=Arc::new(vec![2,20,6,40,60,10,80,14]);
  let expected=[20,6,60,20,6,60,40,10,80,40,10,80,20,6,60,20,6,60,40,10,80,40,10,80];
  assert_eq!(f.legacy_display_rgb().as_ref(),expected);for y in 0..2 {for x in 0..4 {assert_eq!(f.legacy_rgb(x,y).unwrap(),expected[((y*4+x)*3) as usize..((y*4+x)*3+3) as usize]);}}
  for (x,y) in [(4,0),(0,2),(u32::MAX,u32::MAX)] {assert_eq!(f.legacy_rgb(x,y),None);assert_eq!(f.rgb(x,y),None);}
  let f=frame(3,1,"RGB8",9);for out in [f.display_rgb(),f.legacy_display_rgb()] {assert!(matches!(out,std::borrow::Cow::Borrowed(_)));assert_eq!(out.as_ptr(),f.raw.as_ptr());}
 }
}

pub mod failure_record;

pub mod diagnostic;
