use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let qml_files: Vec<std::path::PathBuf> = std::fs::read_dir("qml")
        .expect("qml directory")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "qml").unwrap_or(false))
        .collect();

    CxxQtBuilder::new_qml_module(QmlModule::new("AsterQt").qml_files(qml_files))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("WebEngineQuick")
        .files(["src/bridge.rs"])
        .build();
}
