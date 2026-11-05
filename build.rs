use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    // Embed the app icon (window, taskbar, Explorer and tray).
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon/app.ico");
    embed_resource::compile("assets/app.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();

    bundle_iot_manager();
}

/// Deflates every file in `vendor/iot_manager` into OUT_DIR and writes `iot_files.rs`, a
/// table of `(name, size, compressed bytes)` that `src/iot.rs` includes. Compressing keeps the
/// exe about 17 MB smaller than embedding the 27 MB folder as is.
fn bundle_iot_manager() {
    let src = PathBuf::from("vendor/iot_manager");
    println!("cargo:rerun-if-changed={}", src.display());
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("iot");
    std::fs::create_dir_all(&out).unwrap();

    let mut names: Vec<String> = std::fs::read_dir(&src)
        .unwrap_or_else(|e| panic!("{}: {e}", src.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.ends_with(".exe") || n.ends_with(".dll")
        })
        .collect();
    names.sort();
    assert!(
        names.iter().any(|n| n == "iot_manager_rs.exe"),
        "vendor/iot_manager/iot_manager_rs.exe is missing"
    );

    let mut table = String::from("pub(crate) const FILES: &[(&str, usize, &[u8])] = &[\n");
    for name in &names {
        let path = src.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let data = std::fs::read(&path).unwrap();
        let packed = miniz_oxide::deflate::compress_to_vec(&data, 9);
        let dest = out.join(format!("{name}.deflate"));
        std::fs::write(&dest, packed).unwrap();
        writeln!(
            table,
            "    ({name:?}, {}, include_bytes!({:?})),",
            data.len(),
            dest.display().to_string()
        )
        .unwrap();
    }
    table.push_str("];\n");
    std::fs::write(out.join("iot_files.rs"), table).unwrap();
}
