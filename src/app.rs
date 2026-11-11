use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;

fn format_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} Б", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} КБ", bytes as f64 / 1024.0) }
    else if bytes < 1024 * 1024 * 1024 { format!("{:.1} МБ", bytes as f64 / (1024.0 * 1024.0)) }
    else { format!("{:.1} ГБ", bytes as f64 / (1024.0 * 1024.0 * 1024.0)) }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CloudAccount {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub token: String,
}

#[component]
pub fn App() -> impl IntoView {
    // СТАРТОВЫЙ ПУТЬ ТЕПЕРЬ C:/
    let (current_path, set_current_path) = signal("C:/".to_string());
    let (manual_path, set_manual_path) = signal("C:/".to_string());
    
    let (files, set_files) = signal(Vec::<FileInfo>::new());
    let (selected_files, set_selected_files) = signal(HashMap::<String, u64>::new());
    let (status, set_status) = signal("Готов к работе".to_string());

    let (accounts, set_accounts) = signal(Vec::<CloudAccount>::new());
    let (selected_account_id, set_selected_account_id) = signal(String::new());
    let (new_account_name, set_new_account_name) = signal(String::new());
    let (new_account_token, set_new_account_token) = signal(String::new());
    let (show_add_form, set_show_add_form) = signal(false);

    let fetch_files = move |path: String| {
        // Заменяем обратные слеши на прямые для безопасной передачи в URL
        let safe_path = path.replace('\\', "/");
        let path_clone = safe_path.clone();
        
        spawn_local(async move {
            let url = format!("/api/browse?path={}", path_clone);
            if let Ok(resp) = gloo_net::http::Request::get(&url).send().await {
                if let Ok(data) = resp.json::<Vec<FileInfo>>().await {
                    set_files.set(data);
                    set_current_path.set(path_clone.clone());
                    set_manual_path.set(path_clone);
                }
            }
        });
    };

    let fetch_accounts = move || {
        spawn_local(async move {
            if let Ok(resp) = gloo_net::http::Request::get("/api/cloud-accounts").send().await {
                if let Ok(data) = resp.json::<Vec<CloudAccount>>().await {
                    if selected_account_id.get_untracked().is_empty() && !data.is_empty() {
                        set_selected_account_id.set(data[0].id.clone());
                    }
                    set_accounts.set(data);
                }
            }
        });
    };

    let add_account = move |_| {
        let name = new_account_name.get();
        let token = new_account_token.get();
        if name.is_empty() || token.is_empty() { return; }

        spawn_local(async move {
            let body = serde_json::json!({ "provider": "yandex", "name": name, "token": token });
            if let Ok(_) = gloo_net::http::Request::post("/api/cloud-accounts").json(&body).unwrap().send().await {
                set_new_account_name.set(String::new());
                set_new_account_token.set(String::new());
                set_show_add_form.set(false);
                fetch_accounts();
            }
        });
    };

    let upload_to_cloud = move |_| {
        let paths: Vec<String> = selected_files.get().keys().cloned().collect();
        let account_id = selected_account_id.get();
        if paths.is_empty() { set_status.set("Выберите файлы!".to_string()); return; }
        if account_id.is_empty() { set_status.set("Выберите аккаунт!".to_string()); return; }

        set_status.set("Отправка на облако...".to_string());
        spawn_local(async move {
            let body = serde_json::json!({ "account_id": account_id, "paths": paths });
            let req = gloo_net::http::Request::post("/api/upload").json(&body).unwrap();
            
            match req.send().await {
                Ok(resp) if resp.ok() => {
                    let msg = resp.text().await.unwrap_or_default();
                    set_status.set(msg);
                }
                Ok(_) => set_status.set("Ошибка при отправке".to_string()),
                Err(_) => set_status.set("Сетевая ошибка".to_string()),
            }
        });
    };

    Effect::new(move |_| {
        fetch_files("C:/".to_string());
        fetch_accounts();
    });

    let select_all = move |_| {
        let all_files: HashMap<String, u64> = files.get().into_iter().map(|f| (f.path, f.size)).collect();
        set_selected_files.set(all_files);
    };
    
    let clear_all = move |_| { 
        set_selected_files.update(|map| map.clear()); 
    };
    
    let go_to_folder = move |path: String| { fetch_files(path); };

    // ФУНКЦИЯ ПЕРЕХОДА НА УРОВЕНЬ ВЫШЕ
    let go_up = move |_| {
        let current = current_path.get();
        let path = std::path::Path::new(&current);
        if let Some(parent) = path.parent() {
            fetch_files(parent.to_string_lossy().to_string());
        }
    };

    // ФУНКЦИЯ ПЕРЕХОДА ПО РУЧНОМУ ВВОДУ
    let manual_go = move |_| {
        let path = manual_path.get();
        if !path.is_empty() {
            fetch_files(path);
        }
    };

    let total_size = move || {
        selected_files.get().values().sum::<u64>()
    };

    view! {
        <div style="display: flex; padding: 20px; font-family: sans-serif; gap: 20px; height: 90vh; box-sizing: border-box;">
            
            <div style="flex: 2; border: 1px solid #ccc; padding: 10px; overflow-y: auto; display: flex; flex-direction: column; border-radius: 8px;">
                <h2 style="margin-top: 0;">"Проводник"</h2>
                
                // ПАНЕЛЬ НАВИГАЦИИ
                <div style="margin-bottom: 10px; display: flex; gap: 10px; align-items: center;">
                    <button type="button" on:click=go_up style="padding: 6px 12px; cursor: pointer; font-weight: bold;">"⬆ Назад"</button>
                    <input type="text" 
                           prop:value=move || manual_path.get()
                           on:input=move |ev| set_manual_path.set(event_target_value(&ev))
                           style="flex: 1; padding: 6px; border: 1px solid #ccc; border-radius: 4px;" 
                    />
                    <button type="button" on:click=manual_go style="padding: 6px 12px; cursor: pointer;">"Перейти"</button>
                </div>

                <div style="margin-bottom: 10px; background: #eee; padding: 8px; border-radius: 4px; font-size: 0.9em; word-break: break-all;">
                    <strong>"Текущий путь: "</strong> {move || current_path.get()}
                </div>
                
                <div style="display: flex; gap: 10px; margin-bottom: 15px;">
                    <button type="button" on:click=select_all style="padding: 6px 12px; cursor: pointer;">"Выбрать все"</button>
                    <button type="button" on:click=clear_all style="padding: 6px 12px; cursor: pointer;">"Очистить"</button>
                </div>
                
                <ul style="list-style: none; padding: 0; margin: 0;">
                    {move || {
                        files.get().into_iter().map(|file| {
                            let is_selected = selected_files.get().contains_key(&file.path);
                            let file_path_click = file.path.clone();
                            let file_path_check = file.path.clone();
                            let file_size = file.size;
                            
                            if file.is_dir {
                                view! {
                                    <li style="padding: 8px; border-bottom: 1px solid #eee; display: flex; align-items: center; justify-content: space-between;">
                                        <div style="display: flex; align-items: center; overflow: hidden;">
                                            <input type="checkbox" 
                                                   prop:checked=is_selected
                                                   style="margin-right: 10px; cursor: pointer; width: 16px; min-width: 16px; height: 16px;"
                                                   on:change=move |ev| {
                                                       let checked = event_target_checked(&ev);
                                                       set_selected_files.update(|map| {
                                                           if checked { map.insert(file_path_check.clone(), file_size); } 
                                                           else { map.remove(&file_path_check); }
                                                       });
                                                   }
                                            />
                                            <span style="font-size: 1.1em; cursor: pointer; color: #0366d6; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;" on:click=move |_| go_to_folder(file_path_click.clone())>
                                                "📁 " {file.name.clone()}
                                            </span>
                                        </div>
                                        <span style="color: gray; font-size: 0.9em; white-space: nowrap; margin-left: 10px;">
                                            {format_size(file.size)}
                                        </span>
                                    </li>
                                }.into_any()
                            } else {
                                view! {
                                    <li style="padding: 8px; border-bottom: 1px solid #eee; display: flex; align-items: center; justify-content: space-between;">
                                        <div style="display: flex; align-items: center; overflow: hidden;">
                                            <input type="checkbox" 
                                                   prop:checked=is_selected 
                                                   style="margin-right: 10px; cursor: pointer; width: 16px; min-width: 16px; height: 16px;"
                                                   on:change=move |ev| {
                                                       let checked = event_target_checked(&ev);
                                                       set_selected_files.update(|map| {
                                                           if checked { map.insert(file_path_check.clone(), file_size); } 
                                                           else { map.remove(&file_path_check); }
                                                       });
                                                   }
                                            />
                                            <span style="white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">
                                                "📄 " {file.name.clone()}
                                            </span>
                                        </div>
                                        <span style="color: gray; font-size: 0.9em; white-space: nowrap; margin-left: 10px;">
                                            {format_size(file.size)}
                                        </span>
                                    </li>
                                }.into_any()
                            }
                        }).collect::<Vec<_>>()
                    }}
                </ul>
            </div>

            <div style="flex: 1; border: 1px solid #ccc; padding: 10px; background: #f9f9f9; display: flex; flex-direction: column; border-radius: 8px;">
                <h3 style="margin-top: 0;">
                    "Выбрано: " {move || selected_files.get().len()} 
                    " (" {move || format_size(total_size())} ")"
                </h3>
                
                <div style="flex: 1; overflow-y: auto; background: #fff; border: 1px solid #ddd; padding: 10px; border-radius: 4px; margin-bottom: 10px;">
                    <ul style="font-size: 0.85em; color: #333; word-break: break-all; padding: 0; list-style: none; margin: 0;">
                        {move || {
                            selected_files.get().into_keys().map(|path| {
                                view! { <li style="margin-bottom: 5px;">"✅ " {path}</li> }
                            }).collect::<Vec<_>>()
                        }}
                    </ul>
                </div>

                <div style="border-top: 2px solid #ddd; padding-top: 10px; margin-bottom: 10px;">
                    <h4 style="margin: 0 0 10px 0;">"☁️ Облачное хранилище"</h4>
                    
                    <select style="width: 100%; padding: 8px; margin-bottom: 10px; border-radius: 4px; border: 1px solid #ccc;"
                            on:change=move |ev| { set_selected_account_id.set(event_target_value(&ev)); }>
                        <option value="">"Выберите аккаунт..."</option>
                        {move || {
                            accounts.get().into_iter().map(|acc| {
                                view! { <option value=acc.id.clone()>{acc.name.clone()} " (" {acc.provider.clone()} ")"</option> }
                            }).collect::<Vec<_>>()
                        }}
                    </select>

                    <button type="button" on:click=move |_| set_show_add_form.set(!show_add_form.get()) style="width: 100%; padding: 6px; margin-bottom: 10px; cursor: pointer; background: #eee; border: 1px solid #ccc; border-radius: 4px;">
                        "➕ Добавить аккаунт"
                    </button>

                    <Show when=move || show_add_form.get()>
                        <div style="background: #fff; padding: 10px; border: 1px solid #ddd; border-radius: 4px; margin-bottom: 10px;">
                            <input type="text" placeholder="Название (например, Мой Яндекс)" 
                                   style="width: 100%; padding: 6px; margin-bottom: 5px; box-sizing: border-box;"
                                   on:input=move |ev| set_new_account_name.set(event_target_value(&ev)) />
                            <input type="text" placeholder="OAuth Токен" 
                                   style="width: 100%; padding: 6px; margin-bottom: 5px; box-sizing: border-box;"
                                   on:input=move |ev| set_new_account_token.set(event_target_value(&ev)) />
                            <button type="button" on:click=add_account style="width: 100%; padding: 6px; cursor: pointer; background: #0366d6; color: white; border: none; border-radius: 4px;">
                                "Сохранить"
                            </button>
                        </div>
                    </Show>
                </div>

                <div style="margin-top: auto; padding: 10px; background: #fff; border: 1px solid #ddd; text-align: center; border-radius: 4px; font-weight: bold; font-size: 0.9em;">
                    {move || status.get()}
                </div>

                <button type="button" on:click=upload_to_cloud style="margin-top: 10px; width: 100%; padding: 12px; background: #2ea44f; color: white; border: none; border-radius: 4px; cursor: pointer; font-size: 16px; font-weight: bold;">
                    "🚀 Архивировать и Отправить"
                </button>
            </div>
        </div>
    }
}