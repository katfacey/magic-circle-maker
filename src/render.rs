use crate::model::{self, Document, EmbeddedFont, Layer};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use resvg::{
    tiny_skia,
    usvg::{self, fontdb},
};
use std::{
    f32::consts::TAU,
    fmt::Write as _,
    io::{Cursor, Write},
    path::Path,
    sync::Arc,
};

#[derive(Clone)]
pub struct Fonts {
    pub db: Arc<fontdb::Database>,
    pub families: Vec<String>,
}
impl Fonts {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        for bytes in [
            include_bytes!("../assets/fonts/cinzel.ttf").as_slice(),
            include_bytes!("../assets/fonts/im-fell-english.ttf").as_slice(),
            include_bytes!("../assets/fonts/inter.ttf").as_slice(),
        ] {
            db.load_font_data(bytes.to_vec());
        }
        db.load_system_fonts();
        db.set_serif_family("Cinzel");
        db.set_sans_serif_family("Inter");
        let mut f = Self {
            db: Arc::new(db),
            families: vec![],
        };
        f.refresh();
        f
    }
    fn refresh(&mut self) {
        self.families = self
            .db
            .faces()
            .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()))
            .collect();
        self.families.sort_by_key(|s| s.to_lowercase());
        self.families.dedup();
    }
    pub fn import(&mut self, path: &Path) -> Result<EmbeddedFont, String> {
        if std::fs::metadata(path).map_err(err)?.len() > 12_000_000 {
            return Err("Font files must be under 12 MB.".into());
        }
        let bytes = std::fs::read(path).map_err(err)?;
        self.add_bytes(&bytes)?;
        let face = ttf_parser::Face::parse(&bytes, 0).map_err(err)?;
        let name = face
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::FAMILY && n.is_unicode())
            .and_then(|n| n.to_string())
            .unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        Ok(EmbeddedFont {
            name,
            data: B64.encode(bytes),
        })
    }
    fn add_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        ttf_parser::Face::parse(bytes, 0)
            .map_err(|_| "Use a valid TTF, OTF, or TTC font file.".to_string())?;
        Arc::make_mut(&mut self.db).load_font_data(bytes.to_vec());
        self.refresh();
        Ok(())
    }
    pub fn load_project(&mut self, d: &Document) -> Result<(), String> {
        for f in &d.custom_fonts {
            self.add_bytes(&B64.decode(&f.data).map_err(err)?)?;
        }
        Ok(())
    }
    fn id(&self, name: &str) -> Option<fontdb::ID> {
        self.db
            .query(&fontdb::Query {
                families: &[fontdb::Family::Name(name)],
                ..Default::default()
            })
            .or_else(|| {
                self.db
                    .faces()
                    .find(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
                    .map(|f| f.id)
            })
    }
    pub fn check(&self, d: &Document) -> Result<(), String> {
        for l in &d.layers {
            if l.visible && l.kind == "text" && self.id(&l.font).is_none() {
                return Err(format!(
                    "Font '{}' is unavailable. Install it or use Load font file in the inspector.",
                    l.font
                ));
            }
        }
        Ok(())
    }
    fn advances(&self, name: &str, chars: &[char], size: f32) -> Vec<f32> {
        self.id(name)
            .and_then(|id| {
                self.db.with_face_data(id, |data, index| {
                    ttf_parser::Face::parse(data, index).ok().map(|face| {
                        chars
                            .iter()
                            .map(|c| {
                                face.glyph_index(*c)
                                    .and_then(|g| face.glyph_hor_advance(g))
                                    .unwrap_or(face.units_per_em() / 2)
                                    as f32
                                    * size
                                    / face.units_per_em() as f32
                            })
                            .collect()
                    })
                })
            })
            .flatten()
            .unwrap_or_else(|| vec![size * 0.6; chars.len()])
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn circle(s: &mut String, r: f32, x: f32, y: f32) {
    let _ = write!(s, "<circle cx='{x}' cy='{y}' r='{}'/>", r.max(0.));
}
fn line(s: &mut String, a: [f32; 2], b: [f32; 2]) {
    let _ = write!(s, "<path d='M{} {}L{} {}'/>", a[0], a[1], b[0], b[1]);
}
fn symbol(s: &mut String, index: u32, r: f32) {
    match index % 7 {
        0 | 3 => {
            let _ = write!(
                s,
                "<path d='M0 {}L{} {}L{} {}Z'/>",
                -r,
                r * 0.87,
                r * 0.5,
                -r * 0.87,
                r * 0.5
            );
            if index % 7 == 3 {
                line(s, [-r * 0.58, 0.], [r * 0.58, 0.])
            }
        }
        1 => {
            let _ = write!(
                s,
                "<path d='M{} {}A{r} {r} 0 1 1 {} {}A{} {} 0 1 0 {} {}Z'/>",
                r * 0.588,
                r * 0.809,
                r * 0.588,
                -r * 0.809,
                r * 0.8,
                r * 0.8,
                r * 0.588,
                r * 0.809
            );
        }
        2 => {
            circle(s, r * 0.65, 0., 0.);
            line(s, [-r, 0.], [r, 0.]);
            line(s, [0., -r], [0., r]);
        }
        4 => {
            let _ = write!(
                s,
                "<path d='M0 {}L{} 0L0 {r}L{} 0Z'/>",
                -r,
                r * 0.65,
                -r * 0.65
            );
        }
        5 => {
            circle(s, r * 0.55, 0., -r * 0.2);
            line(s, [0., r * 0.35], [0., r]);
            line(s, [-r * 0.35, r * 0.65], [r * 0.35, r * 0.65]);
        }
        _ => {
            circle(s, r * 0.7, 0., 0.);
            line(s, [-r * 0.5, -r * 0.5], [r * 0.5, r * 0.5]);
            line(s, [r * 0.5, -r * 0.5], [-r * 0.5, r * 0.5]);
        }
    }
}
fn layer_svg(s: &mut String, l: &Layer, ink: &str, fonts: &Fonts, index: usize) {
    let color = if l.color == "auto" { ink } else { &l.color };
    if l.glow > 0. {
        let _ = write!(
            s,
            "<defs><filter id='g{index}' x='-100%' y='-100%' width='300%' height='300%'><feGaussianBlur stdDeviation='{}'/><feMerge><feMergeNode/><feMergeNode in='SourceGraphic'/></feMerge></filter></defs>",
            l.glow * 3.
        );
    }
    let filter = if l.glow > 0. {
        format!(" filter='url(#g{index})'")
    } else {
        String::new()
    };
    let _ = write!(
        s,
        "<g transform='translate({} {}) rotate({}) scale({})' opacity='{}' stroke='{}' fill='none' stroke-width='{}' stroke-linejoin='round'{filter}>",
        l.x, l.y, l.rotation, l.scale, l.opacity, color, l.width
    );
    match l.kind.as_str() {
        "rings" => {
            for i in 0..l.count {
                circle(s, l.radius - i as f32 * l.gap, 0., 0.)
            }
        }
        "star" => {
            for cycle in model::cycles(l.points, l.step) {
                s.push_str("<path d='");
                for (i, n) in cycle.iter().enumerate() {
                    let p = model::polar(*n as f32 / l.points as f32 * TAU, l.radius);
                    let _ = write!(s, "{}{} {}", if i == 0 { 'M' } else { 'L' }, p[0], p[1]);
                }
                s.push_str("Z'/>")
            }
        }
        "marks" => {
            for i in 0..l.count {
                let a = i as f32 / l.count as f32 * TAU;
                line(
                    s,
                    model::polar(a, l.radius),
                    model::polar(a, l.radius - l.length * if i % 5 == 0 { 1.65 } else { 1. }),
                );
            }
        }
        "rosette" => {
            for i in 0..l.count {
                let p = model::polar(i as f32 / l.count as f32 * TAU, l.radius * l.orbit);
                circle(s, l.radius, p[0], p[1]);
            }
        }
        "symbols" => {
            for i in 0..l.count {
                let a = i as f32 / l.count as f32 * TAU;
                let p = model::polar(a, l.radius);
                let angle = match l.orientation.as_str() {
                    "radial" => a.to_degrees(),
                    "tangent" => a.to_degrees() + 90.,
                    _ => 0.,
                };
                let _ = write!(
                    s,
                    "<g transform='translate({} {}) rotate({angle})'>",
                    p[0], p[1]
                );
                if l.count > 1 {
                    circle(s, l.size * 1.5, 0., 0.);
                    circle(s, l.size * 1.7, 0., 0.);
                }
                symbol(s, l.symbol + i, l.size * 0.77);
                s.push_str("</g>");
            }
        }
        "text" => {
            let chars: Vec<char> = l.text.repeat(l.repeat as usize).chars().collect();
            if !chars.is_empty() && l.radius > 0. {
                let widths = fonts.advances(&l.font, &chars, l.size);
                let space = if l.fit {
                    (TAU * l.radius - widths.iter().sum::<f32>()) / chars.len() as f32
                } else {
                    l.tracking
                };
                let mut pos = 0.;
                let dir = if l.reverse { -1. } else { 1. };
                for (c, w) in chars.iter().zip(widths) {
                    let angle = (pos + w / 2.) / l.radius * dir * 180. / std::f32::consts::PI;
                    let _ = write!(
                        s,
                        "<text transform='rotate({angle}) translate({} 0) rotate({})' font-family='{}' font-size='{}' text-anchor='middle' dominant-baseline='central' stroke='none' fill='{color}'>{}</text>",
                        l.radius,
                        dir * 90.,
                        esc(&l.font),
                        l.size,
                        esc(&c.to_string())
                    );
                    pos += w + space;
                }
            }
        }
        "asset" => {
            let _ = write!(
                s,
                "<image x='{}' y='{}' width='{}' height='{}' href='{}'/>",
                -l.asset_width / 2.,
                -l.asset_height / 2.,
                l.asset_width,
                l.asset_height,
                esc(&l.asset_data)
            );
        }
        _ => {}
    }
    s.push_str("</g>");
}
pub fn svg(d: &Document, selected: Option<usize>, fonts: &Fonts) -> String {
    let e = d.extent();
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1000' height='1000' viewBox='{} {} {} {}'>",
        -e,
        -e,
        e * 2.,
        e * 2.
    );
    for (i, l) in d.layers.iter().enumerate().rev() {
        if selected.map_or(l.visible, |n| n == i) {
            layer_svg(&mut s, l, &d.ink, fonts, i);
        }
    }
    s.push_str("</svg>");
    s
}
fn options(fonts: &Fonts) -> usvg::Options<'static> {
    let mut o = usvg::Options {
        fontdb: fonts.db.clone(),
        font_family: "Cinzel".into(),
        ..Default::default()
    };
    o.image_href_resolver.resolve_string = Box::new(|_, _| None);
    o
}
pub fn render(
    d: &Document,
    selected: Option<usize>,
    size: u32,
    fonts: &Fonts,
) -> Result<tiny_skia::Pixmap, String> {
    let tree = usvg::Tree::from_str(&svg(d, selected, fonts), &options(fonts)).map_err(err)?;
    let mut p = tiny_skia::Pixmap::new(size, size).ok_or("Unable to allocate image")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(size as f32 / 1000., size as f32 / 1000.),
        &mut p.as_mut(),
    );
    Ok(p)
}
pub fn import_asset(path: &Path, fonts: &Fonts) -> Result<Layer, String> {
    if std::fs::metadata(path).map_err(err)?.len() > 8_000_000 {
        return Err("Import an image smaller than 8 MB.".into());
    }
    let bytes = std::fs::read(path).map_err(err)?;
    let normalized = if path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .eq_ignore_ascii_case("svg")
    {
        let t = usvg::Tree::from_data(&bytes, &options(fonts)).map_err(err)?;
        let factor = (2048. / t.size().width().max(t.size().height())).min(1.);
        let w = (t.size().width() * factor).round().max(1.) as u32;
        let h = (t.size().height() * factor).round().max(1.) as u32;
        let mut p = tiny_skia::Pixmap::new(w, h).ok_or("Image too large")?;
        resvg::render(
            &t,
            tiny_skia::Transform::from_scale(factor, factor),
            &mut p.as_mut(),
        );
        p.encode_png().map_err(err)?
    } else {
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(err)?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(12000);
        limits.max_image_height = Some(12000);
        reader.limits(limits);
        let im = reader.decode().map_err(err)?.thumbnail(2048, 2048);
        let mut out = Cursor::new(Vec::new());
        im.write_to(&mut out, image::ImageFormat::Png)
            .map_err(err)?;
        out.into_inner()
    };
    if normalized.len() > 11_000_000 {
        return Err("The normalized image exceeds the asset size limit.".into());
    }
    let im = image::load_from_memory(&normalized).map_err(err)?;
    let factor = 300. / im.width().max(im.height()) as f32;
    Ok(Layer {
        kind: "asset".into(),
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(100)
            .collect(),
        source_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        asset_data: format!("data:image/png;base64,{}", B64.encode(normalized)),
        asset_width: im.width() as f32 * factor,
        asset_height: im.height() as f32 * factor,
        ..Default::default()
    })
}
#[derive(Clone, Copy, PartialEq)]
pub enum ExportKind {
    Layers,
    Composite,
    Selected,
}
pub fn export(
    path: &Path,
    d: &Document,
    kind: ExportKind,
    selected: usize,
    size: u32,
    fonts: &Fonts,
) -> Result<(), String> {
    d.validate()?;
    if kind == ExportKind::Selected {
        let mut test = d.clone();
        for (i, l) in test.layers.iter_mut().enumerate() {
            l.visible = i == selected;
        }
        fonts.check(&test)?;
    } else {
        fonts.check(d)?;
    }
    if ![1024, 2048, 4096].contains(&size) {
        return Err("Unsupported export resolution.".into());
    }
    let temp = path.with_extension("export.tmp");
    let result = (|| {
        if kind == ExportKind::Layers {
            let file = std::fs::File::create(&temp).map_err(err)?;
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let mut entries = vec![];
            for (i, l) in d.layers.iter().enumerate() {
                if !l.visible {
                    continue;
                }
                let name = format!("{:02}-{}.png", i + 1, slug(&l.name));
                zip.start_file(&name, opts).map_err(err)?;
                zip.write_all(&render(d, Some(i), size, fonts)?.encode_png().map_err(err)?)
                    .map_err(err)?;
                entries.push(serde_json::json!({"file":name,"layerId":l.id,"name":l.name,"stackIndex":i,"opacityBakedIn":true}));
            }
            let manifest = serde_json::json!({"format":"magic-circle-maker-layers","version":1,"canvas":{"width":size,"height":size,"worldExtent":d.extent(),"background":"transparent"},"stackOrder":"top-to-bottom","layers":entries});
            zip.start_file("manifest.json", opts).map_err(err)?;
            zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(err)?)
                .map_err(err)?;
            zip.finish().map_err(err)?;
        } else {
            let selected = if kind == ExportKind::Selected {
                Some(selected)
            } else {
                None
            };
            std::fs::write(
                &temp,
                render(d, selected, size, fonts)?
                    .encode_png()
                    .map_err(err)?,
            )
            .map_err(err)?;
        }
        std::fs::rename(&temp, path).map_err(err)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
pub fn slug(s: &str) -> String {
    let s: String = s
        .chars()
        .take(70)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-');
    if s.is_empty() {
        "layer".into()
    } else {
        s.into()
    }
}
pub fn self_test(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(err)?;
    let fonts = Fonts::new();
    let mut d = Document::default();
    d.save(&path.join("round-trip.mcm"))?;
    if Document::open(&path.join("round-trip.mcm"))? != d {
        return Err("Project round-trip differs".into());
    }
    export(
        &path.join("layers.zip"),
        &d,
        ExportKind::Layers,
        0,
        1024,
        &fonts,
    )?;
    export(
        &path.join("composite.png"),
        &d,
        ExportKind::Composite,
        0,
        2048,
        &fonts,
    )?;
    let im = image::open(path.join("composite.png"))
        .map_err(err)?
        .to_rgba8();
    if im.get_pixel(0, 0)[3] != 0 || !im.pixels().any(|p| p[3] > 0) {
        return Err("Transparency or rendering check failed".into());
    }
    let asset = import_asset(&path.join("composite.png"), &fonts)?;
    d.layers.insert(0, asset);
    d.layers[0].x = 220.;
    d.layers[0].rotation = 37.;
    d.layers[0].scale = 1.4;
    d.save(&path.join("asset-round-trip.mcm"))?;
    if Document::open(&path.join("asset-round-trip.mcm"))? != d {
        return Err("Asset round-trip differs".into());
    }
    export(
        &path.join("asset-selected.png"),
        &d,
        ExportKind::Selected,
        0,
        1024,
        &fonts,
    )?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path.join("layers.zip")).map_err(err)?)
        .map_err(err)?;
    if zip.len() != 9 {
        return Err("Layer archive is incomplete".into());
    }
    for i in 0..8 {
        let mut f = zip.by_index(i).map_err(err)?;
        let mut b = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut b).map_err(err)?;
        let p = image::load_from_memory(&b).map_err(err)?.to_rgba8();
        if p.width() != 1024
            || p.height() != 1024
            || p.get_pixel(0, 0)[3] != 0
            || !p.pixels().any(|p| p[3] > 0)
        {
            return Err("Layer PNG check failed".into());
        }
    }
    for preset in ["alchemy", "geometry", "sigil"] {
        let recipe = model::generate(12345, preset, 8, 5);
        export(
            &path.join(format!("{preset}.png")),
            &recipe,
            ExportKind::Composite,
            0,
            1024,
            &fonts,
        )?;
    }
    export(
        &path.join("4096.png"),
        &d,
        ExportKind::Composite,
        0,
        4096,
        &fonts,
    )?;
    let font_fixture = path.join("custom-font.ttf");
    std::fs::write(&font_fixture, include_bytes!("../assets/fonts/inter.ttf")).map_err(err)?;
    let custom = fonts.clone().import(&font_fixture)?;
    d.custom_fonts.push(custom);
    d.save(&path.join("font-round-trip.mcm"))?;
    let reopened = Document::open(&path.join("font-round-trip.mcm"))?;
    let mut restored = Fonts::new();
    restored.load_project(&reopened)?;
    restored.check(&reopened)?;
    let fixture = path.join("import.svg");
    std::fs::write(&fixture,"<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100'><circle cx='50' cy='50' r='35' fill='#d8b46a'/></svg>").map_err(err)?;
    let asset = import_asset(&fixture, &fonts)?;
    if asset.asset_width != 300. || !asset.asset_data.starts_with("data:image/png;base64,") {
        return Err("SVG import check failed".into());
    }
    std::fs::write(path.join("result.json"),"{\"passed\":true,\"checks\":[\"project round-trip\",\"embedded asset round-trip\",\"8 aligned transparent layer PNGs\",\"composite and selected PNG\",\"three recipe families\",\"4096-pixel export\",\"SVG import\"]}").map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_xml() {
        assert_eq!(esc("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;")
    }
    #[test]
    fn bundled_fonts_and_transparency() {
        let fonts = Fonts::new();
        assert!(fonts.id("Cinzel").is_some());
        assert!(fonts.id("IM FELL English").is_some());
        let d = Document::default();
        let p = render(&d, None, 256, &fonts).unwrap();
        assert_eq!(p.pixel(0, 0).unwrap().alpha(), 0);
        assert!(p.data().chunks(4).any(|p| p[3] > 0));
    }
    #[test]
    fn export_escapes_inscription() {
        let mut d = Document::default();
        d.layers[1].text = "<spell> & \"star\"".into();
        assert!(render(&d, None, 256, &Fonts::new()).is_ok());
    }
}
