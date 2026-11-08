//! MaterialX documents through the format: MaterialX's example materials
//! (1.39.5, Apache-2.0, `tests/fixtures/examples`) read, written and read
//! again as the same materials, and written as `tests/fixtures/written`
//! holds; included documents read; colors converted both ways; Image nodes
//! written as files and read back as the same images; lengths saying their
//! unit. A hand test has MaterialX itself validate what the format writes
//! (`tools/materialx_oracle.py`).
use kinogaki_materialx::MaterialX;
use kinogaki_render_api::MaterialNetwork;
use kinogaki_studio_api::color::ColorService;
use kinogaki_studio_api::geometry::EmbeddedFile;
use kinogaki_studio_api::geometry::image::{ImageColor, ImageFile, ImageSource};
use kinogaki_studio_api::image_files::{IMAGE_NODE, image_node, image_source};
use kinogaki_studio_api::network::{NetworkSeed, NewNode};
use kinogaki_studio_api::standard::{self, ImportedMaterial, Materials};
use kinogaki_studio_api::{
    Diagnostic, Error, FileFormat, FileItem, FormatOptions, Input, Purpose, Reading, Units, Value,
    Writing,
};
use kinogaki_studio_api_testing::Unconverted;
use kinogaki_studio_api_testing::format::Scratch;
use std::path::{Path, PathBuf};

/// The document at `path` read with `options` over the defaults, colors
/// through `colors`.
fn read_with(
    path: &Path,
    set: &[(&str, Value)],
    colors: &dyn ColorService,
) -> (Materials, Vec<Diagnostic>) {
    let mut options = MaterialX.options();
    options.extend(set.iter().map(|(k, v)| ((*k).to_owned(), v.clone())));
    let reading = Reading::new(Purpose::Objects, Units::METERS, colors);
    let read = MaterialX.read(&Input::file(path), &options, &reading);
    let read = read.unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let data = read.items[0].data.as_ref().expect("a value");
    (
        standard::materials().get(data).expect("materials").clone(),
        read.warnings,
    )
}

fn read(path: &Path) -> (Materials, Vec<Diagnostic>) {
    read_with(path, &[], &Unconverted)
}

/// `held` written at `path` from a project of `units`, colors through
/// `colors`: the writer's warnings.
fn write_with(
    path: &Path,
    held: &Materials,
    units: Units,
    colors: &dyn ColorService,
) -> Vec<Diagnostic> {
    let items = [FileItem::top(
        "Materials",
        standard::materials().wrap(held.clone()),
    )];
    let writing = Writing::new(units, colors);
    let writer = MaterialX.writer().expect("it writes");
    writer
        .write(path, &items, &writing, &FormatOptions::new())
        .unwrap()
}

/// Each material of `a` and `b` alike: key, name, sides, and its network
/// with every input at its value, the defaults' too.
fn same(a: &Materials, b: &Materials, what: &str) {
    assert_eq!(a.materials.len(), b.materials.len(), "{what}");
    for (x, y) in a.materials.iter().zip(&b.materials) {
        assert_eq!(
            (&x.key, &x.name, x.double_sided),
            (&y.key, &y.name, y.double_sided),
            "{what}"
        );
        let network = |m: &ImportedMaterial| {
            MaterialNetwork::try_from(&m.inside)
                .unwrap_or_else(|e| panic!("{what} {}: {e}", m.name))
        };
        assert_eq!(network(x), network(y), "{what}: {}", x.name);
    }
}

/// Every example document, by its path in the fixtures.
fn examples() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/examples");
    let mut found = Vec::new();
    for family in std::fs::read_dir(&root).unwrap() {
        let family = family.unwrap().path();
        if family.is_dir() {
            for file in std::fs::read_dir(&family).unwrap() {
                found.push(file.unwrap().path());
            }
        }
    }
    found.sort();
    found
}

#[test]
fn the_examples_write_back_as_the_same_materials() {
    let written = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/written");
    let refresh = std::env::var("MTLX_WRITTEN").as_deref() == Ok("write");
    let mut count = 0;
    for example in examples() {
        let name = example.file_name().unwrap().to_owned();
        // Read and written in one folder, so the files they name are named alike.
        let dir = Scratch::new(&format!("example-{}", name.to_string_lossy()));
        let family = example.parent().unwrap();
        for file in std::fs::read_dir(family).unwrap() {
            let file = file.unwrap().path();
            std::fs::copy(&file, dir.join(file.file_name().unwrap())).unwrap();
        }
        // MaterialX's repository keeps the examples' pictures three folders
        // up, outside the folder of the files naming them.
        let (held, said) = read(&dir.join(&name));
        // What MaterialX's examples hold that a Material doesn't.
        let expected = match &*name.to_string_lossy() {
            "standard_surface_chess_set.mtlx"
            | "standard_surface_look_brass_tiled.mtlx"
            | "standard_surface_look_wood_tiled.mtlx" => vec!["looks"],
            // A constant where the shading normal is read.
            "gltf_pbr_default.mtlx" => vec!["normal holds", "clearcoat_normal holds"],
            _ => vec![],
        };
        let matched = said
            .iter()
            .zip(&expected)
            .all(|(w, e)| w.message.contains(e));
        assert!(
            said.len() == expected.len() && matched,
            "{name:?}: {said:?}"
        );
        assert!(!held.materials.is_empty(), "{name:?}");
        let copy = dir.join("written.mtlx");
        let said = write_with(&copy, &held, Units::METERS, &Unconverted);
        assert!(said.is_empty(), "{name:?}: {said:?}");
        let (again, _) = read(&copy);
        same(&held, &again, &name.to_string_lossy());
        let text = std::fs::read_to_string(&copy).unwrap();
        let expected = written.join(&name);
        if refresh {
            std::fs::create_dir_all(&written).unwrap();
            std::fs::write(&expected, &text).unwrap();
        }
        let expected = std::fs::read_to_string(&expected).unwrap_or_default();
        assert_eq!(
            text, expected,
            "{name:?}: MTLX_WRITTEN=write writes it again"
        );
        count += 1;
    }
    assert_eq!(count, 50);
}

#[test]
fn an_included_document_s_materials_and_definitions_read() {
    let dir = Scratch::new("include");
    let defs = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_tinted" node="tinted">
    <input name="tint" type="color3" value="1, 0, 0" />
    <output name="out" type="surfaceshader" />
  </nodedef>
  <nodegraph name="NG_tinted" nodedef="ND_tinted">
    <standard_surface name="surface" type="surfaceshader">
      <input name="base_color" type="color3" interfacename="tint" />
    </standard_surface>
    <output name="out" type="surfaceshader" nodename="surface" />
  </nodegraph>
</materialx>
"#;
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("lib/defs.mtlx"), defs).unwrap();
    let document = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <xi:include href="lib/defs.mtlx" />
  <tinted name="green" type="surfaceshader">
    <input name="tint" type="color3" value="0, 1, 0" />
  </tinted>
  <surfacematerial name="Green" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="green" />
  </surfacematerial>
</materialx>
"#;
    std::fs::write(dir.join("look.mtlx"), document).unwrap();
    let (held, said) = read(&dir.join("look.mtlx"));
    assert!(said.is_empty(), "{said:?}");
    // The document's own definition is its graph, flattened.
    let [green] = &held.materials[..] else {
        panic!("one material")
    };
    let [surface] = &green.inside.nodes[..] else {
        panic!("one node: {:?}", green.inside.nodes)
    };
    assert!(surface.display, "a surface alone shows");
    assert_eq!(
        surface.parameters["base_color"],
        Value::Color([0.0, 1.0, 0.0])
    );
    let missing = "<?xml version=\"1.0\"?>\n<materialx version=\"1.39\">\n  <xi:include href=\"gone.mtlx\" />\n</materialx>\n";
    std::fs::write(dir.join("missing.mtlx"), missing).unwrap();
    let (_, said) = read(&dir.join("missing.mtlx"));
    assert!(
        said.iter().any(|w| w.message.contains("gone.mtlx")),
        "{said:?}"
    );
}

/// A color service whose working space is twice `lin_rec709`, by name.
#[derive(Debug)]
struct Doubled;
impl ColorService for Doubled {
    fn working(&self) -> &str {
        "doubled"
    }
    fn convert(&self, from: &str, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
        let k = match (from, to) {
            ("lin_rec709", "doubled") => 2.0,
            ("doubled", "lin_rec709") => 0.5,
            (a, b) if a == b => 1.0,
            _ => return Err(Error::invalid(format!("no {from} here"))),
        };
        rgb.iter_mut().flatten().for_each(|c| *c *= k);
        Ok(())
    }
    fn revision(&self) -> u64 {
        0
    }
}

#[test]
fn colors_read_into_the_working_space_and_write_in_the_document_s() {
    let dir = Scratch::new("colors");
    let document = r#"<?xml version="1.0"?>
<materialx version="1.39" colorspace="lin_rec709">
  <open_pbr_surface name="surface" type="surfaceshader">
    <input name="base_color" type="color3" value="0.25, 0.125, 0.5" />
    <input name="base_weight" type="float" value="0.5" />
  </open_pbr_surface>
  <surfacematerial name="Paint" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="surface" />
  </surfacematerial>
</materialx>
"#;
    std::fs::write(dir.join("paint.mtlx"), document).unwrap();
    let (held, _) = read_with(&dir.join("paint.mtlx"), &[], &Doubled);
    let surface = &held.materials[0].inside.nodes[0];
    assert_eq!(
        surface.parameters["base_color"],
        Value::Color([0.5, 0.25, 1.0])
    );
    assert_eq!(
        surface.parameters["base_weight"],
        Value::Number(0.5),
        "a float isn't a color"
    );
    write_with(&dir.join("again.mtlx"), &held, Units::METERS, &Doubled);
    let text = std::fs::read_to_string(dir.join("again.mtlx")).unwrap();
    assert!(text.contains("value=\"0.25, 0.125, 0.5\""), "{text}");
}

/// A single-sided material whose Image nodes read a file (`dir`'s
/// `maps/wood.png`, made here, in `srgb_texture`), an embedded file (data)
/// and a result.
fn oiled_wood(dir: &Path) -> Materials {
    std::fs::create_dir_all(dir.join("maps")).unwrap();
    let wood = dir.join("maps/wood.png");
    std::fs::write(&wood, [1, 2, 3]).unwrap();
    let mut inside = NetworkSeed::default();
    let surface = inside.node(
        NewNode::materialx("ND_open_pbr_surface_surfaceshader")
            .unwrap()
            .displayed(),
    );
    let base = inside.node(NewNode::materialx("ND_image_color3").unwrap().named("base"));
    let rough = inside.node(NewNode::materialx("ND_image_float").unwrap().named("rough"));
    let glow = inside.node(NewNode::materialx("ND_image_color3").unwrap().named("glow"));
    inside.wire(base, "out", surface, "base_color");
    inside.wire(rough, "out", surface, "specular_roughness");
    inside.wire(glow, "out", surface, "emission_color");
    let srgb = ImageColor::Space("srgb_texture".into());
    let file = ImageSource::new(ImageFile::Path(wood.display().to_string().into()), srgb);
    let packed = EmbeddedFile::new("rough.png", vec![4, 5, 6]);
    let held = ImageSource::new(ImageFile::Embedded(packed), ImageColor::Data);
    let images = [(&file, base), (&held, rough)];
    for (source, into) in images {
        let image = inside.node(image_node::reading(source));
        inside.wire(image, image_node::IMAGE, into, "file");
    }
    let result =
        NewNode::new(IMAGE_NODE).with(image_node::SOURCE, Value::Choice(image_node::RESULT.into()));
    let result = inside.node(result);
    inside.wire(result, image_node::IMAGE, glow, "file");
    let m = ImportedMaterial {
        key: "/Looks/Wood".into(),
        name: "Wood, Oiled".into(),
        inside,
        standard: None,
        double_sided: false,
    };
    Materials { materials: vec![m] }
}

/// Wave 4.5: a material's Image nodes reading a file, an embedded file and
/// a result: the file named by its path with its color space, the embedded
/// one written beside the document, the result left out and said; read
/// back, the same images.
#[test]
fn image_nodes_write_as_files_beside_the_document_and_read_back() {
    let dir = Scratch::new("images");
    let wood = dir.join("maps/wood.png");
    let path = dir.join("wood.mtlx");
    let said = write_with(&path, &oiled_wood(&dir), Units::METERS, &Unconverted);
    assert!(
        said.iter().any(|w| w.message.contains("reading a result")),
        "{said:?}"
    );
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("value=\"maps/wood.png\" colorspace=\"srgb_texture\""),
        "{text}"
    );
    assert!(
        text.contains("value=\"wood_textures/rough.png\" colorspace=\"none\""),
        "{text}"
    );
    assert!(
        text.contains("uiname=\"Wood, Oiled\"") && text.contains("kinogaki:doublesided=\"false\""),
        "{text}"
    );
    assert_eq!(
        std::fs::read(dir.join("wood_textures/rough.png")).unwrap(),
        [4, 5, 6]
    );
    for embed in [false, true] {
        let (back, _) = read_with(&path, &[("embed", Value::Bool(embed))], &Unconverted);
        let m = &back.materials[0];
        assert_eq!((m.name.as_str(), m.double_sided), ("Wood, Oiled", false));
        let read: Vec<ImageSource> = (m.inside.nodes.iter())
            .filter(|n| n.node_type == IMAGE_NODE)
            .map(|n| image_source(&n.parameters).unwrap().unwrap())
            .collect();
        let [a, b] = &read[..] else {
            panic!("two images: {read:?}")
        };
        let bytes = |s: &ImageSource| match s.file() {
            ImageFile::Path(p) => std::fs::read(&**p).unwrap(),
            ImageFile::Embedded(e) => e.bytes().to_vec(),
        };
        let srgb = ImageColor::Space("srgb_texture".into());
        assert_eq!((a.color(), bytes(a)), (&srgb, vec![1, 2, 3]));
        assert_eq!((b.color(), bytes(b)), (&ImageColor::Data, vec![4, 5, 6]));
        assert_eq!(matches!(b.file(), ImageFile::Embedded(_)), embed);
        let embedded = match a.file() {
            ImageFile::Path(p) => Path::new(&**p) == wood.canonicalize().unwrap(),
            ImageFile::Embedded(e) => e.hash() == EmbeddedFile::new("x", vec![1, 2, 3]).hash(),
        };
        assert!(embedded, "{a:?}");
    }
}

/// The document at `path` read into a project of `units`: its materials.
fn read_in(path: &Path, units: Units) -> Materials {
    let reading = Reading::new(Purpose::Objects, units, &Unconverted);
    let read = MaterialX.read(&Input::file(path), &MaterialX.options(), &reading);
    let read = read.unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let data = read.items[0].data.as_ref().expect("a value");
    standard::materials().get(data).expect("materials").clone()
}

/// The value of input `input` of the node of MaterialX node `node` in
/// `held`'s first material.
fn held(held: &Materials, node: &str, input: &str) -> Value {
    let seed = &held.materials[0].inside;
    let found = (seed.nodes.iter()).find(|n| {
        let signature = n.parameters.get("signature").and_then(Value::choice);
        signature.is_some_and(|s| {
            kinogaki_mtlx::library()
                .def(s)
                .is_some_and(|d| d.node == node)
        })
    });
    found
        .and_then(|n| n.parameters.get(input))
        .cloned()
        .unwrap()
}

/// OpenPBR's subsurface radius held, 2, and its transmission depth a glTF
/// texture's factor drives, 3: lengths, in the project's unit.
fn glass() -> Materials {
    let mut inside = NetworkSeed::default();
    let surface = inside.node(
        NewNode::materialx("ND_open_pbr_surface_surfaceshader")
            .unwrap()
            .with("subsurface_radius", 2.0)
            .displayed(),
    );
    let depth = inside.node(
        NewNode::materialx("ND_gltf_image_float_float_1_0")
            .unwrap()
            .with("factor", 3.0),
    );
    inside.wire(depth, "out", surface, "transmission_depth");
    let glass = ImportedMaterial {
        key: "Glass".into(),
        name: "Glass".into(),
        inside,
        standard: None,
        double_sided: true,
    };
    Materials {
        materials: vec![glass],
    }
}

#[test]
fn lengths_say_their_unit_and_read_into_the_projects() {
    let dir = Scratch::new("lengths");
    let millimetres = Units::new(0.001, kinogaki_studio_api::UpAxis::Y).unwrap();
    let centimetres = Units::new(0.01, kinogaki_studio_api::UpAxis::Y).unwrap();
    let path = dir.join("glass.mtlx");
    write_with(&path, &glass(), millimetres, &Unconverted);
    let text = std::fs::read_to_string(&path).unwrap();
    for said in [
        r#"name="subsurface_radius" type="float" value="2" unittype="distance" unit="millimeter""#,
        r#"name="transmission_depth" type="float" nodename="#,
    ] {
        assert!(text.contains(said), "{said} in\n{text}");
    }
    // Read into centimetres: a tenth, the texture's factor too.
    let back = read_in(&path, centimetres);
    assert_eq!(
        held(&back, "open_pbr_surface", "subsurface_radius"),
        Value::Number(0.2)
    );
    let Value::Number(factor) = held(&back, "gltf_image", "factor") else {
        panic!("a factor");
    };
    assert!((factor - 0.3).abs() < 1e-9, "{factor}");
    // Read into millimetres: as written.
    let back = read_in(&path, millimetres);
    assert_eq!(
        held(&back, "open_pbr_surface", "subsurface_radius"),
        Value::Number(2.0)
    );
}

#[test]
fn a_unit_on_a_node_holds_for_its_lengths_and_none_leaves_them_as_read() {
    let dir = Scratch::new("node-unit");
    let document = |unit: &str| {
        format!(
            r#"<?xml version="1.0"?>
<materialx version="1.39">
  <gltf_pbr name="glass" type="surfaceshader"{unit}>
    <input name="thickness" type="float" value="2" />
    <input name="ior" type="float" value="1.4" />
  </gltf_pbr>
  <surfacematerial name="Glass" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="glass" />
  </surfacematerial>
</materialx>
"#
        )
    };
    let millimetres = Units::new(0.001, kinogaki_studio_api::UpAxis::Y).unwrap();
    let path = dir.join("said.mtlx");
    std::fs::write(&path, document(r#" unittype="distance" unit="centimeter""#)).unwrap();
    let back = read_in(&path, millimetres);
    assert_eq!(held(&back, "gltf_pbr", "thickness"), Value::Number(20.0));
    let Value::Number(ior) = held(&back, "gltf_pbr", "ior") else {
        panic!("an index");
    };
    assert!((ior - 1.4).abs() < 1e-6, "no length: {ior}");
    std::fs::write(&path, document("")).unwrap();
    let back = read_in(&path, millimetres);
    assert_eq!(held(&back, "gltf_pbr", "thickness"), Value::Number(2.0));
}

/// Materials of Kinogaki's own definitions, which a document using them
/// carries: a Phong Surface reading a glTF Sampler Image with its
/// minification filter, and a Surface Unlit Cutout.
fn kinogaki_definitions() -> Materials {
    let mut phong = NetworkSeed::default();
    let surface = phong.node(
        NewNode::materialx("ND_kinogaki_phong_surface_surfaceshader")
            .unwrap()
            .with("shininess", 40.0)
            .displayed(),
    );
    let sampler = phong.node(
        NewNode::materialx("ND_kinogaki_gltf_image_color3")
            .unwrap()
            .with("minfilter", Value::Choice("nearest_mipmap_linear".into())),
    );
    phong.wire(sampler, "out", surface, "diffuse_color");
    let mut cutout = NetworkSeed::default();
    cutout.node(
        NewNode::materialx("ND_kinogaki_surface_unlit")
            .unwrap()
            .with("alpha_mode", Value::Integer(1))
            .displayed(),
    );
    let material = |name: &str, inside| ImportedMaterial {
        key: name.into(),
        name: name.into(),
        inside,
        standard: None,
        double_sided: true,
    };
    Materials {
        materials: vec![material("Phong", phong), material("Cutout", cutout)],
    }
}

/// `tools/materialx_oracle.py` run by `python` over the documents in
/// `folders`, MaterialX's examples beside them: whether it passed them, and
/// what it said.
fn oracle(python: &std::ffi::OsStr, folders: &[&Path]) -> (bool, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let run = std::process::Command::new(python)
        .arg("-I")
        .arg(root.join("tools/materialx_oracle.py"))
        .arg(root.join("tests/fixtures/examples"))
        .args(folders)
        .output()
        .expect("MaterialX's Python starts");
    let said = String::from_utf8_lossy(&run.stdout);
    let said = format!("{said}{}", String::from_utf8_lossy(&run.stderr));
    println!("{said}");
    (run.status.success(), said)
}

/// MaterialX's own reading of what the format writes:
/// `tools/materialx_oracle.py`, run by MaterialX 1.39.5's Python
/// (`MATERIALX_PYTHON`, which Studio's `tools/fetch_data.py` makes),
/// validates every written fixture and compares it with its example, and
/// validates the documents the round trips write, written here: Image
/// nodes, lengths in millimetres and Kinogaki's definitions. It refuses a
/// document MaterialX refuses, so its passes say something.
#[test]
#[ignore = "hand: against MaterialX's own Python module in MATERIALX_PYTHON"]
fn hand_run_materialx_validates_every_document_the_format_writes() {
    let python = std::env::var_os("MATERIALX_PYTHON")
        .expect("MATERIALX_PYTHON: MaterialX's Python (Studio's tools/fetch_data.py)");
    let dir = Scratch::new("materialx");
    let millimetres = Units::new(0.001, kinogaki_studio_api::UpAxis::Y).unwrap();
    let documents = [
        ("wood", oiled_wood(&dir), Units::METERS),
        ("glass", glass(), millimetres),
        ("kinogaki", kinogaki_definitions(), Units::METERS),
    ];
    for (name, held, units) in documents {
        let path = dir.join(format!("{name}.mtlx"));
        write_with(&path, &held, units, &Unconverted);
    }
    let written = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/written");
    let (passed, said) = oracle(&python, &[&written, &dir]);
    assert!(passed, "MaterialX refuses:\n{said}");
    // Refused: a node of no definition, a unit MaterialX doesn't define.
    let refused = dir.join("refused");
    std::fs::create_dir_all(&refused).unwrap();
    let document = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <brass name="metal" type="surfaceshader" />
  <open_pbr_surface name="glass" type="surfaceshader">
    <input name="transmission_depth" type="float" value="2" unittype="distance" unit="parsec" />
  </open_pbr_surface>
  <surfacematerial name="Brass" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="metal" />
  </surfacematerial>
  <surfacematerial name="Glass" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="glass" />
  </surfacematerial>
</materialx>
"#;
    std::fs::write(refused.join("refused.mtlx"), document).unwrap();
    let (passed, said) = oracle(&python, &[&refused]);
    let named = said.contains("no definition: ['brass']") && said.contains("unit=\"parsec\"");
    assert!(!passed && named, "{said}");
}

/// PLAN's budget: the whole example set reads in at most 50 ms (M4 Max,
/// release).
#[test]
#[ignore = "hand: a measurement, meaningful in release"]
fn hand_run_the_examples_read_within_their_budget() {
    let examples = examples();
    let best = (0..5)
        .map(|_| {
            let started = std::time::Instant::now();
            for example in &examples {
                read(example);
            }
            started.elapsed().as_secs_f64() * 1000.0
        })
        .fold(f64::INFINITY, f64::min);
    println!("MTLX {} examples read in {best:.2} ms", examples.len());
    assert!(best <= 50.0, "{best:.2} ms");
}
