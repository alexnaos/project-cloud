use axum::{
    routing::{get, post},
    Json, Router,
};
use axum::extract::FromRef;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use leptos_meta::*;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use walkdir::WalkDir;

#[derive(Clone)]
struct AppState {
    leptos_options: LeptosOptions,
    pool: SqlitePool,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct CloudAccount {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub token: String,
}

#[derive(Deserialize)]
pub struct NewAccountRequest {
    pub provider: String,
    pub name: String,
    pub token: String,
}

#[derive(Deserialize)]
pub struct UploadRequest {
    pub account_id: String,
    pub paths: Vec<String>,
}

async fn browse_dir(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Vec<FileInfo>> {
    let path = params.get("path").cloned().unwrap_or_else(|| ".".to_string());
    let mut items = Vec::new();
    
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "target" || name == ".git" || name == "node_modules" { continue; }
            
            let metadata = entry.metadata().ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            
            // Считаем размер (для папок рекурсивно)
            let size = if is_dir {
                WalkDir::new(entry.path())
                    .into_iter()
                    .filter_entry(|e| {
                        let n = e.file_name().to_string_lossy();
                        n != "target" && n != ".git" && n != "node_modules"
                    })
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum()
            } else {
                metadata.map(|m| m.len()).unwrap_or(0)
            };

            items.push(FileInfo {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir,
                size,
            });
        }
    }
    
    items.sort_by(|a, b| {
        if a.is_dir == b.is_dir { a.name.to_lowercase().cmp(&b.name.to_lowercase()) } 
        else if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
    });
    
    Json(items)
}

// --- ВОТ ОНА, ПРОПАВШАЯ ФУНКЦИЯ ---
async fn get_accounts(axum::extract::State(state): axum::extract::State<AppState>) -> Json<Vec<CloudAccount>> {
    let accounts = sqlx::query_as::<_, CloudAccount>("SELECT id, provider, name, token FROM accounts")
        .fetch_all(&state.pool).await.unwrap_or_default();
    Json(accounts)
}

async fn add_account(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<NewAccountRequest>,
) -> Json<CloudAccount> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO accounts (id, provider, name, token) VALUES (?, ?, ?, ?)")
        .bind(&id).bind(&payload.provider).bind(&payload.name).bind(&payload.token)
        .execute(&state.pool).await.unwrap();
    Json(CloudAccount { id, provider: payload.provider, name: payload.name, token: payload.token })
}

async fn upload_to_cloud(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<UploadRequest>,
) -> Result<String, String> {
    let account = sqlx::query_as::<_, CloudAccount>("SELECT * FROM accounts WHERE id = ?")
        .bind(&payload.account_id).fetch_one(&state.pool).await.map_err(|_| "Аккаунт не найден")?;

    let mut buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for path_str in payload.paths {
        let path = std::path::Path::new(&path_str);
        if path.is_file() {
            if let Ok(mut file) = std::fs::File::open(path) {
                let mut contents = Vec::new();
                if file.read_to_end(&mut contents).is_ok() {
                    let _ = zip.start_file(path.file_name().unwrap_or_default().to_string_lossy(), options);
                    let _ = zip.write_all(&contents);
                }
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let full_path = entry.path();
                    if let Ok(mut file) = std::fs::File::open(full_path) {
                        let mut contents = Vec::new();
                        if file.read_to_end(&mut contents).is_ok() {
                            let rel_path = full_path.strip_prefix(path.parent().unwrap_or(path)).unwrap_or(full_path);
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
    let zip_bytes = buffer.into_inner();

    let op = match account.provider.as_str() {
        "yandex" => opendal::Operator::via_iter(
            "yandex-disk",
            [("access_token".to_string(), account.token.clone())]
        ).unwrap(),
        _ => return Err("Провайдер пока не поддерживается".to_string()),
    };

    let remote_filename = format!("archive_{}.zip", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    op.write(&remote_filename, zip_bytes).await.map_err(|e| format!("Ошибка загрузки: {}", e))?;

    Ok(format!("Успешно загружено в {} как {}", account.name, remote_filename))
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

    let pool = SqlitePoolOptions::new().connect("sqlite:cloud.db?mode=rwc").await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS accounts (id TEXT PRIMARY KEY, provider TEXT, name TEXT, token TEXT)")
        .execute(&pool).await.unwrap();

    let state = AppState { leptos_options: leptos_options.clone(), pool };

    let app = Router::new()
        .route("/api/browse", get(browse_dir))
        .route("/api/cloud-accounts", get(get_accounts).post(add_account))
        .route("/api/upload", post(upload_to_cloud))
        .leptos_routes(&state, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service()).await.unwrap();
}