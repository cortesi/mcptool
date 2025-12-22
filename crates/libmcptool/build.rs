#![allow(missing_docs)]
use anyhow::Result;
use vergen_gix::{BuildBuilder, Emitter, GixBuilder};

fn main() -> Result<()> {
    let build = BuildBuilder::all_build()?;
    let gix = GixBuilder::default().sha(true).build()?;
    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&gix)?
        .emit()
}
