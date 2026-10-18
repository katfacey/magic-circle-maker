use crate::{
    model::{self, Document, Layer},
    render::{self, ExportKind, Fonts},
};
use eframe::egui::{self, Color32, Key, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Move,
    Rotate,
    Scale,
    Pan,
}
enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Import,
    Font,
    Generate,
    Undo,
    Redo,
    Duplicate,
    Delete,
    Up,
    Down,
    Add(&'static str),
    Export,
}
struct Drag {
    layer: Layer,
    start: [f32; 2],
    tool: Tool,
}
pub struct MakerApp {
    doc: Document,
    saved: Document,
    path: Option<PathBuf>,
    selected: usize,
    fonts: Fonts,
    dark: bool,
    tool: Tool,
    zoom: f32,
    pan: Vec2,
    view_extent: f32,
    guides: bool,
    checker: bool,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
    drag: Option<Drag>,
    history: Vec<Document>,
    redo: Vec<Document>,
    edit_anchor: Option<Document>,
    status: String,
    error: Option<String>,
    show_export: bool,
    show_about: bool,
    show_discard: bool,
    pending_action: Option<Action>,
    closing: bool,
    export_kind: ExportKind,
    export_size: u32,
    worker: Option<mpsc::Receiver<Result<String, String>>>,
    autosave: Option<PathBuf>,
    autosaved: Document,
    last_autosave: Instant,
    recovery: Option<Document>,
    smoke: Option<PathBuf>,
    frames: u32,
    smoke_started: Instant,
    #[cfg(test)]
    tool_rects: [Rect; 4],
    #[cfg(test)]
    canvas_info: (Pos2, f32),
}
impl MakerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, smoke: Option<String>) -> Self {
        Self::with_context(&cc.egui_ctx, smoke)
    }
    fn with_context(ctx: &egui::Context, smoke: Option<String>) -> Self {
        let mut defs = egui::FontDefinitions::default();
        defs.font_data.insert(
            "inter".into(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/inter.ttf")).into(),
        );
        defs.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "inter".into());
        ctx.set_fonts(defs);
        let dark = !std::env::args().any(|a| a == "--light");
        theme(ctx, dark);
        let doc = Document::default();
        let autosave = if smoke.is_none() {
            directories::ProjectDirs::from("io", "katfacey", "MagicCircleMaker")
                .map(|p| p.data_local_dir().join("recovery.mcm"))
        } else {
            None
        };
        let recovery = autosave.as_ref().and_then(|p| Document::open(p).ok());
        Self {
            saved: doc.clone(),
            autosaved: doc.clone(),
            doc,
            path: None,
            selected: 3,
            fonts: Fonts::new(),
            dark,
            tool: Tool::Move,
            zoom: 1.,
            pan: Vec2::ZERO,
            view_extent: 500.,
            guides: true,
            checker: false,
            texture: None,
            dirty: true,
            drag: None,
            history: vec![],
            redo: vec![],
            edit_anchor: None,
            status: "Ready · Select a layer, then choose a transform tool.".into(),
            error: None,
            show_export: false,
            show_about: false,
            show_discard: false,
            pending_action: None,
            closing: false,
            export_kind: ExportKind::Layers,
            export_size: 2048,
            worker: None,
            autosave,
            last_autosave: Instant::now(),
            recovery,
            smoke: smoke.map(PathBuf::from),
            frames: 0,
            smoke_started: Instant::now(),
            #[cfg(test)]
            tool_rects: [Rect::NOTHING; 4],
            #[cfg(test)]
            canvas_info: (Pos2::ZERO, 1.),
        }
    }
    fn checkpoint(&mut self) {
        if let Some(d) = self.edit_anchor.take()
            && d != self.doc
        {
            self.history.push(d);
            if self.history.len() > 40 {
                self.history.remove(0);
            }
            self.redo.clear();
        }
    }
    fn fit(&mut self) {
        self.zoom = 1.;
        self.pan = Vec2::ZERO;
        self.view_extent = self.doc.extent();
    }
    fn select(&mut self, n: usize) {
        self.checkpoint();
        self.selected = n;
    }
    fn request(&mut self, a: Action) {
        if matches!(a, Action::New | Action::Open) && self.doc != self.saved {
            self.pending_action = Some(a);
            self.show_discard = true;
        } else {
            self.act(a)
        }
    }
    fn act(&mut self, a: Action) {
        match a {
            Action::New => {
                self.checkpoint();
                self.history.push(self.doc.clone());
                self.redo.clear();
                self.doc = Document::default();
                self.path = None;
                self.saved = self.doc.clone();
                self.selected = 3;
                self.fit();
                self.dirty = true;
                self.status = "New circle created.".into();
            }
            Action::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Magic Circle Maker", &["mcm", "json"])
                    .pick_file()
                {
                    match Document::open(&path).and_then(|d| {
                        self.fonts.load_project(&d)?;
                        Ok(d)
                    }) {
                        Ok(d) => {
                            self.checkpoint();
                            self.history.push(self.doc.clone());
                            self.redo.clear();
                            self.doc = d;
                            self.saved = self.doc.clone();
                            self.path = Some(path);
                            self.selected = 0;
                            self.fit();
                            self.dirty = true;
                            self.status =
                                "Project opened. Embedded artwork and fonts restored.".into();
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            Action::Save | Action::SaveAs => {
                let path = if matches!(a, Action::Save) {
                    self.path.clone()
                } else {
                    None
                }
                .or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("Magic Circle Maker", &["mcm"])
                        .set_file_name(format!("{}.mcm", render::slug(&self.doc.name)))
                        .save_file()
                });
                if let Some(path) = path {
                    match self.doc.save(&path) {
                        Ok(()) => {
                            self.saved = self.doc.clone();
                            self.path = Some(path);
                            self.status =
                                "Project saved with embedded images and custom fonts.".into();
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            Action::Import => {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Artwork", &["png", "svg", "jpg", "jpeg", "webp"])
                    .pick_files()
                {
                    for p in paths {
                        self.import(p);
                    }
                }
            }
            Action::Font => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Font files", &["ttf", "otf", "ttc"])
                    .pick_file()
                {
                    if self.doc.custom_fonts.len() >= 30 {
                        self.error = Some(
                            "This beta supports up to 30 custom font files per project.".into(),
                        );
                        return;
                    }
                    match self.fonts.import(&path) {
                        Ok(f) => {
                            if let Some(l) = self.doc.layers.get_mut(self.selected)
                                && l.kind == "text"
                            {
                                l.font = f.name.clone();
                            }
                            if !self.doc.custom_fonts.iter().any(|old| old.data == f.data) {
                                self.doc.custom_fonts.push(f);
                            }
                            self.status="Font loaded from file and embedded. Check its license before sharing.".into();
                            self.dirty = true;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            Action::Generate => {
                self.doc = self.doc.regenerate(self.doc.seed);
                self.selected = self.selected.min(self.doc.layers.len().saturating_sub(1));
                self.fit();
                self.status =
                    "Generated from seed. Protected layers and imported assets preserved.".into();
            }
            Action::Undo => {
                self.checkpoint();
                if let Some(d) = self.history.pop() {
                    self.redo.push(self.doc.clone());
                    self.doc = d;
                    self.selected = self.selected.min(self.doc.layers.len().saturating_sub(1));
                    self.dirty = true;
                }
            }
            Action::Redo => {
                self.checkpoint();
                if let Some(d) = self.redo.pop() {
                    self.history.push(self.doc.clone());
                    self.doc = d;
                    self.selected = self.selected.min(self.doc.layers.len().saturating_sub(1));
                    self.dirty = true;
                }
            }
            Action::Duplicate => {
                if self.doc.layers.len() < 100
                    && let Some(l) = self.doc.layers.get(self.selected)
                {
                    let mut l = l.clone();
                    l.id = model::new_id();
                    l.name = format!("{} copy", l.name.chars().take(290).collect::<String>());
                    self.doc.layers.insert(self.selected, l);
                }
            }
            Action::Delete => {
                if self.selected < self.doc.layers.len() {
                    self.doc.layers.remove(self.selected);
                    self.selected = self.selected.min(self.doc.layers.len().saturating_sub(1));
                }
            }
            Action::Up => {
                if self.selected > 0 && self.selected < self.doc.layers.len() {
                    self.doc.layers.swap(self.selected, self.selected - 1);
                    self.selected -= 1;
                }
            }
            Action::Down => {
                if self.selected + 1 < self.doc.layers.len() {
                    self.doc.layers.swap(self.selected, self.selected + 1);
                    self.selected += 1;
                }
            }
            Action::Add(kind) => {
                if self.doc.layers.len() < 100 {
                    let mut l = Layer {
                        kind: kind.into(),
                        name: format!("New {kind}"),
                        ..Default::default()
                    };
                    if kind == "text" {
                        l.radius = 260.;
                        l.rotation = -90.;
                    }
                    self.doc.layers.insert(0, l);
                    self.selected = 0;
                }
            }
            Action::Export => self.show_export = true,
        }
    }
    fn import(&mut self, path: PathBuf) {
        if self.doc.layers.len() >= 100 {
            self.error = Some("This beta supports up to 100 layers.".into());
            return;
        }
        match render::import_asset(&path, &self.fonts) {
            Ok(l) => {
                let mut next = self.doc.clone();
                next.layers.insert(0, l);
                if let Err(e) = next.validate() {
                    self.error = Some(e);
                    return;
                }
                self.doc = next;
                self.selected = 0;
                self.dirty = true;
                self.status =
                    "Artwork imported as an editable layer. Original transparency preserved."
                        .into();
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn start_export(&mut self) {
        if self.worker.is_some() {
            return;
        }
        if self.export_kind == ExportKind::Selected && self.doc.layers.is_empty() {
            self.error = Some("Select a layer first.".into());
            return;
        }
        let (ext, label) = if self.export_kind == ExportKind::Layers {
            ("zip", "Layer PNG archive")
        } else {
            ("png", "Transparent PNG")
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(label, &[ext])
            .set_file_name(format!("{}.{}", render::slug(&self.doc.name), ext))
            .save_file()
        {
            let (d, fonts, kind, selected, size) = (
                self.doc.clone(),
                self.fonts.clone(),
                self.export_kind,
                self.selected,
                self.export_size,
            );
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = render::export(&path, &d, kind, selected, size, &fonts)
                    .map(|_| format!("Exported {}", path.display()));
                let _ = tx.send(result);
            });
            self.worker = Some(rx);
            self.show_export = false;
            self.status = "Rendering transparent PNG export…".into();
        }
    }
    fn header(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().fill(panel(self.dark)).inner_margin(14.))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("M").size(25.).color(gold(self.dark)));
                    ui.label(RichText::new("MAGIC CIRCLE MAKER").size(18.).strong());
                    ui.label(RichText::new("BETA 0.1").size(10.).color(gold(self.dark)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("About").clicked() {
                            self.show_about = true;
                        }
                        if ui
                            .button(if self.dark {
                                "Ivory theme"
                            } else {
                                "Obsidian theme"
                            })
                            .clicked()
                        {
                            self.dark = !self.dark;
                            theme(ctx, self.dark);
                        }
                        ui.label(if self.doc == self.saved {
                            "Saved"
                        } else {
                            "● Unsaved"
                        });
                    });
                });
                ui.add_space(10.);
                ui.horizontal(|ui| {
                    for (label, a) in [
                        ("New", Action::New),
                        ("Open…", Action::Open),
                        ("Save", Action::Save),
                        ("Save as…", Action::SaveAs),
                        ("Import artwork…", Action::Import),
                    ] {
                        if ui.button(label).clicked() {
                            actions.push(a);
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.history.is_empty() || self.edit_anchor.is_some(),
                            egui::Button::new("Undo"),
                        )
                        .clicked()
                    {
                        actions.push(Action::Undo);
                    }
                    if ui
                        .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        actions.push(Action::Redo);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                self.worker.is_none(),
                                egui::Button::new(RichText::new("Export PNGs…").color(
                                    if self.dark {
                                        Color32::BLACK
                                    } else {
                                        Color32::WHITE
                                    },
                                ))
                                .fill(gold(self.dark)),
                            )
                            .clicked()
                        {
                            actions.push(Action::Export);
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.doc.name)
                                .desired_width(190.)
                                .char_limit(300),
                        );
                    });
                });
            });
    }
    fn layers(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::SidePanel::left("layers").default_width(245.).min_width(215.).max_width(350.).frame(egui::Frame::new().fill(panel(self.dark)).inner_margin(16.)).show(ctx,|ui|{
   egui::ScrollArea::vertical().id_salt("left-panel").show(ui,|ui| {
 eyebrow(ui,"COMPOSITION",self.dark);
ui.horizontal(|ui|{
ui.heading("Layers");
ui.label(RichText::new(format!("{} / 100",self.doc.layers.len())).small().weak());
}
);
ui.label(RichText::new("Top layer appears in front").small().weak());
ui.add_space(10.);
   egui::ScrollArea::vertical().id_salt("layer-list").max_height((ui.available_height()-330.).max(150.)).show(ui,|ui|{
for i in 0..self.doc.layers.len(){
let selected=self.selected==i;
let name=self.doc.layers[i].name.clone();
let kind=self.doc.layers[i].kind.clone();
    egui::Frame::new().fill(if selected{
gold(self.dark).gamma_multiply(0.12)}
else{
Color32::TRANSPARENT}
).inner_margin(5.).corner_radius(5).show(ui,|ui|{
ui.horizontal(|ui|{
ui.checkbox(&mut self.doc.layers[i].visible,"").on_hover_text("Layer visibility");
if ui.selectable_label(selected,format!("{}  {}",kind_icon(&kind),name)).clicked(){
self.select(i);
}
}
);
ui.horizontal(|ui|{
ui.add_space(24.);
ui.label(RichText::new(kind.to_uppercase()).size(9.).weak());
ui.checkbox(&mut self.doc.layers[i].locked,"Protect").on_hover_text("Preserve this layer during generation. Manual editing remains available.");
}
);
}
);
ui.add_space(3.);
   }
}
);
ui.add_space(8.);
ui.horizontal(|ui|{
for(label,a)in[("↑",Action::Up),("↓",Action::Down),("Duplicate",Action::Duplicate),("Delete",Action::Delete)]{
if ui.add_enabled(!self.doc.layers.is_empty(),egui::Button::new(label)).clicked(){
actions.push(a);
}
}
}
);
   ui.menu_button("+ Add element",|ui|{
for kind in["rings","star","marks","rosette","symbols","text"]{
if ui.button(kind).clicked(){
actions.push(Action::Add(kind));
ui.close();
}
}
if ui.button("Import artwork…").clicked(){
actions.push(Action::Import);
ui.close();
}
}
);
   ui.add_space(16.);
ui.separator();
ui.add_space(12.);
eyebrow(ui,"PROCEDURAL GENERATOR",self.dark);
   egui::ComboBox::from_id_salt("preset").selected_text(preset_name(&self.doc.preset)).width(ui.available_width()-12.).show_ui(ui,|ui|{
for(p,n)in[("alchemy","Alchemy"),("geometry","Sacred geometry"),("sigil","Astral sigil")]{
ui.selectable_value(&mut self.doc.preset,p.into(),n);
}
}
);
   ui.add_space(8.);
ui.label("Seed");
ui.horizontal(|ui|{
ui.add(egui::DragValue::new(&mut self.doc.seed).speed(1));
if ui.button("Shuffle").clicked(){
self.doc.seed=model::new_seed();
actions.push(Action::Generate);
}
}
);
   ui.add(egui::Slider::new(&mut self.doc.symmetry,3..=24).text("Symmetry"));
ui.add(egui::Slider::new(&mut self.doc.complexity,1..=5).text("Complexity"));
   if ui.add_sized([ui.available_width(),32.],egui::Button::new("Generate from seed")).clicked(){
actions.push(Action::Generate);
}
ui.label(RichText::new("Protect layers to preserve them. Imported artwork stays in your composition.").small().weak());
 }
);
  }
);
    }
    fn inspector(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::SidePanel::right("inspector")
            .default_width(275.)
            .min_width(250.)
            .max_width(400.)
            .frame(egui::Frame::new().fill(panel(self.dark)).inner_margin(18.))
            .show(ctx, |ui| {
                eyebrow(ui, "ELEMENT INSPECTOR", self.dark);
                ui.add_space(8.);
                egui::ScrollArea::vertical()
                    .id_salt("inspector-scroll")
                    .show(ui, |ui| {
                        if let Some(l) = self.doc.layers.get_mut(self.selected) {
                            ui.add(
                                egui::TextEdit::singleline(&mut l.name)
                                    .font(egui::TextStyle::Heading)
                                    .desired_width(ui.available_width())
                                    .char_limit(300),
                            );
                            ui.label(RichText::new(format!("{} layer", l.kind)).small().weak());
                            ui.add_space(16.);
                            eyebrow(ui, "TRANSFORM", self.dark);
                            ui.add_space(8.);
                            egui::Grid::new("transform-grid")
                                .num_columns(2)
                                .spacing([14., 10.])
                                .show(ui, |ui| {
                                    ui.label("Position X");
                                    ui.add(
                                        egui::DragValue::new(&mut l.x)
                                            .range(-2000.0..=2000.0)
                                            .speed(0.5)
                                            .suffix(" px"),
                                    );
                                    ui.end_row();
                                    ui.label("Position Y");
                                    ui.add(
                                        egui::DragValue::new(&mut l.y)
                                            .range(-2000.0..=2000.0)
                                            .speed(0.5)
                                            .suffix(" px"),
                                    );
                                    ui.end_row();
                                    ui.label("Rotation");
                                    ui.add(
                                        egui::DragValue::new(&mut l.rotation)
                                            .range(-360.0..=360.0)
                                            .speed(0.5)
                                            .suffix("°"),
                                    );
                                    ui.end_row();
                                    ui.label("Scale");
                                    ui.add(
                                        egui::DragValue::new(&mut l.scale)
                                            .range(0.05..=5.0)
                                            .speed(0.01)
                                            .suffix("×"),
                                    );
                                    ui.end_row();
                                });
                            ui.add_space(7.);
                            if ui.button("Reset transform").clicked() {
                                l.x = 0.;
                                l.y = 0.;
                                l.rotation = 0.;
                                l.scale = 1.;
                            }
                            ui.add_space(15.);
                            ui.separator();
                            ui.add_space(12.);
                            eyebrow(ui, "GEOMETRY", self.dark);
                            ui.add_space(8.);
                            if l.kind != "asset" {
                                slider(ui, &mut l.radius, 0.0..=470.0, "Radius");
                            }
                            match l.kind.as_str() {
                                "rings" => {
                                    ui.add(egui::Slider::new(&mut l.count, 1..=12).text("Rings"));
                                    slider(ui, &mut l.gap, 1.0..=30.0, "Gap");
                                }
                                "star" => {
                                    ui.add(egui::Slider::new(&mut l.points, 3..=32).text("Points"));
                                    l.step = l.step.min(l.points - 1);
                                    ui.add(
                                        egui::Slider::new(&mut l.step, 1..=l.points - 1)
                                            .text("Step"),
                                    );
                                }
                                "marks" => {
                                    ui.add(egui::Slider::new(&mut l.count, 3..=240).text("Marks"));
                                    slider(ui, &mut l.length, 1.0..=30.0, "Length");
                                }
                                "rosette" => {
                                    ui.add(egui::Slider::new(&mut l.count, 2..=32).text("Petals"));
                                    slider(ui, &mut l.orbit, 0.0..=1.5, "Orbit");
                                }
                                "symbols" => {
                                    ui.add(
                                        egui::Slider::new(&mut l.count, 1..=32).text("Stations"),
                                    );
                                    slider(ui, &mut l.size, 6.0..=80.0, "Size");
                                    ui.add(egui::Slider::new(&mut l.symbol, 0..=6).text("Symbol"));
                                    egui::ComboBox::from_id_salt("orientation")
                                        .selected_text(&l.orientation)
                                        .show_ui(ui, |ui| {
                                            for s in ["upright", "radial", "tangent"] {
                                                ui.selectable_value(
                                                    &mut l.orientation,
                                                    s.into(),
                                                    s,
                                                );
                                            }
                                        });
                                }
                                "text" => {
                                    ui.label("Inscription");
                                    ui.add(
                                        egui::TextEdit::multiline(&mut l.text)
                                            .desired_width(ui.available_width())
                                            .desired_rows(3)
                                            .char_limit(1000),
                                    );
                                    ui.add_space(8.);
                                    ui.label("Font family · bundled + system");
                                    egui::ComboBox::from_id_salt("font-family")
                                        .selected_text(&l.font)
                                        .width(ui.available_width() - 10.)
                                        .height(250.)
                                        .show_ui(ui, |ui| {
                                            for name in &self.fonts.families {
                                                ui.selectable_value(
                                                    &mut l.font,
                                                    name.clone(),
                                                    name,
                                                );
                                            }
                                        });
                                    if ui.button("Load font file…").clicked() {
                                        actions.push(Action::Font);
                                    }
                                    slider(ui, &mut l.size, 6.0..=80.0, "Size");
                                    ui.add(egui::Slider::new(&mut l.repeat, 1..=8).text("Repeat"));
                                    ui.checkbox(&mut l.fit, "Fit around circle");
                                    if !l.fit {
                                        slider(ui, &mut l.tracking, -5.0..=30.0, "Tracking");
                                    }
                                    ui.checkbox(&mut l.reverse, "Reverse direction");
                                }
                                "asset" => {
                                    ui.label(&l.source_name);
                                    ui.label(
                                        RichText::new(format!(
                                            "{:.0} × {:.0} world px",
                                            l.asset_width, l.asset_height
                                        ))
                                        .small()
                                        .weak(),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "PNG stored inside your project. Use Scale to resize.",
                                        )
                                        .small()
                                        .weak(),
                                    );
                                }
                                _ => {}
                            }
                            ui.add_space(15.);
                            ui.separator();
                            ui.add_space(12.);
                            eyebrow(ui, "APPEARANCE", self.dark);
                            ui.add_space(8.);
                            if l.kind != "asset" {
                                slider(ui, &mut l.width, 0.1..=10.0, "Stroke");
                                let mut inherit = l.color == "auto";
                                if ui.checkbox(&mut inherit, "Use composition ink").changed() {
                                    l.color = if inherit {
                                        "auto".into()
                                    } else {
                                        self.doc.ink.clone()
                                    };
                                }
                                if !inherit {
                                    color_ui(ui, &mut l.color, "Element ink");
                                }
                            }
                            slider(ui, &mut l.opacity, 0.0..=1.0, "Opacity");
                            slider(ui, &mut l.glow, 0.0..=2.0, "Glow");
                        } else {
                            ui.label("Add an element to begin.");
                        }
                        ui.add_space(18.);
                        ui.separator();
                        ui.add_space(12.);
                        eyebrow(ui, "COMPOSITION INK", self.dark);
                        color_ui(ui, &mut self.doc.ink, "Global color");
                    });
            });
    }
    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(if self.dark {
                        Color32::from_rgb(17, 19, 24)
                    } else {
                        Color32::from_rgb(235, 232, 224)
                    })
                    .inner_margin(18.),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (tool, label) in [
                        (Tool::Move, "Move  V"),
                        (Tool::Rotate, "Rotate  R"),
                        (Tool::Scale, "Scale  S"),
                        (Tool::Pan, "Pan  H"),
                    ] {
                        let response = tool_button(ui, tool, label, self.tool == tool, self.dark);
                        #[cfg(test)]
                        {
                            self.tool_rects[tool as usize] = response.rect;
                        }
                        if response.clicked() {
                            self.tool = tool;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Fit").clicked() {
                            self.fit();
                        }
                        ui.label(format!("{:.0}%", self.zoom * 100.));
                    });
                });
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.guides, "Guides");
                    ui.checkbox(&mut self.checker, "Transparency grid");
                });
                ui.add_space(6.);
                let available = ui.available_size() - Vec2::new(0., 38.);
                let (rect, response) = ui
                    .allocate_exact_size(available.max(Vec2::splat(50.)), Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                let center = rect.center() + self.pan;
                let side = rect.width().min(rect.height()) - 40.;
                let factor = side * self.zoom / (self.view_extent * 2.);
                #[cfg(test)]
                {
                    self.canvas_info = (center, factor);
                }
                let screen = |p: [f32; 2]| center + Vec2::new(p[0], p[1]) * factor;
                if self.checker {
                    let size = 18.;
                    let mut y = rect.top();
                    let mut row = 0;
                    while y < rect.bottom() {
                        let mut x = rect.left();
                        let mut col = 0;
                        while x < rect.right() {
                            if (row + col) % 2 == 0 {
                                painter.rect_filled(
                                    Rect::from_min_size(Pos2::new(x, y), Vec2::splat(size)),
                                    0.,
                                    if self.dark {
                                        Color32::from_rgb(24, 26, 32)
                                    } else {
                                        Color32::from_rgb(225, 222, 215)
                                    },
                                );
                            }
                            x += size;
                            col += 1;
                        }
                        y += size;
                        row += 1;
                    }
                }
                if self.guides {
                    let stroke = Stroke::new(
                        1_f32,
                        if self.dark {
                            Color32::from_rgb(40, 43, 49)
                        } else {
                            Color32::from_rgb(207, 202, 190)
                        },
                    );
                    for n in [-400., -200., 0., 200., 400.] {
                        painter.line_segment([screen([n, -500.]), screen([n, 500.])], stroke);
                        painter.line_segment([screen([-500., n]), screen([500., n])], stroke);
                    }
                    painter.circle_stroke(center, 426. * factor, stroke);
                }
                if self.dirty || self.texture.is_none() {
                    match render::render(
                        &self.doc,
                        None,
                        if self.drag.is_some() { 700 } else { 1100 },
                        &self.fonts,
                    ) {
                        Ok(p) => {
                            let image = egui::ColorImage::from_rgba_premultiplied(
                                [p.width() as usize, p.height() as usize],
                                p.data(),
                            );
                            if let Some(t) = &mut self.texture {
                                t.set(image, egui::TextureOptions::LINEAR);
                            } else {
                                self.texture = Some(ctx.load_texture(
                                    "artwork",
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            self.dirty = false;
                        }
                        Err(e) => {
                            self.status = format!("Preview error: {e}");
                            self.dirty = false;
                        }
                    }
                }
                if let Some(t) = &self.texture {
                    let half = self.doc.extent() * factor;
                    let artwork = Rect::from_center_size(center, Vec2::splat(half * 2.));
                    painter.image(
                        t.id(),
                        artwork,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                        Color32::WHITE,
                    );
                }
                let accent = gold(self.dark);
                if let Some(l) = self.doc.layers.get(self.selected) {
                    let corners = l.corners().map(screen);
                    for n in 0..4 {
                        painter.line_segment(
                            [corners[n], corners[(n + 1) % 4]],
                            Stroke::new(1_f32, accent.gamma_multiply(0.65)),
                        );
                    }
                    let origin = screen([l.x, l.y]);
                    painter.circle_stroke(origin, 5., Stroke::new(1_f32, accent));
                    match self.tool {
                        Tool::Move => {
                            painter.line_segment(
                                [origin - Vec2::new(16., 0.), origin + Vec2::new(16., 0.)],
                                Stroke::new(2_f32, accent),
                            );
                            painter.line_segment(
                                [origin - Vec2::new(0., 16.), origin + Vec2::new(0., 16.)],
                                Stroke::new(2_f32, accent),
                            );
                        }
                        Tool::Rotate => {
                            let r = (l.bounds()[0].max(l.bounds()[1]) * l.scale * factor + 20.)
                                .max(35.);
                            painter.circle_stroke(origin, r, Stroke::new(1.5_f32, accent));
                            painter.circle_filled(origin + Vec2::new(0., -r), 6., accent);
                        }
                        Tool::Scale => {
                            for p in corners {
                                painter.rect_filled(
                                    Rect::from_center_size(p, Vec2::splat(9.)),
                                    1.,
                                    accent,
                                );
                            }
                        }
                        Tool::Pan => {}
                    }
                }
                if response.hovered() {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0. {
                        self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.2, 6.);
                    }
                }
                let panning = self.tool == Tool::Pan
                    || ctx.input(|i| i.key_down(Key::Space))
                    || response.dragged_by(PointerButton::Middle);
                if response.drag_started()
                    && let (Some(p), Some(l)) = (
                        response.interact_pointer_pos(),
                        self.doc.layers.get(self.selected),
                    )
                {
                    let origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or(p);
                    let w = (origin - center) / factor;
                    self.drag = Some(Drag {
                        layer: l.clone(),
                        start: [w.x, w.y],
                        tool: if panning { Tool::Pan } else { self.tool },
                    });
                }
                if response.dragged()
                    && let Some(drag) = &self.drag
                {
                    if drag.tool == Tool::Pan {
                        self.pan += response.drag_delta();
                    } else if let (Some(p), Some(l)) = (
                        response.interact_pointer_pos(),
                        self.doc.layers.get_mut(self.selected),
                    ) {
                        let w = (p - center) / factor;
                        let p = [w.x, w.y];
                        match drag.tool {
                            Tool::Move => {
                                l.x = (drag.layer.x + p[0] - drag.start[0]).clamp(-2000., 2000.);
                                l.y = (drag.layer.y + p[1] - drag.start[1]).clamp(-2000., 2000.);
                            }
                            Tool::Rotate => {
                                l.rotation = model::rotate_drag(
                                    &drag.layer,
                                    drag.start,
                                    p,
                                    ctx.input(|i| i.modifiers.shift),
                                )
                            }
                            Tool::Scale => l.scale = model::scale_drag(&drag.layer, drag.start, p),
                            _ => {}
                        }
                    }
                }
                if response.drag_stopped() {
                    self.drag = None;
                    self.dirty = true;
                }
                if response.double_clicked()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    let w = (p - center) / factor;
                    let point = [w.x, w.y];
                    if let Some((i, _)) = self
                        .doc
                        .layers
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.visible)
                        .filter(|(_, l)| {
                            let p = l.local(point);
                            let b = l.bounds();
                            p[0].abs() < b[0] && p[1].abs() < b[1]
                        })
                        .min_by(|(_, a), (_, b)| a.bounds()[0].total_cmp(&b.bounds()[0]))
                    {
                        self.select(i);
                    }
                }
                ui.add_space(10.);
                ui.label(
                    RichText::new(match self.tool {
                        Tool::Move => "Drag to move selected element · Space to pan",
                        Tool::Rotate => "Drag around center · Shift snaps to 15°",
                        Tool::Scale => "Drag toward or away from center to scale",
                        Tool::Pan => "Drag to pan · Scroll to zoom",
                    })
                    .small()
                    .weak(),
                );
            });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_export {
            let mut open = true;
            egui::Window::new("Export transparent artwork").open(&mut open).resizable(false).collapsible(false).default_width(420.).show(ctx,|ui|{
ui.label("Every PNG has an alpha channel. All layer exports share the same canvas size and origin.");
ui.add_space(12.);
ui.radio_value(&mut self.export_kind,ExportKind::Layers,"Each visible layer → PNGs in a ZIP");
ui.radio_value(&mut self.export_kind,ExportKind::Composite,"Composite → one transparent PNG");
ui.radio_value(&mut self.export_kind,ExportKind::Selected,"Selected layer → one transparent PNG");
ui.add_space(12.);
egui::ComboBox::from_id_salt("exportsize").selected_text(format!("{} × {} px",self.export_size,self.export_size)).show_ui(ui,|ui|{
for s in[1024,2048,4096]{
ui.selectable_value(&mut self.export_size,s,format!("{s} × {s} px"));
}
}
);
ui.add_space(10.);
ui.label(RichText::new("Layer ZIP includes manifest.json with stacking order. Opacity and glow are baked into each PNG.").small().weak());
ui.add_space(12.);
if ui.button("Choose destination and export…").clicked(){
self.start_export();
}
}
);
            self.show_export &= open;
        }
        if self.show_about {
            egui::Window::new("About Magic Circle Maker").open(&mut self.show_about).default_width(500.).show(ctx,|ui|{
ui.heading("Magic Circle Maker");
ui.label(concat!("Version ",env!("CARGO_PKG_VERSION")," · Rust desktop beta"));
ui.label("A procedural workbench for alchemical circles, geometric seals, and imaginary spells.");
ui.hyperlink_to("Source, releases, and issue reports","https://github.com/katfacey/magic-circle-maker");
ui.separator();
ui.label("Application code: MIT License. Bundled Cinzel, IM FELL English, and Inter: SIL Open Font License 1.1. The bundled editions allow commercial use and redistribution under that license. System and imported fonts retain their own licenses.");
egui::CollapsingHeader::new("Bundled font license (OFL 1.1)").show(ui,|ui|{
egui::ScrollArea::vertical().max_height(200.).show(ui,|ui|{
ui.label(include_str!("../assets/fonts/cinzel-OFL.txt"));
ui.label(include_str!("../assets/fonts/im-fell-english-OFL.txt"));
ui.label(include_str!("../assets/fonts/inter-OFL.txt"));
}
);
}
);
ui.separator();
ui.label("Created with assistance from OpenAI’s ChatGPT/Codex. This beta may contain bugs and is provided as-is.");
}
);
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Could not complete that action")
                .collapsible(false)
                .resizable(false)
                .default_width(420.)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }
        if self.show_discard {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Save your project before continuing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save…").clicked() {
                            self.act(Action::Save);
                            if self.doc == self.saved {
                                self.show_discard = false;
                                if self.closing {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                } else if let Some(a) = self.pending_action.take() {
                                    self.act(a);
                                }
                            }
                        }
                        if ui.button("Discard changes").clicked() {
                            self.show_discard = false;
                            if self.closing {
                                self.saved = self.doc.clone();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else if let Some(a) = self.pending_action.take() {
                                self.act(a);
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_discard = false;
                            self.closing = false;
                            self.pending_action = None;
                        }
                    });
                });
        }
        if self.recovery.is_some() {
            egui::Window::new("Recover your previous session?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("A recovery copy is available from your last editing session.");
                    ui.horizontal(|ui| {
                        if ui.button("Restore session").clicked()
                            && let Some(d) = self.recovery.take()
                        {
                            match self.fonts.load_project(&d) {
                                Ok(()) => {
                                    self.doc = d;
                                    self.fit();
                                    self.dirty = true;
                                    self.status =
                                        "Recovered session. Save it to a project file to keep it."
                                            .into();
                                }
                                Err(e) => self.error = Some(e),
                            }
                        }
                        if ui.button("Start fresh").clicked() {
                            self.recovery = None;
                        }
                    });
                });
        }
    }
}
impl eframe::App for MakerApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.frames += 1;
        let before = self.doc.clone();
        let mut actions = vec![];
        if let Some(rx) = &self.worker {
            match rx.try_recv() {
                Ok(result) => {
                    match result {
                        Ok(s) => self.status = s,
                        Err(e) => self.error = Some(e),
                    }
                    self.worker = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                    self.error = Some("The export worker stopped unexpectedly.".into());
                }
                _ => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        }
        let drops = ctx.input(|i| i.raw.dropped_files.clone());
        for f in drops {
            if let Some(path) = f.path {
                self.import(path);
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            actions.push(if ctx.input(|i| i.modifiers.shift) {
                Action::SaveAs
            } else {
                Action::Save
            });
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::O)) {
            actions.push(Action::Open);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Z)) {
            actions.push(if ctx.input(|i| i.modifiers.shift) {
                Action::Redo
            } else {
                Action::Undo
            });
        }
        if !ctx.wants_keyboard_input() {
            for (k, t) in [
                (Key::V, Tool::Move),
                (Key::R, Tool::Rotate),
                (Key::S, Tool::Scale),
                (Key::H, Tool::Pan),
            ] {
                if ctx.input(|i| i.key_pressed(k) && !i.modifiers.command) {
                    self.tool = t;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Delete)) {
                actions.push(Action::Delete);
            }
        }
        self.header(ctx, &mut actions);
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(panel(self.dark)).inner_margin(8.))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.worker.is_some() {
                        ui.spinner();
                    }
                    ui.label(RichText::new(&self.status).size(11.));
                });
            });
        self.layers(ctx, &mut actions);
        self.inspector(ctx, &mut actions);
        self.canvas(ctx);
        let history_action = actions
            .iter()
            .any(|a| matches!(a, Action::Undo | Action::Redo | Action::New | Action::Open));
        for a in actions {
            self.request(a);
        }
        self.dialogs(ctx);
        if self.doc != before {
            self.dirty = true;
            if !history_action && self.edit_anchor.is_none() {
                self.edit_anchor = Some(before);
            }
            ctx.request_repaint();
        }
        if !ctx.input(|i| i.pointer.any_down()) && !ctx.wants_keyboard_input() {
            self.checkpoint();
        }
        if self.last_autosave.elapsed() > Duration::from_secs(15)
            && self.doc != self.autosaved
            && self.recovery.is_none()
        {
            if let Some(path) = &self.autosave {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if self.doc.save(path).is_ok() {
                    self.autosaved = self.doc.clone();
                }
            }
            self.last_autosave = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs(5));
        if ctx.input(|i| i.viewport().close_requested())
            && self.doc != self.saved
            && self.smoke.is_none()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
            self.show_discard = true;
        }
        if let Some(path) = &self.smoke {
            if self.frames == 5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(image) = shot {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                let _ = image::save_buffer(
                    path,
                    &bytes,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.smoke_started.elapsed() > Duration::from_secs(30) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(p) = &self.autosave {
            let _ = std::fs::remove_file(p);
        }
    }
}
fn gold(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(216, 180, 106)
    } else {
        Color32::from_rgb(133, 94, 30)
    }
}
fn panel(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(24, 26, 32)
    } else {
        Color32::from_rgb(249, 247, 241)
    }
}
fn theme(ctx: &egui::Context, dark: bool) {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = panel(dark);
    v.window_fill = panel(dark);
    v.selection.bg_fill = gold(dark).gamma_multiply(0.25);
    v.selection.stroke = Stroke::new(1_f32, gold(dark));
    v.widgets.active.bg_fill = gold(dark).gamma_multiply(0.35);
    v.widgets.hovered.bg_fill = gold(dark).gamma_multiply(0.18);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(7., 7.);
        s.spacing.button_padding = Vec2::new(9., 6.);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(12.));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(19.));
    });
}
fn eyebrow(ui: &mut egui::Ui, s: &str, dark: bool) {
    ui.label(RichText::new(s).size(10.).color(gold(dark)).strong());
}
fn slider(ui: &mut egui::Ui, v: &mut f32, r: std::ops::RangeInclusive<f32>, name: &str) {
    ui.add(egui::Slider::new(v, r).text(name));
}
fn color_ui(ui: &mut egui::Ui, value: &mut String, name: &str) {
    let rgb = u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap_or(0xd8b46a);
    let mut c = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    ui.horizontal(|ui| {
        if ui.color_edit_button_srgb(&mut c).changed() {
            *value = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        }
        ui.label(name);
        ui.label(RichText::new(value.as_str()).small().weak());
    });
}
fn kind_icon(kind: &str) -> &str {
    match kind {
        "rings" => "O",
        "star" => "*",
        "marks" => "|",
        "text" => "T",
        "rosette" => "@",
        "symbols" => "<>",
        "asset" => "[]",
        _ => "·",
    }
}
fn tool_button(
    ui: &mut egui::Ui,
    tool: Tool,
    label: &str,
    selected: bool,
    dark: bool,
) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(Vec2::new(94., 30.), Sense::click());
    let p = ui.painter();
    let color = if selected {
        gold(dark)
    } else {
        ui.visuals().text_color()
    };
    if selected || response.hovered() {
        p.rect_filled(
            r,
            4.,
            gold(dark).gamma_multiply(if selected { 0.22 } else { 0.10 }),
        );
    }
    let c = Pos2::new(r.left() + 14., r.center().y);
    let s = Stroke::new(1.4_f32, color);
    match tool {
        Tool::Move => {
            p.line_segment([c - Vec2::new(7., 0.), c + Vec2::new(7., 0.)], s);
            p.line_segment([c - Vec2::new(0., 7.), c + Vec2::new(0., 7.)], s);
            for v in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                let tip = c + v * 7.;
                let side = Vec2::new(-v.y, v.x);
                p.line_segment([tip, tip - v * 3. + side * 2.], s);
                p.line_segment([tip, tip - v * 3. - side * 2.], s);
            }
        }
        Tool::Rotate => {
            let pts = (0..25)
                .map(|i| {
                    let a = (i as f32 / 24. * 300. - 90.).to_radians();
                    c + Vec2::angled(a) * 6.
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([c + Vec2::new(0., -6.), c + Vec2::new(3., -8.)], s);
            p.line_segment([c + Vec2::new(0., -6.), c + Vec2::new(3., -3.)], s);
        }
        Tool::Scale => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(11.)),
                0.,
                s,
                egui::StrokeKind::Middle,
            );
            p.line_segment([c - Vec2::splat(3.), c + Vec2::splat(3.)], s);
            for v in [-1., 1.] {
                p.rect_filled(
                    Rect::from_center_size(c + Vec2::splat(5.5 * v), Vec2::splat(4.)),
                    0.,
                    color,
                );
            }
        }
        Tool::Pan => {
            for i in 0..3 {
                p.line_segment(
                    [
                        c + Vec2::new(i as f32 * 3. - 3., -6.),
                        c + Vec2::new(i as f32 * 3. - 3., 4.),
                    ],
                    s,
                );
            }
            p.line_segment([c + Vec2::new(-6., 0.), c + Vec2::new(-3., 6.)], s);
            p.line_segment([c + Vec2::new(-3., 6.), c + Vec2::new(4., 6.)], s);
        }
    }
    p.text(
        Pos2::new(r.left() + 28., r.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.),
        color,
    );
    response.on_hover_text("Select a layer, choose this tool, then drag on the canvas")
}
fn preset_name(s: &str) -> &str {
    match s {
        "geometry" => "Sacred geometry",
        "sigil" => "Astral sigil",
        _ => "Alchemy",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut MakerApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500., 1000.))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            let before = app.doc.clone();
            let mut actions = vec![];
            app.header(ctx, &mut actions);
            app.layers(ctx, &mut actions);
            app.inspector(ctx, &mut actions);
            app.canvas(ctx);
            for a in actions {
                app.request(a);
            }
            if app.doc != before {
                app.dirty = true;
                if app.edit_anchor.is_none() {
                    app.edit_anchor = Some(before);
                }
            }
            if !ctx.input(|i| i.pointer.any_down()) {
                app.checkpoint();
            }
        });
    }
    fn mouse(p: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn gui_tools_transform_and_undo() {
        let ctx = egui::Context::default();
        let mut app = MakerApp::with_context(&ctx, Some("test".into()));
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        for tool in [Tool::Move, Tool::Rotate, Tool::Scale] {
            let button = app.tool_rects[tool as usize].center();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(button), mouse(button, true)],
            );
            frame(&mut app, &ctx, vec![mouse(button, false)]);
            assert!(app.tool == tool);
            let original = app.doc.clone();
            let (center, factor) = app.canvas_info;
            let start = center + Vec2::new(120., 0.) * factor;
            let end = center
                + match tool {
                    Tool::Move => Vec2::new(155., 20.),
                    Tool::Rotate => Vec2::new(0., 120.),
                    _ => Vec2::new(240., 0.),
                } * factor;
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), mouse(start, true)],
            );
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start + Vec2::new(6., 0.))],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            frame(&mut app, &ctx, vec![mouse(end, false)]);
            assert_ne!(app.doc, original);
            app.act(Action::Undo);
            assert_eq!(app.doc, original);
            app.act(Action::Redo);
            assert_ne!(app.doc, original);
            app.act(Action::Undo);
        }
    }
}
