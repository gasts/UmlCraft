//! Embeds the app icon in `umlcraft.exe`. Does nothing when building for other systems.

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/umlcraft.ico");
    embed_resource::compile("umlcraft.rc", embed_resource::NONE).manifest_required().unwrap();
}
