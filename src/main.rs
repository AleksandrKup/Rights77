#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod application;
use std::path::Path;
fn main() -> eframe::Result {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.first().is_some_and(|value| value == "--inspect") {
        let Some(path) = arguments.get(1) else {
            eprintln!("rights77 --inspect <папка базы или MD> [--json file] [--csv file]");
            std::process::exit(2)
        };
        match rights77::reader::load(Path::new(path)) {
            Ok(database) => {
                println!(
                    "{}: {} objects, {} roles, {} users",
                    database.title,
                    database.objects.len(),
                    database.roles.len(),
                    database.users.len()
                );
                for permission_set in &database.roles {
                    println!(
                        "{}: {} blocks",
                        permission_set.name,
                        permission_set.blocks.len()
                    );
                }
                for warning in &database.warnings {
                    println!("WARNING: {warning}");
                }
                for pair in arguments[2..].chunks(2) {
                    if pair.len() != 2 {
                        eprintln!("Missing output path");
                        std::process::exit(2)
                    }
                    let export_json = match pair[0].as_str() {
                        "--json" => true,
                        "--csv" => false,
                        _ => {
                            eprintln!("Unknown flag");
                            std::process::exit(2)
                        }
                    };
                    if let Err(error) =
                        rights77::report::save_export(&database, Path::new(&pair[1]), export_json)
                    {
                        eprintln!("{error}");
                        std::process::exit(1)
                    }
                }
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1)
            }
        }
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!("../assets/rights77.png"))
        .expect("icon")
        .to_rgba8();
    let window_size = if std::env::var_os("RIGHTS77_RENDER_DIR").is_some() {
        std::env::var("RIGHTS77_QA_SIZE")
            .ok()
            .and_then(|value| {
                let (width, height) = value.split_once('x')?;
                Some([width.parse::<f32>().ok()?, height.parse::<f32>().ok()?])
            })
            .unwrap_or([1220., 820.])
    } else {
        [1220., 820.]
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(window_size)
            .with_min_inner_size([960., 650.])
            .with_icon(eframe::egui::IconData {
                width: icon.width(),
                height: icon.height(),
                rgba: icon.into_raw(),
            }),
        ..Default::default()
    };
    let initial_database_path = arguments.first().cloned();
    eframe::run_native(
        "Права 1С 7.7 — Rights77",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(application::RightsApplication::new(
                creation_context,
                initial_database_path,
            )))
        }),
    )
}
