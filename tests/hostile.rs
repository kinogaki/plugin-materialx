//! Hostile documents, each read as File ▸ Import reads one (on a 2 MiB
//! stack, within a budget): a clean error or warning, quickly, never a
//! stack overflow or a read of a file out of reach. Each is a document an
//! audit once made Studio abort or read files it shouldn't with.
use kinogaki_materialx::MaterialX;
use kinogaki_studio_api::format::{ReadLimits, options};
use kinogaki_studio_api::geometry::image::ImageFile;
use kinogaki_studio_api::image_files::{IMAGE_NODE, image_source};
use kinogaki_studio_api::standard::{self, Materials};
use kinogaki_studio_api::{Error, FileFormat, FormatOptions, Imported, Input, Value};
use kinogaki_studio_api_testing::format::Scratch;
use kinogaki_studio_api_testing::read_hostile;
use std::path::Path;
use std::time::Duration;

/// Generous for a debug build: each read here takes milliseconds.
const QUICK: Duration = Duration::from_secs(5);

/// A folder of this test's own holding `files`, the first the document read.
fn folder(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Scratch::new(&format!("mtlx-hostile-{name}"));
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

/// `path` read with `set` over the default options, within `limits`,
/// quickly.
fn read(path: &Path, set: &[(&str, bool)], limits: ReadLimits) -> Result<Imported, Error> {
    let mut chosen: FormatOptions = MaterialX.options();
    chosen.extend(set.iter().map(|(k, v)| ((*k).to_owned(), Value::Bool(*v))));
    let (read, took) = read_hostile(&MaterialX, &Input::file(path), &chosen, limits);
    assert!(took < QUICK, "{}: {took:?}", path.display());
    read
}

/// What a read said, in one text.
fn said(read: &Imported) -> String {
    read.warnings
        .iter()
        .map(|w| w.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The files the read's Image nodes read.
fn images(read: &Imported) -> Vec<ImageFile> {
    let held = standard::materials().get(read.items[0].data.as_ref().unwrap());
    let materials: &Materials = held.unwrap();
    (materials.materials.iter())
        .flat_map(|m| m.inside.nodes.iter())
        .filter(|n| n.node_type == IMAGE_NODE)
        .map(|n| image_source(&n.parameters).unwrap().unwrap().file().clone())
        .collect()
}

/// A material whose surface's `base_color` reads `read` of `nodes`.
fn material(nodes: &str, read: &str) -> String {
    format!(
        "<materialx version=\"1.39\">{nodes}<standard_surface name=\"s\" type=\"surfaceshader\"><input name=\"base_color\" type=\"color3\" nodename=\"{read}\" /></standard_surface><surfacematerial name=\"m\" type=\"material\"><input name=\"surfaceshader\" type=\"surfaceshader\" nodename=\"s\" /></surfacematerial></materialx>"
    )
}

#[test]
fn a_node_reading_itself_is_said_not_followed() {
    let looped = "<multiply name=\"a\" type=\"color3\"><input name=\"in1\" type=\"color3\" nodename=\"a\" /></multiply>";
    let dir = folder("self", &[("self.mtlx", &material(looped, "a"))]);
    let read = read(&dir.join("self.mtlx"), &[], ReadLimits::default()).unwrap();
    assert!(said(&read).contains("a reads itself"), "{}", said(&read));
}

#[test]
fn a_chain_of_two_thousand_nodes_is_cut_at_its_depth() {
    let node = |k: usize| {
        format!(
            "<multiply name=\"n{k}\" type=\"color3\"><input name=\"in1\" type=\"color3\" nodename=\"n{}\" /></multiply>",
            k + 1
        )
    };
    let nodes: String =
        (0..2000).map(node).collect::<String>() + "<constant name=\"n2000\" type=\"color3\" />";
    let dir = folder("chain", &[("chain.mtlx", &material(&nodes, "n0"))]);
    let read = read(&dir.join("chain.mtlx"), &[], ReadLimits::default()).unwrap();
    assert!(said(&read).contains("deeper than 256"), "{}", said(&read));
}

#[test]
fn elements_nested_five_thousand_deep_are_refused() {
    let deep = format!(
        "<materialx version=\"1.39\">{}{}</materialx>",
        "<a>".repeat(5000),
        "</a>".repeat(5000)
    );
    let dir = folder("deep", &[("deep.mtlx", &deep)]);
    let why = read(&dir.join("deep.mtlx"), &[], ReadLimits::default()).unwrap_err();
    assert!(why.message().contains("nested deeper than 256"), "{why}");
}

#[test]
fn a_document_including_itself_twice_reads_once() {
    let twice = "<materialx version=\"1.39\"><xi:include href=\"./self.mtlx\" /><xi:include href=\"self.mtlx\" /><constant name=\"c\" type=\"float\" /></materialx>";
    let dir = folder("include-self", &[("self.mtlx", twice)]);
    let read = read(&dir.join("self.mtlx"), &[], ReadLimits::default()).unwrap();
    assert_eq!(
        said(&read).matches("includes itself").count(),
        2,
        "{}",
        said(&read)
    );
}

#[test]
fn documents_including_the_next_twice_read_each_once() {
    // Thirty files, each including the next twice: read whole, 2^30 copies.
    let level = |k: usize| {
        let next = format!("<xi:include href=\"{}.mtlx\" />", k + 1);
        format!(
            "<materialx version=\"1.39\">{next}{next}<constant name=\"c{k}\" type=\"float\" /></materialx>"
        )
    };
    let texts: Vec<(String, String)> = (0..30).map(|k| (format!("{k}.mtlx"), level(k))).collect();
    let files: Vec<(&str, &str)> = texts
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    let dir = folder("diamond", &files);
    let read = read(&dir.join("0.mtlx"), &[], ReadLimits::default()).unwrap();
    assert!(
        said(&read).contains("30.mtlx"),
        "the one missing: {}",
        said(&read)
    );
}

#[test]
fn includes_and_images_outside_the_folder_are_read() {
    let nodes = "<xi:include href=\"../shared/lib.mtlx\" /><image name=\"i\" type=\"color3\"><input name=\"file\" type=\"filename\" value=\"../maps/wood.png\" /></image><image name=\"j\" type=\"color3\" fileprefix=\"../\"><input name=\"file\" type=\"filename\" value=\"maps/oak.png\" /></image><mix name=\"x\" type=\"color3\"><input name=\"fg\" type=\"color3\" nodename=\"i\" /><input name=\"bg\" type=\"color3\" nodename=\"j\" /></mix>";
    let lib = "<materialx version=\"1.39\"><constant name=\"shared\" type=\"float\" /></materialx>";
    let files = [
        ("scene/look.mtlx", material(nodes, "x")),
        ("shared/lib.mtlx", lib.to_owned()),
        ("maps/wood.png", "wood".to_owned()),
        ("maps/oak.png", "oak".to_owned()),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let dir = folder("outside", &files);
    for embed in [false, true] {
        let set = [(options::EMBED, embed)];
        let read = read(&dir.join("scene/look.mtlx"), &set, ReadLimits::default()).unwrap();
        assert!(said(&read).is_empty(), "{}", said(&read));
        let mut found: Vec<String> = (images(&read).iter())
            .map(|file| match file {
                ImageFile::Path(path) => path.rsplit('/').next().unwrap().to_owned(),
                ImageFile::Embedded(held) => String::from_utf8_lossy(held.bytes()).into_owned(),
            })
            .collect();
        found.sort();
        match embed {
            false => assert_eq!(found, ["oak.png", "wood.png"]),
            true => assert_eq!(found, ["oak", "wood"]),
        }
    }
}

#[test]
fn devices_and_pipes_are_never_read() {
    let text = "<materialx version=\"1.39\"><xi:include href=\"/dev/zero\" /><xi:include href=\"pipe.mtlx\" /></materialx>";
    let dir = folder("devices", &[("look.mtlx", text)]);
    let made = std::process::Command::new("mkfifo")
        .arg(dir.join("pipe.mtlx"))
        .status();
    assert!(made.unwrap().success(), "a pipe to read");
    let read = read(&dir.join("look.mtlx"), &[], ReadLimits::default()).unwrap();
    let said = said(&read);
    assert!(
        said.contains("not regular files, never read: /dev/zero, pipe.mtlx"),
        "{said}"
    );
}

#[test]
fn included_documents_are_read_within_the_budget() {
    let big = format!(
        "<materialx version=\"1.39\"><!-- {} --></materialx>",
        "x".repeat(4096)
    );
    let text = "<materialx version=\"1.39\"><xi:include href=\"big.mtlx\" /></materialx>";
    let dir = folder("budget", &[("look.mtlx", text), ("big.mtlx", &big)]);
    let small = ReadLimits {
        bytes: 1024,
        ..ReadLimits::default()
    };
    let read = read(&dir.join("look.mtlx"), &[], small).unwrap();
    assert!(
        said(&read).contains("over the read's budget of 1.0 KiB"),
        "{}",
        said(&read)
    );
}
