use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub visible: bool,
    pub locked: bool,
    pub color: String,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub radius: f32,
    pub rotation: f32,
    pub width: f32,
    pub opacity: f32,
    pub glow: f32,
    pub count: u32,
    pub points: u32,
    pub step: u32,
    pub gap: f32,
    pub length: f32,
    pub size: f32,
    pub orbit: f32,
    pub symbol: u32,
    pub orientation: String,
    pub text: String,
    pub font: String,
    pub tracking: f32,
    pub repeat: u32,
    pub fit: bool,
    pub reverse: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub asset_data: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub asset_width: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub asset_height: f32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_name: String,
}
fn is_zero(n: &f32) -> bool {
    *n == 0.0
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: "New element".into(),
            kind: "rings".into(),
            visible: true,
            locked: false,
            color: "auto".into(),
            x: 0.,
            y: 0.,
            scale: 1.,
            radius: 150.,
            rotation: 0.,
            width: 1.7,
            opacity: 1.,
            glow: 0.,
            count: 3,
            points: 7,
            step: 3,
            gap: 6.,
            length: 10.,
            size: 24.,
            orbit: 0.72,
            symbol: 0,
            orientation: "upright".into(),
            text: "LUMEN · ORBIS · AETHER · ".into(),
            font: "Cinzel".into(),
            tracking: 2.,
            repeat: 2,
            fit: true,
            reverse: false,
            asset_data: String::new(),
            asset_width: 0.,
            asset_height: 0.,
            source_name: String::new(),
        }
    }
}
impl Layer {
    pub fn make(id: &str, name: &str, kind: &str, radius: f32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            radius,
            ..Default::default()
        }
    }
    pub fn bounds(&self) -> [f32; 2] {
        if self.kind == "asset" {
            return [self.asset_width / 2., self.asset_height / 2.];
        }
        let r = match self.kind.as_str() {
            "rosette" => self.radius * (1. + self.orbit),
            "symbols" => self.radius + self.size * 1.7,
            "text" => self.radius + self.size,
            _ => self.radius,
        };
        let r = (r + self.width / 2.).max(12.);
        [r, r]
    }
    pub fn world(&self, p: [f32; 2]) -> [f32; 2] {
        let a = self.rotation.to_radians();
        [
            self.x + self.scale * (p[0] * a.cos() - p[1] * a.sin()),
            self.y + self.scale * (p[0] * a.sin() + p[1] * a.cos()),
        ]
    }
    pub fn local(&self, p: [f32; 2]) -> [f32; 2] {
        let a = -self.rotation.to_radians();
        let (x, y) = (p[0] - self.x, p[1] - self.y);
        [
            (x * a.cos() - y * a.sin()) / self.scale,
            (x * a.sin() + y * a.cos()) / self.scale,
        ]
    }
    pub fn corners(&self) -> [[f32; 2]; 4] {
        let b = self.bounds();
        [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]].map(|p| {
            let p = [p[0] * b[0], p[1] * b[1]];
            if self.kind == "asset" {
                self.world(p)
            } else {
                [self.x + p[0] * self.scale, self.y + p[1] * self.scale]
            }
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedFont {
    pub name: String,
    pub data: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Document {
    pub version: u32,
    pub name: String,
    pub seed: u32,
    pub preset: String,
    pub symmetry: u32,
    pub complexity: u32,
    pub ink: String,
    pub layers: Vec<Layer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_fonts: Vec<EmbeddedFont>,
}
impl Default for Document {
    fn default() -> Self {
        generate(48271, "alchemy", 7, 3)
    }
}
impl Document {
    pub fn extent(&self) -> f32 {
        self.layers
            .iter()
            .map(|l| {
                l.corners()
                    .iter()
                    .flat_map(|p| [p[0].abs(), p[1].abs()])
                    .fold(0., f32::max)
                    + l.glow * 12. * l.scale
                    + 28.
            })
            .fold(500., f32::max)
    }
    pub fn regenerate(&self, seed: u32) -> Self {
        let mut next = generate(seed, &self.preset, self.symmetry, self.complexity);
        next.name = self.name.clone();
        next.ink = self.ink.clone();
        next.custom_fonts = self.custom_fonts.clone();
        for l in &mut next.layers {
            if let Some(old) = self.layers.iter().find(|o| o.id == l.id && o.locked) {
                *l = old.clone();
            }
        }
        for (i, l) in self.layers.iter().enumerate() {
            if (l.locked || l.kind == "asset") && !next.layers.iter().any(|n| n.id == l.id) {
                next.layers.insert(i.min(next.layers.len()), l.clone());
            }
        }
        next
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.layers.len() > 100
            || self.name.len() > 300
            || !valid_color(&self.ink)
            || !(3..=24).contains(&self.symmetry)
            || !(1..=5).contains(&self.complexity)
            || !["alchemy", "geometry", "sigil"].contains(&self.preset.as_str())
        {
            return Err("Unsupported or invalid project settings.".into());
        }
        let mut ids = HashSet::new();
        let mut assets = 0;
        for l in &self.layers {
            if !ids.insert(&l.id)
                || l.name.len() > 300
                || ![
                    "rings", "text", "marks", "star", "symbols", "rosette", "asset",
                ]
                .contains(&l.kind.as_str())
                || (l.color != "auto" && !valid_color(&l.color))
            {
                return Err("Invalid layer in project.".into());
            }
            for (v, min, max) in [
                (l.x, -2000., 2000.),
                (l.y, -2000., 2000.),
                (l.scale, 0.05, 5.),
                (l.radius, 0., 470.),
                (l.rotation, -360., 360.),
                (l.width, 0.1, 10.),
                (l.opacity, 0., 1.),
                (l.glow, 0., 2.),
                (l.size, 6., 80.),
                (l.orbit, 0., 1.5),
                (l.gap, 1., 30.),
                (l.length, 1., 30.),
                (l.tracking, -5., 30.),
            ] {
                if !v.is_finite() || v < min || v > max {
                    return Err(format!("Invalid geometry in {}.", l.name));
                }
            }
            if l.count < 1
                || l.count > 240
                || l.points < 3
                || l.points > 32
                || l.step < 1
                || l.step >= l.points
                || l.repeat < 1
                || l.repeat > 8
                || l.symbol > 6
                || l.text.len() > 4000
                || l.font.len() > 300
            {
                return Err("Invalid element parameters.".into());
            }
            if l.kind == "asset" {
                if !l.asset_data.starts_with("data:image/png;base64,")
                    || l.asset_data.len() > 16_000_000
                    || ![l.asset_width, l.asset_height]
                        .iter()
                        .all(|v| v.is_finite() && *v >= 1. && *v <= 1600.)
                {
                    return Err("Invalid imported artwork.".into());
                }
                assets += l.asset_data.len();
            }
        }
        if assets > 28_000_000
            || self.custom_fonts.len() > 30
            || self
                .custom_fonts
                .iter()
                .map(|f| f.data.len())
                .sum::<usize>()
                > 40_000_000
        {
            return Err("Project assets exceed the beta size limit.".into());
        }
        Ok(())
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 80_000_000 {
            return Err("Project is larger than 80 MB.".into());
        }
        let d: Self = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        d.validate()?;
        Ok(d)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let temp = path.with_extension("mcm.tmp");
        std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    }
}
pub fn valid_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!(
        "layer-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}
pub fn new_seed() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32
}
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64 / 4294967296.) as f32
    }
}
pub fn generate(seed: u32, preset: &str, symmetry: u32, complexity: u32) -> Document {
    let n = symmetry.clamp(3, 24);
    let c = complexity.clamp(1, 5);
    let mut rng = Rng(seed);
    let angle = (rng.next() * 12.).floor() * 5.;
    let mut boundary = Layer::make("boundary", "Outer boundary", "rings", 426.);
    boundary.width = 2.;
    let mut text = Layer::make("inscription", "Outer inscription", "text", 397.);
    text.text = "LUMEN · ORBIS · AETHER · VINCULUM · ".into();
    text.rotation = -90.;
    let mut marks = Layer::make("marks", "Radial marks", "marks", 364.);
    marks.count = (n * (8 + c * 2)).min(240);
    marks.length = 9. + (rng.next() * 5.).floor();
    marks.rotation = angle;
    let mut star = Layer::make(
        "star",
        &format!("{n}-point star"),
        "star",
        321. - (rng.next() * 15.).floor(),
    );
    star.points = n;
    star.step = ((n - 1) / 2).max(1);
    star.rotation = -90. + angle;
    star.width = 2.3;
    let mut symbols = Layer::make("stations", "Symbol stations", "symbols", 300.);
    symbols.count = n;
    symbols.size = 25.;
    symbols.rotation = -90. + angle;
    symbols.symbol = (rng.next() * 6.).floor() as u32;
    let mut inner = Layer::make(
        "inner",
        "Inner boundaries",
        "rings",
        145. + (rng.next() * 15.).floor(),
    );
    inner.count = 2 + c / 2;
    inner.gap = 8.;
    let mut rosette = Layer::make("rosette", "Petal rosette", "rosette", 72.);
    rosette.count = n;
    rosette.rotation = angle;
    rosette.width = 1.1;
    let mut seal = Layer::make("seal", "Center seal", "symbols", 0.);
    seal.count = 1;
    seal.size = 28.;
    seal.symbol = 2;
    if preset == "geometry" {
        text.text = "FORMA · ORBIS · HARMONIA · ".into();
        text.font = "IM FELL English".into();
        symbols.kind = "rosette".into();
        symbols.radius = 155.;
        symbols.count = 6;
        symbols.orbit = 1.;
        symbols.opacity = 0.45;
        symbols.width = 0.8;
    }
    if preset == "sigil" {
        text.text = "SIGILLUM · LUX · ASTRUM · ".into();
        star.step = 2;
        symbols.size = 35.;
    }
    let mut layers = vec![boundary, text, marks, star, symbols, inner, rosette, seal];
    if c >= 4 {
        let mut fine = Layer::make("chords", "Fine construction", "star", 269.);
        fine.points = (n * 2).min(32);
        fine.step = (n - 2).max(1);
        fine.rotation = angle;
        fine.width = 0.6;
        fine.opacity = 0.55;
        layers.insert(4, fine);
    }
    Document {
        version: 1,
        name: "Untitled circle".into(),
        seed,
        preset: preset.into(),
        symmetry: n,
        complexity: c,
        ink: "#d8b46a".into(),
        layers,
        custom_fonts: vec![],
    }
}
pub fn cycles(n: u32, step: u32) -> Vec<Vec<u32>> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for start in 0..n {
        if seen.contains(&start) {
            continue;
        }
        let mut cycle = vec![];
        let mut next = start;
        while seen.insert(next) {
            cycle.push(next);
            next = (next + step) % n;
        }
        out.push(cycle);
    }
    out
}
pub fn polar(a: f32, r: f32) -> [f32; 2] {
    [a.cos() * r, a.sin() * r]
}
pub fn rotate_drag(l: &Layer, start: [f32; 2], p: [f32; 2], snap: bool) -> f32 {
    let a = (start[1] - l.y).atan2(start[0] - l.x);
    let b = (p[1] - l.y).atan2(p[0] - l.x);
    let delta = (b - a).sin().atan2((b - a).cos());
    let angle = (l.rotation + delta.to_degrees() + 540.).rem_euclid(360.) - 180.;
    if snap {
        (angle / 15.).round() * 15.
    } else {
        (angle * 10.).round() / 10.
    }
}
pub fn scale_drag(l: &Layer, start: [f32; 2], p: [f32; 2]) -> f32 {
    let a = (start[0] - l.x).hypot(start[1] - l.y).max(1.);
    let b = (p[0] - l.x).hypot(p[1] - l.y);
    (l.scale * b / a).clamp(0.05, 5.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeded_generation_is_repeatable() {
        let a = generate(48271, "alchemy", 7, 3);
        assert_eq!(a, generate(48271, "alchemy", 7, 3));
        assert_ne!(a, generate(48272, "alchemy", 7, 3));
    }
    #[test]
    fn every_recipe_validates() {
        for p in ["alchemy", "geometry", "sigil"] {
            for n in [3, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24] {
                for c in 1..=5 {
                    generate(u32::MAX, p, n, c).validate().unwrap();
                }
            }
        }
    }
    #[test]
    fn compound_star_covers_every_vertex() {
        for n in 3..=32 {
            for s in 1..n {
                let all: Vec<_> = cycles(n, s).into_iter().flatten().collect();
                assert_eq!(all.len(), n as usize);
                assert_eq!(all.into_iter().collect::<HashSet<_>>().len(), n as usize)
            }
        }
    }
    #[test]
    fn protected_layers_survive_generation() {
        let mut d = Document::default();
        d.layers[1].locked = true;
        d.layers[1].text = "MY SPELL".into();
        assert_eq!(d.regenerate(35).layers[1], d.layers[1]);
    }
    #[test]
    fn transforms_invert() {
        let l = Layer {
            x: 20.,
            y: 30.,
            rotation: 90.,
            scale: 2.,
            ..Default::default()
        };
        let p = l.world([10., 0.]);
        assert!((p[0] - 20.).abs() < 0.001 && (p[1] - 50.).abs() < 0.001);
        let q = l.local(p);
        assert!((q[0] - 10.).abs() < 0.001 && q[1].abs() < 0.001);
    }
    #[test]
    fn legacy_projects_use_transform_defaults() {
        let mut value = serde_json::to_value(Document::default()).unwrap();
        let layer = &mut value["layers"][0];
        layer.as_object_mut().unwrap().remove("scale");
        let d: Document = serde_json::from_value(value).unwrap();
        assert_eq!(d.layers[0].scale, 1.);
    }
    #[test]
    fn invalid_geometry_is_rejected() {
        let mut d = Document::default();
        d.layers[0].scale = 0.;
        assert!(d.validate().is_err());
    }
    #[test]
    fn dragging_scales_and_wraps_rotation() {
        let l = Layer::default();
        assert_eq!(scale_drag(&l, [10., 0.], [20., 0.]), 2.);
        assert_eq!(rotate_drag(&l, [1., 0.], [0., 1.], false), 90.);
    }
}
