//! MaterialX documents (`.mtlx`), a format plugin: a document's materials
//! read as Studio's Materials, MaterialX networks of nodes, and Materials
//! written as one. The SDK reads and writes the document
//! (`kinogaki-mtlx`'s `Document`) and converts between its materials and
//! Studio's (`format::materialx`: values, colors, files, lengths, the shown
//! node, Double Sided, an engine's material by its standard form); this
//! crate is the format around them: the file read and written whole, its
//! includes and images found through `format::files::Named` (wherever they
//! are, regular files alone; embedded with **Embed Textures**), written
//! images beside the document through `format::files::Beside`.
use kinogaki_studio_api::format::files::{Beside, Named, replace};
use kinogaki_studio_api::format::{materialx, options};
use kinogaki_studio_api::params::Schema;
use kinogaki_studio_api::{
    DataTypeId, Diagnostic, Error, FileFormat, FileItem, FormatId, FormatOptions, FormatWriter,
    Imported, Input, Package, PackageId, Purpose, Reading, Value, Writing, standard,
};
use std::path::Path;

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
const MATERIALS: &[DataTypeId] = &[standard::materials().id()];

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
        MATERIALS
    }
    fn options(&self) -> FormatOptions {
        FormatOptions::from([(options::EMBED.to_owned(), Value::Bool(false))])
    }
    fn schema(&self, chosen: &FormatOptions, _: Purpose) -> Schema {
        let on = options::option(chosen, options::EMBED, Value::bool).unwrap_or(false);
        options::embed(Schema::default(), on)
    }
    /// One materials item, named after the file.
    fn read(
        &self,
        input: &Input<'_>,
        chosen: &FormatOptions,
        reading: &Reading<'_>,
    ) -> Result<Imported, Error> {
        reading.check()?;
        let bytes = input.bytes()?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Error::invalid(format!("{input}: not UTF-8 text")))?;
        let named = Named::new(input, chosen, reading);
        let (materials, mut warnings) = materialx::read(text, input, &named, reading)?;
        warnings.extend(named.warnings());
        let name = Path::new(input.name())
            .file_stem()
            .map_or_else(|| "MaterialX".into(), |s| s.to_string_lossy());
        let held = standard::Materials { materials };
        Ok(Imported {
            items: vec![FileItem::top(name, standard::materials().wrap(held))],
            warnings,
        })
    }
    fn writer(&self) -> Option<&dyn FormatWriter> {
        Some(&Writer)
    }
}

/// The format's writer.
struct Writer;

impl FormatWriter for Writer {
    fn accepts(&self) -> &[DataTypeId] {
        MATERIALS
    }

    /// Every materials item's Materials in one document; the images lights
    /// name have no place in one, and are said.
    fn write(
        &self,
        path: &Path,
        items: &[FileItem],
        writing: &Writing<'_>,
        _: &FormatOptions,
    ) -> Result<Vec<Diagnostic>, Error> {
        let mut materials = Vec::new();
        for item in items {
            writing.check()?;
            let held = item
                .data
                .as_ref()
                .and_then(|d| standard::materials().get(d));
            let Some(held) = held else { continue };
            materials.extend(&held.materials);
        }
        let (document, warnings) = materialx::write(&materials, &mut Beside::new(path), writing)?;
        let text = document.to_string();
        let failed = |e: std::io::Error| Error::invalid(format!("{}: {e}", path.display()));
        replace(path, |file| {
            std::io::Write::write_all(file, text.as_bytes())
        })
        .map_err(failed)?;
        Ok(warnings)
    }
}
