// Empêche l'ouverture d'une console supplémentaire sur Windows en release (sans effet sur macOS).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    flux_lib::run()
}
