use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::JsCast;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[component]
pub fn App() -> impl IntoView {
    let (current_path, set_current_path) = signal(".".to_string());
    let (files, set_files) = signal(Vec::<FileInfo>::new());
    let (selected_files, set_selected_files) = signal(HashSet::<String>::new());
    let (status, set_status) = signal("Готов к работе".to_string());

    let fetch_files = move |path: String| {
        let path_clone = path.clone();
        spawn_local(async move {
            let url = format!("/api/browse?path={}", path_clone);
            if let Ok(resp) = gloo_net::http::Request::get(&url).send().await {
                if let Ok(data) = resp.json::<Vec<FileInfo>>().await {
                    set_files.set(data);
                    set_current_path.set(path_clone);
                }
            }
        });
    };

    Effect::new(move |_| {
        fetch_files(".".to_string());
    });

    let select_all = move |_| {
        // Теперь "Выбрать все" берет и файлы, и папки!
        let all_paths: HashSet<String> = files.get().into_iter().map(|f| f.path).collect();
        set_selected_files.set(all_paths);
    };

    let clear_all = move |_| {
        set_selected_files.set(HashSet::new());
    };

    let go_to_folder = move |path: String| {
        fetch_files(path);
    };

    let archive_files = move |_| {
        let paths: Vec<String> = selected_files.get().into_iter().collect();
        if paths.is_empty() {
            set_status.set("Выберите файлы или папки!".to_string());
            return;
        }
        set_status.set("Создание архива...".to_string());
        
        spawn_local(async move {
            let req = gloo_net::http::Request::post("/api/archive")
                .json(&paths).unwrap();
            
            match req.send().await {
                Ok(resp) => {
                    if let Ok(blob_bytes) = resp.binary().await {
                        let array = js_sys::Uint8Array::from(&blob_bytes[..]);
                        let blob = web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&array)).unwrap();
                        let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                        
                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();
                        let a = document.create_element("a").unwrap();
                        a.set_attribute("href", &url).unwrap();
                        a.set_attribute("download", "archive.zip").unwrap();
                        
                        let html_a = a.dyn_into::<web_sys::HtmlElement>().unwrap();
                        html_a.click();
                        
                        set_status.set("Архив скачан!".to_string());
                    }
                }
                Err(_) => set_status.set("Ошибка при создании архива".to_string()),
            }
        });
    };

    view! {
        <div style="display: flex; padding: 20px; font-family: sans-serif; gap: 20px; height: 90vh; box-sizing: border-box;">
            
            <div style="flex: 2; border: 1px solid #ccc; padding: 10px; overflow-y: auto; display: flex; flex-direction: column; border-radius: 8px;">
                <h2 style="margin-top: 0;">"Проводник"</h2>
                <div style="margin-bottom: 10px; background: #eee; padding: 8px; border-radius: 4px;">
                    <strong>"Текущий путь: "</strong> {move || current_path.get()}
                </div>
                
                <div style="display: flex; gap: 10px; margin-bottom: 15px;">
                    <button on:click=select_all style="padding: 6px 12px; cursor: pointer;">"Выбрать все"</button>
                    <button on:click=clear_all style="padding: 6px 12px; cursor: pointer;">"Очистить"</button>
                    <button on:click=move |_| fetch_files(".".to_string()) style="padding: 6px 12px; cursor: pointer;">"Обновить"</button>
                </div>

                <ul style="list-style: none; padding: 0; margin: 0;">
                    {move || {
                        files.get().into_iter().map(|file| {
                            let is_selected = selected_files.get().contains(&file.path);
                            let file_path_click = file.path.clone();
                            let file_path_check = file.path.clone();
                            
                            if file.is_dir {
                                view! {
                                    <li style="padding: 8px; border-bottom: 1px solid #eee; display: flex; align-items: center;">
                                        // Чекбокс для выбора папки
                                        <input type="checkbox" 
                                               checked=is_selected
                                               style="margin-right: 10px; cursor: pointer; width: 16px; height: 16px;"
                                               on:change=move |ev| {
                                                   let checked = event_target_checked(&ev);
                                                   let mut current = selected_files.get();
                                                   if checked {
                                                       current.insert(file_path_check.clone());
                                                   } else {
                                                       current.remove(&file_path_check);
                                                   }
                                                   set_selected_files.set(current);
                                               }
                                        />
                                        // Кликабельное название для входа внутрь
                                        <span style="font-size: 1.1em; cursor: pointer; color: #0366d6;" 
                                              on:click=move |_| go_to_folder(file_path_click.clone())>
                                            "📁 " {file.name.clone()} " (открыть)"
                                        </span>
                                    </li>
                                }.into_any()
                            } else {
                                view! {
                                    <li style="padding: 8px; border-bottom: 1px solid #eee; display: flex; align-items: center;">
                                        <input type="checkbox" 
                                               checked=is_selected
                                               style="margin-right: 10px; cursor: pointer; width: 16px; height: 16px;"
                                               on:change=move |ev| {
                                                   let checked = event_target_checked(&ev);
                                                   let mut current = selected_files.get();
                                                   if checked {
                                                       current.insert(file_path_check.clone());
                                                   } else {
                                                       current.remove(&file_path_check);
                                                   }
                                                   set_selected_files.set(current);
                                               }
                                        />
                                        "📄 " {file.name.clone()}
                                    </li>
                                }.into_any()
                            }
                        }).collect::<Vec<_>>()
                    }}
                </ul>
            </div>

            <div style="flex: 1; border: 1px solid #ccc; padding: 10px; background: #f9f9f9; display: flex; flex-direction: column; border-radius: 8px;">
                <h3 style="margin-top: 0;">"Выбрано элементов: " {move || selected_files.get().len()}</h3>
                
                <div style="flex: 1; overflow-y: auto; background: #fff; border: 1px solid #ddd; padding: 10px; border-radius: 4px;">
                    <ul style="font-size: 0.85em; color: #333; word-break: break-all; padding: 0; list-style: none; margin: 0;">
                        {move || {
                            selected_files.get().into_iter().map(|path| {
                                view! { <li style="margin-bottom: 5px;">"✅ " {path}</li> }
                            }).collect::<Vec<_>>()
                        }}
                    </ul>
                </div>
                
                <div style="margin-top: 10px; padding: 10px; background: #fff; border: 1px solid #ddd; text-align: center; border-radius: 4px; font-weight: bold;">
                    {move || status.get()}
                </div>

                <button on:click=archive_files style="margin-top: 10px; width: 100%; padding: 12px; background: #2ea44f; color: white; border: none; border-radius: 4px; cursor: pointer; font-size: 16px; font-weight: bold;">
                    "📦 Скачать ZIP-архив"
                </button>
            </div>
        </div>
    }
}