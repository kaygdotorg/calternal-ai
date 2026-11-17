//! Optional UniFFI command-line generator for native bindings; requires the uniffi feature.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
fn main() {
    uniffi::uniffi_bindgen_main();
}
