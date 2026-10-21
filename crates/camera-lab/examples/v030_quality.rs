//! Offline reference-free evidence generator and bounded host render benchmark.
use camera_lab::{Frame,Metadata,display_recipe::Recipe};
use std::{path::PathBuf,sync::Arc,time::Instant};
fn frame(w:u32,h:u32,raw:Vec<u8>)->Frame {Frame{raw:Arc::new(raw),metadata:Metadata{acquisition:None,source:"own offline chart/collected RAW".into(),simulated:true,sequence:1,measurement:None,unix_millis:0,width:w,height:h,format:"BayerGRBG8".into()}}}
fn png(p:PathBuf,w:u32,h:u32,data:&[u8]) {image::save_buffer(p,data,w,h,image::ColorType::Rgb8).unwrap();}
fn main() {
 let root=PathBuf::from(std::env::args().nth(1).expect("evidence root"));std::fs::create_dir_all(root.join("comparisons")).unwrap();let mut records=Vec::new();
 let chart=|w:u32,h:u32|frame(w,h,(0..h).flat_map(|y|(0..w).map(move |x| {let c=match(y%2,x%2){(0,1)=>0,(1,0)=>2,_=>1};if x<w/3 {if x%8<4 {220}else{20}}else if x<2*w/3 {if x%64>y%64 {230}else{10}}else{[40+(x%128) as u8,50+(y%128) as u8,100][c]}})).collect());
 let synthetic=chart(1024,768);std::fs::write(root.join("comparisons/synthetic.raw"),synthetic.raw.as_slice()).unwrap();
 let mut inputs=vec![("synthetic".to_string(),synthetic)];
 let collected=PathBuf::from(std::env::args().nth(2).expect("own collected directory"));
 for name in ["skip-start.png","full-start.tiff"] {let m:serde_json::Value=serde_json::from_slice(&std::fs::read(collected.join(format!("{name}.json"))).unwrap()).unwrap();let meta:Metadata=serde_json::from_value(m["frame"].clone()).unwrap();inputs.push((name.to_string(),Frame{raw:Arc::new(std::fs::read(collected.join(format!("{name}.raw"))).unwrap()),metadata:meta}));}
 for (name,f) in inputs {
  let mut outputs=Vec::new();let mut neutral_pairs=Vec::new();for (label,r) in [("before-neutral-v1",Recipe::legacy()),("after-neutral-v2",Recipe::neutral()),("appearance-v2",Recipe::reference()),("user-default-v033",Recipe::default())] {
   let (w,h,p)=r.render(&f,None).unwrap();let full=root.join("comparisons").join(format!("{name}-{label}-full.png"));png(full.clone(),w,h,&p);
   let (fw,fh,fit)=r.render(&f,Some(1024)).unwrap();let fitpath=root.join("comparisons").join(format!("{name}-{label}-fit.png"));png(fitpath.clone(),fw,fh,&fit);
   let (cw,ch)=(w.min(256),h.min(256));let (cx,cy)=((w-cw)/2,(h-ch)/2);let mut crop=Vec::new();for y in cy..cy+ch {crop.extend_from_slice(&p[((y*w+cx)*3) as usize..((y*w+cx+cw)*3) as usize]);}let cp=root.join("comparisons").join(format!("{name}-{label}-crop-1to1.png"));png(cp.clone(),cw,ch,&crop);
   if label=="before-neutral-v1" || label=="after-neutral-v2" {neutral_pairs.push((fit.clone(),fw,fh,crop.clone(),cw,ch));}
   outputs.push(serde_json::json!({"recipe":r,"processing":r.processing(&f.metadata.format),"full":full,"dimensions":[w,h],"output_pixels":w*h,"fit":fitpath,"fit_dimensions":[fw,fh],"crop":cp,"crop_origin":[cx,cy],"crop_dimensions":[cw,ch]}));
  }
  for crop in [false,true] {let (a,w,h,b,_,_)=&neutral_pairs[0];let (c,_,_,d,_,_)=&neutral_pairs[1];let (left,right,w,h)=if crop {(b,d,256.min(*w),256.min(*h))}else{(a,c,*w,*h)};
    let mut pair=Vec::new();for y in 0..h {for pixels in [left,right] {pair.extend_from_slice(&pixels[(y*w*3) as usize..((y+1)*w*3) as usize]);}}
    png(root.join("comparisons").join(format!("{name}-before-left-after-right-{}.png",if crop {"crop-1to1"}else{"fit"})),2*w,h,&pair);
  }
  records.push(serde_json::json!({"input":name,"raw_bytes":f.raw.len(),"source_dimensions":[f.metadata.width,f.metadata.height],"outputs":outputs}));
 }
 std::fs::write(root.join("comparisons/outputs.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
 let mut bench=Vec::new();for (w,h) in [(1024,768),(2048,1536)] {let f=chart(w,h);for (label,r) in [("legacy-neutral",Recipe::legacy()),("v2-neutral",Recipe::neutral()),("v2-appearance",Recipe::default())] {let start=Instant::now();for _ in 0..12 {std::hint::black_box(r.render(&f,Some(1024)).unwrap());}let elapsed=start.elapsed().as_secs_f64();bench.push(serde_json::json!({"dimensions":[w,h],"recipe":label,"renders":12,"wall_seconds":elapsed,"host_renders_per_second":12./elapsed,"physical_fps":false}));}}
 std::fs::write(root.join("validation/render-benchmark.json"),serde_json::to_vec_pretty(&bench).unwrap()).unwrap();
}
