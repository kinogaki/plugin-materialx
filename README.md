# MaterialX

MaterialX documents (`.mtlx`) as a format plugin for [Kinogaki Studio](https://github.com/kinogaki): a document's materials read as Materials of the public [studio-sdk](https://github.com/kinogaki/studio-sdk), MaterialX networks of nodes with their files as Image nodes, and Materials write as one document. The SDK's `kinogaki-mtlx` reads and writes the document, resolves its includes and flattens its graphs, and its `format::materialx` converts between those materials and Studio's; this crate is the format around them.

The library crate is `kinogaki-materialx`; `plugin/` (`kinogaki-plugin-materialx`) is the dylib Studio loads, `formats/materialx.dylib`. Package `kinogaki.materialx`, format `kinogaki.format.mtlx`, extension `mtlx`.

## Reading

One materials item, named after the file, which File ▸ Import makes a Material Library of:

- **Materials:** each material node at the document's top level (`surfacematerial`, `volumematerial`) with every node it reads, keyed by its name and named by its `uiname` where it has one. `xi:include` reads the documents it names, relative to the one naming them, each once however often it is named, at most 256 deep.
- **Files a document names** (includes and images) are found as every Studio format finds them (the SDK's `format::files::Named`): wherever they are, regular files alone; what can't be read is listed in one warning.
- **Graphs flattened:** a node graph's nodes are the material's (every node of a graph it reads from, read or not), and a node the document defines itself by a graph is that graph's nodes; the library's nodes stay nodes (Studio's MaterialX nodes, `kinogaki.mtlx.<node>` with their signature).
- **The shown node:** a material node holding only a surface shows that surface, as a Material shows one; any other (a displacement, a back surface) shows itself.
- **Values** as the inputs hold them; colors converted from the space the document says (`colorspace`, its own or an enclosing element's; `lin_rec709` where none says) into the project's working space.
- **Files:** each `filename` value an Image node wired into its input, one per distinct file and color space in a material: its path (with `fileprefix`) relative to the document's folder, embedded with **Embed Textures**; its `colorspace` the Image node's Color (none: Auto; `raw`, `none` or `data`: Data; another: that space). A `<UDIM>` set reads as the one file it names, said.
- **Lengths** (glTF PBR's thickness, a transmission depth: the inputs the SDK holds as lengths) are written in the project's unit, said on each (`unittype="distance" unit="millimeter"`; metres where MaterialX names no unit of the project's size). Read, a length whose unit the document says (on the input, its node, its graph or the document) moves into the project's, a glTF texture's factor driving it too; one it says none of stays as read.
- **Double Sided:** Kinogaki's `kinogaki:doublesided="false"` on the material node; double sided where nothing says.
- **Left out, said:** looks and their material assignments (they bind materials to geometry a document alone doesn't have), a node no definition resolves (what it fed holds its own value), a value its input can't hold.

## Writing

One document of every Material the materials item holds:

- each Material as a node graph `NG_<name>` of its nodes beside its material node, which reads the graph's outputs; a surface shown alone read by a `surfacematerial` named as the Material;
- names made valid MaterialX names, unique, a Material's own as its `uiname` where that changes it; `kinogaki:doublesided="false"` for a single-sided one;
- each input that differs from its definition's default; colors in `lin_rec709`, the document's `colorspace`;
- an Image node feeding a `filename`: a file by its path relative to the document with its Color as the input's `colorspace` (Auto as `""`, Data as `none`, MaterialX's own spelling); an embedded file written beside the document into `<stem>_textures/`, once for equal bytes; a Result source left out, said;
- Kinogaki's definitions the materials use (`kinogaki_phong_surface`, glTF's minification filter, Surface Unlit's cutout), so any MaterialX reader resolves them;
- a node's `nodedef` where its category, type and inputs wouldn't resolve to it;
- an engine's own material as its standard form, an OpenPBR Surface; one nothing standard approximates left out, said.

## Code

| File | Role |
|---|---|
| `src/lib.rs` | The package and the format: the file read and written whole around the SDK's `format::materialx` |

## Tests and data

`tests/documents.rs`:

- MaterialX's example materials (`tests/fixtures/examples`, 1.39.5's `resources/Materials/Examples`, the `.mtlx` files alone, Apache-2.0: [LICENSE](tests/fixtures/examples/LICENSE)) read, written and read again as the same materials (every input at its value, the defaults' too), each written as `tests/fixtures/written` holds (`MTLX_WRITTEN=write cargo test` writes them again);
- an included document's definitions and a missing one;
- colors into the working space and back;
- lengths written in millimetres saying their unit and read into centimetres (a texture's factor too), a unit said on a node, and a document saying none;
- Image nodes reading a file, an embedded file and a result, written and read back as the same images, with and without Embed Textures.

`tools/materialx_oracle.py` reads documents with MaterialX 1.39.5's own Python module: it validates each written one, checks that every node its materials read resolves to a definition, and compares one written of an example with that example material by material (every node a material reads through its graphs, by category, definition and values, names aside; a value on an input reading the geometry, which the format leaves out and says, aside). `tools/make_materialx.py FOLDER` makes the module's virtual environment: PyPI's MaterialX 1.39.5 wheel for macOS arm64, refused unless its SHA-256 is one the script pins. The hand test `hand_run_materialx_validates_every_document_the_format_writes` runs the oracle over `tests/fixtures/written` and over the documents the round trips write (Image nodes, lengths in millimetres, Kinogaki's definitions); in Studio's tree, `tools/fetch_data.py` makes the environment in `~/.kinogaki/data/materialx` and the test runner names its Python to the test in `MATERIALX_PYTHON`. The module is never a dependency.

## Building

The format builds from this repository and the public studio-sdk, checked out side by side as Studio's tree lays them out (`sdk/` and `plugins/materialx/`), with the compiler the SDK pins (`sdk/rust-toolchain.toml`):

```sh
cd plugins/materialx
cargo test
```

`plugin/` links Studio's shared library (`kinogaki_shared`), so it is built in Studio's tree, or against a Studio release's plugin kit as the SDK's guides describe. Studio's tree also runs the MaterialX workflow (`workflows/`): a document imported and rendered in Spectre.

## License

Dual-licensed under the MIT license ([LICENSE-MIT](LICENSE-MIT)) and the Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option. The test fixtures under `tests/fixtures/examples` are MaterialX's, under its Apache License 2.0 ([LICENSE](tests/fixtures/examples/LICENSE)).
