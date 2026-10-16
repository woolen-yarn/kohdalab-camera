//! Independent software-only rendering. Sensor bytes and typed metadata never change.
use crate::Frame;
use serde::{Serialize, Deserialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub version: u32,
    pub ev: f32,
    pub gamma: f32,
    pub wb: [f32;3],
    pub monochrome: bool,
    pub mirror: bool,
    pub flip: bool,
    pub quarter_turns: u8,
}
impl Default for Recipe {
    fn default()->Self { Self {ev:1.20_f32.log2(),gamma:2.2,quarter_turns:2,..Self::neutral()} }
}
impl Recipe {
    pub fn reference()->Self {Self {ev:1.5,gamma:2.2,quarter_turns:2,..Self::neutral()}}
    pub fn brightness(self)->f32 {2_f32.powf(self.ev)}
    pub fn set_brightness(&mut self,value:f32) {if value.is_finite() && (0.0625..=16.).contains(&value) {self.ev=value.log2();}}
    pub fn neutral()->Self {Self {version:2,ev:0.,gamma:1.,wb:[1.;3],monochrome:false,mirror:false,flip:false,quarter_turns:0}}
    pub fn legacy()->Self {Self {version:1,..Self::neutral()}}
    pub fn processing(self,format:&str)->String {
        let reconstruction=if format=="RGB8" {"source-RGB8"}else if self.version==1 {"GRBG-cell-RGB8"}else {"GRBG-bilinear-reflect101-u16-half-up"};
        format!("{reconstruction}; software recipe/v{}; {}",self.version,if self.version==1 {"integer-stride"}else {"channel-gain-clamp-gamma-LUT256; mono-post-gamma-half-up; processed-RGB-box-u64-half-up; terminal-box-clipped"})
    }

    pub fn validate(self)->Result<(),String> {
        if !matches!(self.version,1|2) || !self.ev.is_finite() || !(-4.0..=4.0).contains(&self.ev) || !self.gamma.is_finite() || !(0.25..=4.).contains(&self.gamma) || self.wb.iter().any(|v| !v.is_finite() || !(0.25..=4.).contains(v)) || self.quarter_turns>3 {return Err("Invalid software display recipe".into());} Ok(())
    }
    pub fn dimensions(self,w:u32,h:u32)->(u32,u32) {if self.quarter_turns%2==1 {(h,w)} else {(w,h)}}
    /// Output to original sensor pixel; reflection precedes clockwise rotation.
    pub fn source(self,x:u32,y:u32,w:u32,h:u32)->(u32,u32) {
        let (mut x,mut y)=match self.quarter_turns {1=>(y,h-1-x),2=>(w-1-x,h-1-y),3=>(w-1-y,x),_=>(x,y)};
        if self.mirror {x=w-1-x;} if self.flip {y=h-1-y;} (x,y)
    }
    pub fn pixel(self,rgb:[u8;3])->[u8;3] {
        if self.version==2 && self.monochrome {
            let p=Self{monochrome:false,..self}.pixel(rgb);
            let mean=((u16::from(p[0])+u16::from(p[1])+u16::from(p[2])+1)/3) as u8;return [mean;3];
        }
        if self.ev==0. && self.gamma==1. && self.wb==[1.;3] && !self.monochrome {return rgb;}
        let mut p=std::array::from_fn::<_,3,_>(|i| (f32::from(rgb[i])/255.*2f32.powf(self.ev)*self.wb[i]).clamp(0.,1.));
        if self.monochrome {let mean=(p[0]+p[1]+p[2])/3.;p=[mean;3];}
        p.map(|v| (255.*v.powf(1./self.gamma)).round().clamp(0.,255.) as u8)
    }
    pub fn render(self,f:&Frame,max_edge:Option<u32>)->Result<(u32,u32,Vec<u8>),String> {
        f.validate()?;self.validate()?;
        let (w,h)=self.dimensions(f.metadata.width,f.metadata.height);
        let stride=match max_edge {Some(0)=>return Err("Zero preview bound".into()),Some(n)=>w.max(h).div_ceil(n),None=>1};
        let (ow,oh)=(w.div_ceil(stride),h.div_ceil(stride));
        if self.version==2 {
            // LUT is bounded by recipe, never pixel count. V2 monochrome
            // averages processed encoded channels (half up), before box scaling.
            let gain=2f32.powf(self.ev);
            let linear: [[f32;256];3]=std::array::from_fn(|c|std::array::from_fn(|v|(v as f32/255.*gain*self.wb[c]).clamp(0.,1.)));
            let lut:[[u8;256];3]=std::array::from_fn(|c|std::array::from_fn(|v|(255.*linear[c][v].powf(1./self.gamma)).round() as u8));
            let mut out=Vec::with_capacity((ow*oh*3) as usize);
            for by in 0..oh {for bx in 0..ow {
                let mut sum=[0u64;3];let mut n=0u64;
                for y in by*stride..((by+1)*stride).min(h) {for x in bx*stride..((bx+1)*stride).min(w) {
                    let (sx,sy)=self.source(x,y,f.metadata.width,f.metadata.height);
                    let p=f.rgb(sx,sy).ok_or("Render coordinate")?;
                    let mut p:[u8;3]=std::array::from_fn(|c|lut[c][p[c] as usize]);
                    if self.monochrome {p=[((u16::from(p[0])+u16::from(p[1])+u16::from(p[2])+1)/3) as u8;3];}
                    for c in 0..3 {sum[c]+=u64::from(p[c]);}n+=1;
                }}
                out.extend_from_slice(&sum.map(|v|((v+n/2)/n) as u8));
            }}return Ok((ow,oh,out));
        }
        let mut out=Vec::with_capacity((ow*oh*3) as usize);
        for y in 0..oh {for x in 0..ow {let (sx,sy)=self.source(x*stride,y*stride,f.metadata.width,f.metadata.height);out.extend_from_slice(&self.pixel(f.legacy_rgb(sx,sy).ok_or("Render coordinate")?));}}
        Ok((ow,oh,out))
    }
    /// Bounded scene estimate from the held valid RAW, never calibration.
    pub fn estimate(self,f:&Frame,white_balance:bool)->Result<Self,String> {
        self.validate()?;f.validate()?;let stride=f.metadata.width.max(f.metadata.height).div_ceil(128);let mut sum=[0f64;3];let mut n=0.;
        for y in (0..f.metadata.height).step_by(stride as usize) {for x in (0..f.metadata.width).step_by(stride as usize) {let p=if self.version==1 {f.legacy_rgb(x,y)}else{f.rgb(x,y)}.ok_or("Estimate coordinate")?;for i in 0..3 {sum[i]+=f64::from(p[i]);}n+=1.;}}
        if sum.iter().any(|v| *v<=0.) || n==0. {return Err("Scene estimate unavailable: zero-content channel".into());}
        let means=sum.map(|v|v/n);let mean=means.iter().sum::<f64>()/3.;let mut r=self;
        if white_balance {r.wb=means.map(|v|(mean/v).clamp(0.25,4.) as f32);} else {r.ev=(128./mean).log2().clamp(-4.,4.) as f32;}
        Ok(r)
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn literal_arithmetic_and_coordinates() {
        let d=Recipe::legacy();assert_eq!(d.pixel([10,15,20]),[10,15,20]);
        assert_eq!(Recipe{ev:1.,..d}.pixel([10,15,20]),[20,30,40]);
        assert_eq!(Recipe{gamma:2.,..d}.pixel([0,64,255]),[0,128,255]);
        assert_eq!(Recipe{wb:[2.,1.,0.5],..d}.pixel([10,20,40]),[20,20,20]);
        assert_eq!(Recipe{quarter_turns:1,..d}.source(0,0,3,2),(0,1));
        assert_eq!(Recipe{quarter_turns:3,..d}.source(1,2,3,2),(0,1));
        assert_eq!(Recipe{mirror:true,flip:true,..d}.source(0,0,3,2),(2,1));
        assert_eq!(Recipe{monochrome:true,..d}.pixel([30,60,90]),[60,60,60]);
    }
    #[test] fn independent_rotation_grid_and_estimate_refusal() {
        let mut f=Frame{raw:std::sync::Arc::new(vec![1,0,0,2,0,0,3,0,0,4,0,0,5,0,0,6,0,0]),metadata:crate::Metadata{acquisition:None,source:"literal grid 123 / 456".into(),simulated:true,sequence:1,measurement: None, unix_millis:0,width:3,height:2,format:"RGB8".into()}};
        for (turn,expected) in [(0,vec![1,2,3,4,5,6]),(1,vec![4,1,5,2,6,3]),(2,vec![6,5,4,3,2,1]),(3,vec![3,6,2,5,1,4])] {
            let r=Recipe{quarter_turns:turn,..Recipe::legacy()};let (_,_,p)=r.render(&f,None).unwrap();assert_eq!(p.chunks_exact(3).map(|p|p[0]).collect::<Vec<_>>(),expected);
        }
        assert!(Recipe::default().estimate(&f,true).is_err());
        f.raw=std::sync::Arc::new(vec![64;18]);let estimated=Recipe::default().estimate(&f,false).unwrap();assert_eq!(estimated.ev,1.);
        assert_eq!(Recipe::default().estimate(&f,true).unwrap().wb,[1.;3]);
    }
}

#[cfg(test)] mod quality_v2 {
    use super::*;
    fn frame(w:u32,h:u32,raw:Vec<u8>,format:&str)->Frame {Frame{raw:std::sync::Arc::new(raw),metadata:crate::Metadata{acquisition:None,source:"reference-free literal".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:w,height:h,format:format.into()}}}
    #[test] fn literal_smallest_border_and_legacy_defect() {
        let f=frame(2,2,vec![0,10,20,30],"BayerGRBG8");
        let literal=vec![10,0,20,10,15,20,10,15,20,10,30,20];
        assert_eq!(Recipe::neutral().render(&f,None).unwrap().2,literal);
        assert_eq!(f.display_rgb().as_ref(),literal);
        assert_eq!(Recipe::legacy().render(&f,None).unwrap().2,vec![10,15,20,10,15,20,10,15,20,10,15,20]);
        assert_eq!(Recipe::neutral().render(&f,Some(1)).unwrap(),(1,1,vec![10,15,20]));
    }
    #[test] fn independent_planes_ramp_diagonal_checker_and_immutability() {
        for kind in 0..3 {
            let plane=|x:u32,y:u32,c:usize| ->u8 {match kind {0=>[20+2*x+4*y,60+4*x+2*y,100+2*x+2*y][c] as u8,1=>if x>y {[210,150,90][c]}else{[10,30,50][c]},_=>if (x/2+y/2)%2==0 {[240,160,80][c]}else{[12,24,36][c]}}};
            let raw=(0..8).flat_map(|y|(0..10).map(move |x|plane(x,y,match(y%2,x%2){(0,1)=>0,(1,0)=>2,_=>1}))).collect();
            let f=frame(10,8,raw,"BayerGRBG8");let before=f.raw.clone();let meta=f.metadata.clone();
            for y in 0..8 {for x in 0..10 {
                let c=match(y%2,x%2){(0,1)=>0,(1,0)=>2,_=>1};let p=f.rgb(x,y).unwrap();
                assert_eq!(p[c],plane(x,y,c));
                if kind==0 && x>0 && x<9 && y>0 && y<7 {assert_eq!(p,std::array::from_fn(|c|plane(x,y,c)));}
            }}
            let r=Recipe{mirror:true,flip:true,quarter_turns:1,..Recipe::neutral()};
            let (w,h,out)=r.render(&f,None).unwrap();assert_eq!((w,h),(8,10));
            // Independent oriented coordinate equation for clockwise turn after both reflections.
            for y in 0..h {for x in 0..w {assert_eq!(&out[((y*w+x)*3) as usize..((y*w+x)*3+3) as usize],&f.rgb(9-y,x).unwrap());}}
            assert_eq!(f.raw,before);assert_eq!(f.metadata,meta);assert_eq!(r.version,2);
        }
    }
    #[test] fn frozen_all_sites_and_reflect101_edges() {
        let f=frame(4,4,(0..16).collect(),"BayerGRBG8");
        assert_eq!(Recipe::neutral().render(&f,None).unwrap().2,vec![
            1,0,4, 1,3,5, 2,2,6, 3,5,6,
            5,5,4, 5,5,5, 6,6,6, 7,7,6,
            9,8,8, 9,9,9, 10,10,10, 11,11,10,
            9,11,12, 9,13,13, 10,12,14, 11,15,14]);
    }
    #[test] fn rgb_identity_scaling_and_invalid() {
        let f=frame(3,2,vec![1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18],"RGB8");
        assert_eq!(Recipe::neutral().render(&f,None).unwrap().2,*f.raw);
        assert_eq!(Recipe::neutral().render(&f,Some(2)).unwrap(),(2,1,vec![7,8,9,12,13,14]));
        for r in [Recipe{version:3,..Recipe::neutral()},Recipe{gamma:f32::NAN,..Recipe::neutral()},Recipe{ev:f32::INFINITY,..Recipe::neutral()}] {assert!(r.render(&f,None).is_err());}
    }
    #[test] fn v2_lut_matches_pixel_and_rgb_metadata_truthful() {
        let f=frame(256,1,(0..=255).flat_map(|v|[v,255-v,v/2]).collect(),"RGB8");
        for monochrome in [false,true] {let r=Recipe{ev:0.5,gamma:2.2,wb:[0.7,1.2,2.],monochrome,..Recipe::neutral()};let (_,_,pixels)=r.render(&f,None).unwrap();for (raw,out) in f.raw.chunks_exact(3).zip(pixels.chunks_exact(3)) {assert_eq!(out,r.pixel(raw.try_into().unwrap()));}}
        let root=std::env::temp_dir().join(format!("rgb-v030-{}",std::process::id()));std::fs::create_dir(&root).unwrap();let p=root.join("rgb.png");crate::image_export::save_recipe(&f,&p,Recipe::neutral()).unwrap();
        let m:serde_json::Value=serde_json::from_slice(&std::fs::read(format!("{}.json",p.display())).unwrap()).unwrap();assert_eq!(m["original_sensor_tiff"]["available"],false);assert!(m["processing"].as_str().unwrap().starts_with("source-RGB8"));assert!(!root.join("rgb.png.original-RAW.tiff").exists());std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn export_full_literal_and_original_tiff() {
        let f=frame(2,2,vec![0,10,20,30],"BayerGRBG8");let r=Recipe::neutral();
        let root=std::env::temp_dir().join(format!("quality-v030-{}",std::process::id()));std::fs::create_dir(&root).unwrap();
        for ext in ["png","tiff"] {let p=root.join(format!("literal.{ext}"));crate::image_export::save_recipe(&f,&p,r).unwrap();
            assert_eq!(image::open(&p).unwrap().to_rgb8().into_raw(),vec![10,0,20,10,15,20,10,15,20,10,30,20]);
            let original=image::open(format!("{}.original-RAW.tiff",p.display())).unwrap().to_luma8();assert_eq!(original.dimensions(),(2,2));assert_eq!(original.as_raw(),f.raw.as_ref());
            let m:serde_json::Value=serde_json::from_slice(&std::fs::read(format!("{}.json",p.display())).unwrap()).unwrap();assert_eq!(m["original_sensor_tiff"]["data_unchanged"],true);assert!(m["processing"].as_str().unwrap().contains("bilinear"));assert_eq!(m["display_recipe"],serde_json::to_value(r).unwrap());
        }std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)] mod correction_v031 {
 use super::*;
 #[test] fn invalid_estimates_rejected_before_arithmetic() {
  let f=Frame{raw:std::sync::Arc::new(vec![64;12]),metadata:crate::Metadata{acquisition:None,source:"literal".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:2,height:2,format:"RGB8".into()}};
  for r in [Recipe{version:3,..Recipe::neutral()},Recipe{ev:f32::NAN,..Recipe::neutral()},Recipe{gamma:f32::NAN,..Recipe::neutral()},Recipe{wb:[1.,f32::NAN,1.],..Recipe::neutral()},Recipe{quarter_turns:4,..Recipe::neutral()},Recipe{ev:5.,..Recipe::legacy()},Recipe{gamma:0.,..Recipe::legacy()},Recipe{wb:[0.,1.,1.],..Recipe::legacy()}] {
   for mode in [false,true] {assert!(r.estimate(&f,mode).is_err(),"accepted {r:?}, mode={mode}");}
  }
  assert_eq!(Recipe::legacy().estimate(&f,false).unwrap().ev,1.);
  assert_eq!(Recipe::legacy().estimate(&f,true).unwrap().wb,[1.;3]);
 }
}

#[cfg(test)] mod brightness_v033 {
 use super::*;
 #[test] fn multiplier_default_range_and_stored_semantics() {
  let mut r=Recipe::default();assert_eq!(format!("{:.2}",r.brightness()),"1.20");assert!((r.ev-1.2_f32.log2()).abs()<1e-7);assert_eq!((r.gamma,r.wb,r.quarter_turns),(2.2,[1.;3],2));
  for (m,ev) in [(0.0625,-4.),(0.5,-1.),(1.,0.),(2.,1.),(16.,4.)] {r.set_brightness(m);assert_eq!(r.ev,ev);assert_eq!(r.brightness(),m);r.validate().unwrap();}
  for bad in [0.,-1.,f32::INFINITY,f32::NAN,17.] {let old=r;r.set_brightness(bad);assert_eq!(r,old);}
  for version in [1,2] {let stored=Recipe{version,ev:-2.,..Recipe::neutral()};let round:Recipe=serde_json::from_value(serde_json::to_value(stored).unwrap()).unwrap();assert_eq!(round,stored);assert_eq!(round.brightness(),0.25);}
  assert_eq!(Recipe::reference().ev,1.5);
 }
}
