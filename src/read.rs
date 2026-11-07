//! A document's materials as Studio's (the crate docs say how).
use crate::DOCUMENT_SPACE;
use kinogaki_mtlx::{Bound, Document, Material, Type};
use kinogaki_render_api::material::Input as Port;
use kinogaki_render_api::material::mtlx::{def, value};
use kinogaki_studio_api::color::ColorService;
use kinogaki_studio_api::format::options;
use kinogaki_studio_api::geometry::image::{EmbeddedFile, ImageColor, ImageFile, ImageSource};
use kinogaki_studio_api::image_files::image_node;
use kinogaki_studio_api::network::{NetworkSeed, NewNode};
use kinogaki_studio_api::standard::{self, ImportedMaterial, Materials};
use kinogaki_studio_api::{
    Diagnostic, Error, FileItem, FormatOptions, Imported, Input, Reading, Value,
};
use std::path::Path;
use std::sync::Arc;

/// `input` read: one materials item, named after the file.
pub(crate) fn read(
    input: &Input<'_>,
    chosen: &FormatOptions,
    reading: &Reading<'_>,
) -> Result<Imported, Error> {
    reading.check()?;
    let bytes = input.bytes()?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::invalid(format!("{input}: not UTF-8 text")))?;
    let mut document: Document =
        (text.parse()).map_err(|why: String| Error::invalid(format!("{input}: {why}")))?;
    let folder = input.path().and_then(Path::parent);
    let mut said = document.include(&mut |href| match folder {
        Some(folder) => std::fs::read_to_string(folder.join(href)).map_err(|e| e.to_string()),
        None => Err("a document read from memory names no folder".into()),
    });
    reading.check()?;
    let (materials, left) = document.materials();
    said.extend(left);
    let mut read = Read {
        folder,
        embed: options::option(chosen, options::EMBED, Value::bool)?,
        colors: reading.colors,
        said,
    };
    let materials = materials.iter().map(|m| read.material(m)).collect();
    let name = Path::new(input.name())
        .file_stem()
        .map_or_else(|| "MaterialX".into(), |s| s.to_string_lossy());
    let held = Materials {
        materials,
        images: Vec::new(),
    };
    Ok(Imported {
        items: vec![FileItem::top(name, standard::materials().wrap(held))],
        warnings: read.said.into_iter().map(Diagnostic::warning).collect(),
    })
}

/// What a read holds while it reads.
struct Read<'a> {
    /// The document's folder, which its files' paths are relative to.
    folder: Option<&'a Path>,
    embed: bool,
    colors: &'a dyn ColorService,
    /// What was left out or approximated, in words.
    said: Vec<String>,
}

impl Read<'_> {
    /// `m` as a Material of Studio's.
    fn material(&mut self, m: &Material) -> ImportedMaterial {
        // A material node holding a surface alone shows the surface.
        let held = &m.nodes[m.material];
        let surface = match &held.inputs[..] {
            [(input, Bound::Output { node, .. })]
                if held.def.node == "surfacematerial" && input == "surfaceshader" =>
            {
                Some(*node)
            }
            _ => None,
        };
        let shown = surface.unwrap_or(m.material);
        let mut seed = NetworkSeed::default();
        let mut made = vec![None; m.nodes.len()];
        for (at, node) in m.nodes.iter().enumerate() {
            if surface.is_some() && at == m.material {
                continue;
            }
            let Some(new) = NewNode::materialx(&node.def.name) else {
                self.said.push(format!(
                    "{}: no node {} here, left out",
                    m.name, node.def.name
                ));
                continue;
            };
            let new = new.named(&node.name);
            made[at] = Some(seed.node(if at == shown { new.displayed() } else { new }));
        }
        let mut images: Vec<(ImageSource, usize)> = Vec::new();
        for (at, node) in m.nodes.iter().enumerate() {
            let Some(to) = made[at] else { continue };
            let ports = def(&node.def.name);
            for (input, bound) in &node.inputs {
                match bound {
                    Bound::Output { node, output } => {
                        if let Some(from) = made[*node] {
                            seed.wire(from, output, to, input);
                        }
                    }
                    Bound::Value {
                        value: v,
                        colorspace,
                    } => {
                        let ty = &node
                            .def
                            .input(input)
                            .expect("a bound input is its definition's")
                            .ty;
                        if *ty == Type::Filename {
                            let kinogaki_mtlx::Value::Text(file) = v else {
                                continue;
                            };
                            if file.is_empty() {
                                continue;
                            }
                            let source = self.source(file, colorspace.as_deref());
                            let image = match images.iter().find(|(s, _)| *s == source) {
                                Some((_, image)) => *image,
                                None => {
                                    let stem = Path::new(file).file_stem();
                                    let name = stem
                                        .map_or_else(|| "Image".into(), |s| s.to_string_lossy());
                                    let image = seed.node(image_node::reading(&source).named(name));
                                    images.push((source, image));
                                    image
                                }
                            };
                            seed.wire(image, image_node::IMAGE, to, input);
                            continue;
                        }
                        let port = ports
                            .as_ref()
                            .and_then(|d| d.find_input(input))
                            .and_then(Port::value);
                        let held = port.and_then(|p| value(&p.kind, v));
                        let Some(mut held) = held else {
                            let why = match port.is_some() {
                                true => "a value its input doesn't hold",
                                false => "a value where it takes a wire alone",
                            };
                            self.said.push(format!(
                                "{}: {}'s {input} holds {why}, left out",
                                m.name, node.name
                            ));
                            continue;
                        };
                        single(&mut held);
                        if ty.is_color() {
                            self.working(
                                &mut held,
                                colorspace.as_deref().unwrap_or(DOCUMENT_SPACE),
                            );
                        }
                        seed.nodes[to].parameters.insert(input.clone(), held);
                    }
                }
            }
        }
        ImportedMaterial {
            key: m.name.clone(),
            name: m.label.clone().unwrap_or_else(|| m.name.clone()),
            inside: seed,
            standard: None,
            double_sided: m.double_sided,
        }
    }

    /// What file `file` (relative to the document's folder) in color space
    /// `colorspace` reads.
    fn source(&mut self, file: &str, colorspace: Option<&str>) -> ImageSource {
        if file.contains("<UDIM>") || file.contains("<UVTILE>") {
            self.said
                .push(format!("{file}: a tile set, read as the one file it names"));
        }
        let path = match self.folder {
            Some(folder) if Path::new(file).is_relative() => folder.join(file),
            _ => file.into(),
        };
        let mut read = ImageFile::Path(Arc::from(path.display().to_string()));
        if self.embed {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let name =
                        (path.file_name()).map_or_else(|| file.into(), |n| n.to_string_lossy());
                    read = ImageFile::Embedded(EmbeddedFile::new(&name, bytes));
                }
                Err(e) => self
                    .said
                    .push(format!("{}: {e}, kept as a path", path.display())),
            }
        }
        let color = match colorspace {
            None => ImageColor::Auto,
            Some(raw) if raw.eq_ignore_ascii_case("raw") => ImageColor::Data,
            Some(space) => ImageColor::Space(Arc::from(space)),
        };
        ImageSource::new(read, color)
    }

    /// `held`, a color in `space`, in the working space.
    fn working(&mut self, held: &mut Value, space: &str) {
        let rgb = match held {
            Value::Color(c) => &mut c[..],
            Value::Numbers(n) if n.len() >= 3 => &mut n[..3],
            _ => return,
        };
        let mut three = [[rgb[0], rgb[1], rgb[2]]];
        match self.colors.to_working(space, &mut three) {
            Ok(()) => rgb.copy_from_slice(&three[0]),
            Err(e) => {
                let said = format!("colors in {space} kept as they are: {e}");
                if !self.said.contains(&said) {
                    self.said.push(said);
                }
            }
        }
    }
}

/// `v`'s numbers at MaterialX's precision, 32 bits: what a document holds.
fn single(v: &mut Value) {
    let round = |x: &mut f64| *x = f64::from(*x as f32);
    match v {
        Value::Number(x) => round(x),
        Value::Color(c) => c.iter_mut().for_each(round),
        Value::Numbers(n) => n.iter_mut().for_each(round),
        _ => {}
    }
}
