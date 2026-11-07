//! MaterialX documents as a Studio plugin library (`formats/materialx.dylib`).
use kinogaki_studio_api::export_package;

kinogaki_shared::link!();
export_package!(kinogaki_materialx::package);
