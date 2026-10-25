use super::*;
#[derive(Default)]
pub(crate) struct UvControls {
    islands: Vec<Vec<usize>>,
    report: crate::sculpt::uv::Report,
    stretch: bool,
    texture_tab: bool,
    pub channel: crate::sculpt::texture::Channel,
    pub image_path: String,
    pub export_overwrite: Option<String>,
    pub reload_conflict: bool,
    image_preview: Option<((u64, u64), egui::TextureHandle)>,
    geometry_hash: Option<u64>,
    size: u32,
    export_path: String,
    pub open: bool,
    pub checker: bool,
    pub selected: Vec<usize>,
    padding: f32,
    offset: [f32; 2],
    scale: f32,
    angle: f32,
    revision: Option<u64>,
}
impl UiState {
    pub(crate) fn uv_window(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
        model_faces: &[usize],
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        if !self.uv.open {
            return commands;
        }
        if self.uv.revision.is_none() {
            self.uv.padding = 0.01;
            self.uv.export_path = "texture.png".into();
        }
        let mesh = document.mesh();
        let uv = &mesh.uv;
        let current = uv.current(mesh);
        if self.uv.revision != Some(document.display_revision()) {
            self.uv.selected.retain(|&f| f < mesh.triangles.len());
            self.uv.revision = Some(document.display_revision());
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            for p in &mesh.vertices {
                [p.0.to_bits(), p.1.to_bits(), p.2.to_bits()].hash(&mut hash);
            }
            mesh.triangles.hash(&mut hash);
            current.hash(&mut hash);
            for q in uv.corners.iter().flatten() {
                q.map(f32::to_bits).hash(&mut hash);
            }
            let hash = hash.finish();
            if self.uv.geometry_hash != Some(hash) {
                self.uv.islands = if current { uv.islands() } else { Vec::new() };
                self.uv.report = uv.inspect(mesh);
                self.uv.geometry_hash = Some(hash);
            }
        }
        let mut open = true;
        egui::Window::new("UVs and textures").id(egui::Id::new("uv_editor")).open(&mut open).default_width(480.)
            .default_pos(egui::pos2(20.,110.)).default_height(800.).max_height((ctx.content_rect().height()-140.).max(200.)).vscroll(true).show(ctx,|ui|{
            if self.mesh_busy || self.pending.is_some(){ui.disable();}
            ui.horizontal(|ui|{ui.selectable_value(&mut self.uv.texture_tab,false,"UV layout");ui.selectable_value(&mut self.uv.texture_tab,true,"Texture images");});
            if self.uv.texture_tab { ui.scope(|ui|{
                use crate::sculpt::texture::Channel;
                use crate::app::textures::TextureCommand as T;
                ui.horizontal_wrapped(|ui|{for channel in Channel::ALL{if ui.selectable_value(&mut self.uv.channel,channel,channel.label()).changed(){self.uv.reload_conflict=false;}}});
                if self.uv.size==0{self.uv.size=512;}
                ui.horizontal_wrapped(|ui|{
                    egui::ComboBox::from_id_salt("texture_resolution").selected_text(format!("{} × {}",self.uv.size,self.uv.size)).show_ui(ui,|ui|{for size in [256,512,1024,2048]{ui.selectable_value(&mut self.uv.size,size,size.to_string());}});
                    if ui.button("Create texture").clicked(){commands.push(Command::Texture(T::Create(self.uv.channel,self.uv.size)));}
                    if ui.button("Import PNG / JPEG…").clicked(){commands.push(Command::BrowseAsset(crate::command::AssetKind::Texture));}
                });
                if let Some(texture)=uv.textures.iter().find(|t|t.channel==self.uv.channel){
                    ui.label(format!("{} · {} × {} · RGBA16",texture.name,texture.width,texture.height));
                    ui.horizontal_wrapped(|ui|{
                        if ui.selectable_label(document.texture_paint==Some(self.uv.channel),"Paint this texture").clicked(){commands.push(Command::Texture(T::Paint(Some(self.uv.channel))));}
                        if ui.button("Paint vertices").clicked(){commands.push(Command::Texture(T::Paint(None)));}
                        if ui.button("Make independent").clicked(){commands.push(Command::Texture(T::Copy(self.uv.channel)));}
                        if ui.button("Remove texture").clicked(){commands.push(Command::Texture(T::Remove(self.uv.channel)));}
                    });
                    ui.label("Images are shared by duplicated models. Make independent to edit only this model. Overlapping UVs share pixels.");
                    ui.horizontal_wrapped(|ui|{ui.label("PNG export");ui.text_edit_singleline(&mut self.uv.export_path);if ui.button("Export PNG").clicked(){commands.push(Command::Texture(T::Export(self.uv.channel,self.uv.export_path.clone(),false)));}});
                    if let Some(path)=self.uv.export_overwrite.clone(){ui.horizontal_wrapped(|ui|{ui.label(format!("Replace {path}?"));if ui.button("Replace PNG").clicked(){commands.push(Command::Texture(T::Export(self.uv.channel,path,true)));}if ui.button("Keep existing file").clicked(){self.uv.export_overwrite=None;}});}
                    ui.horizontal_wrapped(|ui|{
                        if ui.button("Edit in GIMP").clicked(){commands.push(Command::Texture(T::EditExternal(self.uv.channel)));}
                        if ui.add_enabled(texture.exchange.is_some(),egui::Button::new("Reload external image")).clicked(){commands.push(Command::Texture(T::Reload(self.uv.channel,false)));}
                    });
                    if self.uv.reload_conflict{ui.colored_label(Color32::YELLOW,"Local paint changed after export. Reload would replace it.");ui.horizontal(|ui|{if ui.button("Replace local paint").clicked(){commands.push(Command::Texture(T::Reload(self.uv.channel,true)));}if ui.button("Keep local paint").clicked(){self.uv.reload_conflict=false;}});}
                    if let Some(exchange)=&texture.exchange{ui.label(format!("Exchange image: {}",exchange.path));}
                    ui.label("GIMP: export back to the exchange PNG before Reload. Save layered XCF separately. Imported pixels are embedded in the project.");
                }
                ui.horizontal(|ui|{ui.label("GIMP executable");ui.add(egui::TextEdit::singleline(&mut self.preferences.gimp_path).hint_text("gimp"));});
            }); }
            ui.add_enabled_ui(!self.mesh_busy && self.pending.is_none(),|ui|{
                if !self.uv.texture_tab { ui.horizontal_wrapped(|ui|{
                    if ui.button("Automatic unwrap").clicked(){commands.push(Command::UvUnwrap(true,self.uv.padding));}
                    if ui.button("Unwrap marked seams").clicked(){commands.push(Command::UvUnwrap(false,self.uv.padding));}
                    if ui.button("Mark selected edges").clicked(){commands.push(Command::UvSeams(true));}
                    if ui.button("Clear selected seams").clicked(){commands.push(Command::UvSeams(false));}
                });
                ui.label("Select edges in Modeling to define seams. Unwrap runs in the background and can be cancelled."); }
                if !current {
                    ui.colored_label(Color32::YELLOW,if uv.corners.is_empty(){"Generate UVs to begin, or import a UV-mapped OBJ."}else{"Topology changed: these UVs are inactive. Undo the topology edit or unwrap again."});
                }
                ui.add_enabled_ui(current,|ui|{
                    if !self.uv.texture_tab { ui.horizontal_wrapped(|ui|{
                        ui.checkbox(&mut self.uv.checker,"3D checker");
                        ui.checkbox(&mut self.uv.stretch,"UV stretch");
                        ui.add(egui::DragValue::new(&mut self.uv.padding).range(0.001..=0.1).speed(0.001).prefix("Padding "));
                        if ui.button("Pack islands").clicked(){commands.push(Command::UvPack(self.uv.padding));}
                        if ui.button("Select all").clicked(){self.uv.selected=(0..uv.corners.len()).collect();commands.push(Command::UvSelect(self.uv.selected.clone()));}
                        if ui.button("From 3D selection").clicked(){
                            let polygons=crate::sculpt::uv::polygon_triangles(mesh);let selected:std::collections::BTreeSet<_>=model_faces.iter().filter_map(|&f|polygons.get(f)).flatten().copied().collect();
                            self.uv.selected=self.uv.islands.iter().filter(|fs|fs.iter().any(|f|selected.contains(f))).flatten().copied().collect();
                        }
                    });
                    }
                    let size=ui.available_width().clamp(180.,if self.uv.texture_tab{320.}else{500.});let (rect,response)=ui.allocate_exact_size(Vec2::splat(size),Sense::click());
                    let painter=ui.painter_at(rect);painter.rect_filled(rect,0.,Color32::from_gray(24));
                    let map=|q:[f32;2]|egui::pos2(rect.left()+q[0]*size,rect.bottom()-q[1]*size);
                    for y in 0..16 {for x in 0..16 {if (x+y)%2==0 {painter.rect_filled(egui::Rect::from_min_max(map([x as f32/16.,(y+1) as f32/16.]),map([(x+1) as f32/16.,y as f32/16.])),0.,Color32::from_gray(38));}}}
                    if let Some(texture)=uv.textures.iter().find(|t|t.channel==self.uv.channel){
                        let id=(texture.id,document.display_revision());
                        if self.uv.image_preview.as_ref().is_none_or(|(old,_)|*old!=id){
                            let rgba:Vec<u8>=texture.pixels.iter().map(|v|(*v/257) as u8).collect();
                            self.uv.image_preview=Some((id,ctx.load_texture("uv_texture_image",egui::ColorImage::from_rgba_unmultiplied([texture.width as usize,texture.height as usize],&rgba),egui::TextureOptions::LINEAR)));
                        }
                        if let Some((_,image))=&self.uv.image_preview{painter.image(image.id(),rect,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),Color32::WHITE);}
                    }else{self.uv.image_preview=None;}
                    let selection:std::collections::BTreeSet<_>=self.uv.selected.iter().copied().collect();
                    for (f,t) in uv.corners.iter().enumerate().filter(|_|current){
                        let color=if selection.contains(&f){Color32::from_rgb(255,185,70)}else if crate::sculpt::uv::area(*t).abs()<1e-10{Color32::RED}else if self.uv.stretch{let amount=((self.uv.report.stretch[f]-1.)/3.).clamp(0.,1.);Color32::from_rgb((amount*255.) as u8,((1.-amount)*180.) as u8,90)}else{Color32::from_rgb(115,185,215)};
                        for c in 0..3 {painter.line_segment([map(t[c]),map(t[(c+1)%3])],(1.,color));}
                    }
                    if !self.uv.texture_tab && response.clicked() && let Some(p)=response.interact_pointer_pos(){
                        let q=[(p.x-rect.left())/size,(rect.bottom()-p.y)/size];
                        let face=uv.corners.iter().position(|t|{let a=crate::sculpt::uv::area(*t);if a.abs()<1e-12{return false;}let weights=[crate::sculpt::uv::area([q,t[1],t[2]])/a,crate::sculpt::uv::area([t[0],q,t[2]])/a,crate::sculpt::uv::area([t[0],t[1],q])/a];weights.iter().all(|v|*v>=-1e-5)});
                        if !ui.input(|i|i.modifiers.shift){self.uv.selected.clear();}
                        if let Some(f)=face && let Some(island)=self.uv.islands.iter().find(|fs|fs.contains(&f)){self.uv.selected.extend(island.iter().copied());self.uv.selected.sort_unstable();self.uv.selected.dedup();}
                        commands.push(Command::UvSelect(self.uv.selected.clone()));
                    }
                    if !self.uv.texture_tab { ui.label("Click an island; Shift-click adds. Orange islands are selected in 3D too.");
                    ui.horizontal_wrapped(|ui|{
                        for k in 0..2 {ui.add(egui::DragValue::new(&mut self.uv.offset[k]).speed(0.01).prefix(if k==0{"U "}else{"V "}));}
                        if self.uv.scale==0.{self.uv.scale=1.;}
                        ui.add(egui::DragValue::new(&mut self.uv.scale).range(0.001..=100.).speed(0.01).prefix("Scale "));
                        ui.add(egui::DragValue::new(&mut self.uv.angle).speed(1.).prefix("Rotate ° "));
                        if ui.button("Apply transform").clicked(){commands.push(Command::UvTransform(self.uv.selected.clone(),self.uv.offset,self.uv.scale,self.uv.angle.to_radians()));self.uv.offset=[0.;2];self.uv.scale=1.;self.uv.angle=0.;}
                    });
                    ui.label(format!("{} degenerate triangles · {} overlapping pairs · {} outside 0–1{}",self.uv.report.degenerate,self.uv.report.overlaps,self.uv.report.outside,if self.uv.report.complete{""}else{" · overlap check reached its limit"}));
                    ui.label(format!("{} islands · {} seams · UV changes are undoable",self.uv.islands.len(),uv.seams.len())); }
                });
            });
        });
        self.uv.open = open;
        commands
    }
}
