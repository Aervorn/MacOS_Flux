use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_swift();
    }
    tauri_build::build()
}

fn xcrun(args: &[&str]) -> String {
    let out = Command::new("xcrun")
        .args(args)
        .output()
        .expect("xcrun introuvable : installe les outils de ligne de commande de Xcode");
    assert!(out.status.success(), "xcrun {args:?} a échoué");
    String::from_utf8(out.stdout).expect("sortie de xcrun illisible").trim().to_string()
}

/// Compile le code Swift (résumés par Apple Intelligence, autorisation des notifications) en
/// bibliothèque statique.
fn build_swift() {
    let sources = ["swift/Summarizer.swift", "swift/Notifications.swift"];
    for source in sources {
        println!("cargo:rerun-if-changed={source}");
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let swift_arch = if arch == "aarch64" { "arm64" } else { "x86_64" };
    let sdk = xcrun(&["--sdk", "macosx", "--show-sdk-path"]);
    // Apple Intelligence n'existe que sur Apple Silicon, et seulement si le SDK fournit le framework
    let with_ai =
        arch == "aarch64" && Path::new(&sdk).join("System/Library/Frameworks/FoundationModels.framework").exists();
    if !with_ai {
        println!("cargo:warning=Flux ({arch}) est compilé sans les résumés par IA (SDK de macOS 26 et Apple Silicon requis)");
    }

    let mut cmd = Command::new("xcrun");
    cmd.args(["swiftc", "-emit-library", "-static", "-parse-as-library", "-O", "-swift-version", "5"])
        .args(["-module-name", "FluxAI", "-target", &format!("{swift_arch}-apple-macos14.0"), "-sdk", &sdk])
        // Pas de lien automatique (donc fort) vers FoundationModels : il est lié faiblement plus bas,
        // pour que Flux s'ouvre aussi sur macOS 14 et 15
        .args(["-Xfrontend", "-disable-autolink-framework", "-Xfrontend", "FoundationModels"]);
    if !with_ai {
        cmd.args(["-D", "FLUX_NO_AI"]);
    }
    cmd.args(sources).arg("-o").arg(out_dir.join("libfluxai.a"));
    let status = cmd.status().expect("swiftc introuvable : installe Xcode");
    assert!(status.success(), "la compilation du code Swift a échoué");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=fluxai");
    // Bibliothèques Swift : celles du système (macOS 14+) et celles de compatibilité de la chaîne d'outils
    let swiftc = PathBuf::from(xcrun(&["--find", "swiftc"]));
    if let Some(usr) = swiftc.parent().and_then(Path::parent) {
        println!("cargo:rustc-link-search=native={}", usr.join("lib/swift/macosx").display());
    }
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    if with_ai {
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,FoundationModels");
    }
}
