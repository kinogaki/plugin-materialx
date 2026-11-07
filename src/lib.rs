//! MaterialX documents (`.mtlx`), a format plugin: a document's materials
//! read as Studio's Materials, MaterialX networks of nodes, and Materials
//! written as one. The SDK's `kinogaki-mtlx` reads and writes the document
//! and flattens its graphs ([`Document::materials`]); this crate converts
//! between those materials and the SDK's, at its boundary:
//!
//! - **Values** take the types Studio's MaterialX nodes hold them in, numbers
//!   at MaterialX's precision (32 bits); colors
//!   are converted from the space the document says they are in (else
//!   `lin_rec709`, MaterialX's working space) into the project's working
//!   space, and written in `lin_rec709`.
//! - **Files.** A `filename` value is an Image node wired into its input, one
//!   per distinct file and color space in a material: its path relative to
//!   the document's folder (embedded with **Embed Textures**), its
//!   `colorspace` the Image node's Color (none said: Auto; `raw`: Data).
//!   Images and `xi:include`s are found as every format finds the files a
//!   file names (the SDK's `format::files::Named`): in the document's folder
//!   and below unless **Read Files Outside the Folder** is on, regular files
//!   alone; a document is included once, however often it is named.
//!   Written, an Image node reading a file names its path relative to the
//!   document; one holding an embedded file writes it beside the document
//!   (`<stem>_textures/`); one reading a result is left out, said.
//! - **The shown node.** A material node holding only a surface shows that
//!   surface, as a Material shows one; any other material node shows itself.
//!   Written, a surface shown alone is read by a `surfacematerial` named as
//!   the Material.
//! - **Lengths** (an input of a `Length`: glTF PBR's thickness, a
//!   transmission depth) say their unit (`unittype="distance"
//!   unit="millimeter"`): the project's, or metres where MaterialX names no
//!   unit of its size. Read, a length the document says the unit of (on the
//!   input, its node, its graph or the document) moves into the project's,
//!   a texture's factor driving it too; one it says none of stays as read.
//! - **Double Sided** is Kinogaki's `kinogaki:doublesided` on the material
//!   node; the name a Material shows, its `uiname` where it isn't a valid
//!   MaterialX name.
//! - **An engine's material** is written as its standard form, an OpenPBR
//!   Surface; a material nothing standard approximates is left out, said.
//!
//! [`Document::materials`]: kinogaki_mtlx::Document::materials
mod read;
mod write;

use kinogaki_studio_api::format::options;
use kinogaki_studio_api::params::Schema;
use kinogaki_studio_api::{
    DataTypeId, Error, FileFormat, FormatId, FormatOptions, FormatWriter, Imported, Input, Package,
    PackageId, Purpose, Reading, Value, standard,
};
use std::sync::LazyLock;

/// The space a document's colors are in where it says none, and the one
/// they are written in: MaterialX's working space.
const DOCUMENT_SPACE: &str = "lin_rec709";

/// The package's id.
pub fn id() -> PackageId {
    PackageId::declared("kinogaki.materialx")
}

/// The format's id.
pub fn format() -> FormatId {
    FormatId::declared("kinogaki.format.mtlx")
}

/// The package: the format alone.
pub fn package() -> Package {
    Package::new(id(), "MaterialX").with_format(MaterialX)
}

/// What it reads and writes: materials.
static MATERIALS: LazyLock<DataTypeId> = LazyLock::new(|| standard::materials().id());

/// The format.
pub struct MaterialX;

impl FileFormat for MaterialX {
    fn id(&self) -> FormatId {
        format()
    }
    fn label(&self) -> &str {
        "MaterialX"
    }
    fn extensions(&self) -> &[&str] {
        &["mtlx"]
    }
    fn produces(&self) -> &[DataTypeId] {
        std::slice::from_ref(&*MATERIALS)
    }
    fn options(&self) -> FormatOptions {
        FormatOptions::from([
            (options::EMBED.to_owned(), Value::Bool(false)),
            (options::OUTSIDE.to_owned(), Value::Bool(false)),
        ])
    }
    fn schema(&self, chosen: &FormatOptions, _: Purpose) -> Schema {
        let on = |name| options::option(chosen, name, Value::bool).unwrap_or(false);
        let schema = options::embed(Schema::default(), on(options::EMBED));
        options::outside(schema, on(options::OUTSIDE))
    }
    fn read(
        &self,
        input: &Input<'_>,
        options: &FormatOptions,
        reading: &Reading<'_>,
    ) -> Result<Imported, Error> {
        read::read(input, options, reading)
    }
    fn writer(&self) -> Option<&dyn FormatWriter> {
        Some(&write::Writer)
    }
}
