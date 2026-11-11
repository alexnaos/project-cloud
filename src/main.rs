use axum::{
    routing::{get, post},
    Json, Router,
};
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use leptos_meta::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use walkdir::WalkDir; // <--- Новая библиотека для обхода папок

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

async fn browse_dir(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Vec<FileInfo>> {
    let path = params.get("path").cloned().unwrap_or_else(|| ".".to_string());
    let mut items = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
            items.push(FileInfo {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                is_dir,
            });
        }
    }
    Json(items)
}

// ОБНОВЛЕННАЯ ФУНКЦИЯ АРХИВАЦИИ
async fn create_archive(Json(paths): Json<Vec<String>>) -> impl axum::response::IntoResponse {
    let mut buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for path_str in paths {
        let path = std::path::Path::new(&path_str);
        
        if path.is_file() {
            // Если это файл - просто добавляем его
            if let Ok(mut file) = std::fs::File::open(path) {
                let mut contents = Vec::new();
                if file.read_to_end(&mut contents).is_ok() {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    let _ = zip.start_file(file_name, options);
                    let _ = zip.write_all(&contents);
                }
            }
        } else if path.is_dir() {
            // Если это ПАПКА - рекурсивно обходим её содержимое
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let full_path = entry.path();
                    if let Ok(mut file) = std::fs::File::open(full_path) {
                        let mut contents = Vec::new();
                        if file.read_to_end(&mut contents).is_ok() {
                            // Вычисляем путь файла относительно выбранной папки
                            let rel_path = full_path.strip_prefix(path.parent().unwrap_or(path)).unwrap_or(full_path);
                            // Заменяем обратные слеши Windows на прямые для корректного ZIP
                            let zip_path = rel_path.to_string_lossy().replace('\\', "/");
                            
                            let _ = zip.start_file(zip_path, options);
                            let _ = zip.write_all(&contents);
                        }
                    }
                }
            }
        }
    }
    let _ = zip.finish();

    let bytes = buffer.into_inner();
    let headers = [
        ("Content-Type", "application/zip"),
        ("Content-Disposition", "attachment; filename=\"archive.zip\""),
    ];
    
    (headers, bytes)
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <project_cloud::app::App/>
            </body>
        </html>
    }
}

#[tokio::main]
async fn main() {
    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(project_cloud::app::App);

    let app = Router::new()
        .route("/api/browse", get(browse_dir))
        .route("/api/archive", post(create_archive))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service()).await.unwrap();
}