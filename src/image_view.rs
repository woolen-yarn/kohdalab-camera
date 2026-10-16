//! Presentation transform only; never changes a frame or export recipe.
use eframe::egui::{Rect,Vec2,Pos2};
#[derive(Clone,Debug)]
pub struct View {pub zoom:f32,pub pan:Vec2}
impl Default for View {fn default()->Self{Self{zoom:1.,pan:Vec2::ZERO}}}
impl View {
    fn fit(area:Rect,size:Vec2)->f32 {(area.width()/size.x).min(area.height()/size.y).max(0.0001)}
    pub fn rect(&self,area:Rect,size:Vec2)->Rect {Rect::from_center_size(area.center()+self.pan,size*Self::fit(area,size)*self.zoom)}
    fn clamp(&mut self,area:Rect,size:Vec2) {
        let rendered=size*Self::fit(area,size)*self.zoom;
        let limit=(rendered-area.size()).max(Vec2::ZERO)*0.5;
        self.pan=self.pan.clamp(-limit,limit);
    }
    pub fn actual(&mut self,area:Rect,size:Vec2) {self.zoom=(1./Self::fit(area,size)).clamp(0.1,32.);self.pan=Vec2::ZERO;}
    pub fn zoom_at(&mut self,area:Rect,size:Vec2,anchor:Pos2,factor:f32) {
        if !factor.is_finite() || factor<=0. {return;}
        let previous=self.zoom;self.zoom=(self.zoom*factor).clamp(0.1,32.);
        let relative=anchor-area.center();self.pan=relative-(relative-self.pan)*(self.zoom/previous);self.clamp(area,size);
    }
    pub fn drag(&mut self,area:Rect,size:Vec2,delta:Vec2) {self.pan+=delta;self.clamp(area,size);}
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn fit_actual_anchor_and_pan_bounds() {
        let area=Rect::from_min_size(Pos2::ZERO,Vec2::new(800.,600.));let size=Vec2::new(1600.,1200.);let mut view=View::default();
        assert_eq!(view.rect(area,size),area);view.actual(area,size);assert_eq!(view.rect(area,size).size(),size);
        let anchor=Pos2::new(500.,350.);let before=(anchor-view.rect(area,size).min)/view.rect(area,size).size();
        view.zoom_at(area,size,anchor,2.);let after=(anchor-view.rect(area,size).min)/view.rect(area,size).size();assert!((before-after).length()<1e-6);
        view.drag(area,size,Vec2::splat(1e6));assert!(view.rect(area,size).contains_rect(area));
        for factor in [1e6,1e-6,f32::NAN] {view.zoom_at(area,size,anchor,factor);assert!((0.1..=32.).contains(&view.zoom));assert!(view.pan.is_finite());}
    }
}
