#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

mod collect;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

const WIDTH: f64 = 540.0; // логические px
const MARGIN: f64 = 0.0; // отступ от правого края экрана
const IMAGE_BLOCKS: [&str; 4] = ["awwwards", "behance", "producthunt", "news"];

// Запасной источник: data.json, который собирает GitHub Actions. Адрес можно переопределить
// переменной WIDGET_DATA_URL при сборке или полем url в config.json.
const DEFAULT_URL: &str = match option_env!("WIDGET_DATA_URL") {
    Some(u) => u,
    None => "https://raw.githubusercontent.com/SayBearDes/sidewidget/data/data.json",
};

/// %LOCALAPPDATA%\com.evgen.sidewidget\config.json. Читается при каждом обновлении, перезапуск не нужен.
#[derive(Deserialize)]
#[serde(default)]
struct Config {
    url: String,
    interval_hours: u64,
    ph_client_id: String,
    ph_client_secret: String,
    ph_token: String,
}
impl Default for Config {
    fn default() -> Self {
        Self { url: DEFAULT_URL.into(), interval_hours: 3, ph_client_id: String::new(), ph_client_secret: String::new(), ph_token: String::new() }
    }
}

impl Config {
    fn ph_keys(&self) -> collect::PhKeys {
        collect::PhKeys {
            client_id: self.ph_client_id.trim().into(),
            client_secret: self.ph_client_secret.trim().into(),
            token: self.ph_token.trim().into(),
        }
    }
}

/// Создаёт config.json с пустыми полями для ключей Product Hunt, если файла ещё нет.
fn ensure_config(app: &AppHandle) {
    let dir = cache_dir(app);
    let path = dir.join("config.json");
    if path.exists() {
        return;
    }
    let template = json!({ "url": DEFAULT_URL, "interval_hours": 3, "ph_client_id": "", "ph_client_secret": "" });
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(path, serde_json::to_string_pretty(&template).unwrap_or_default());
}

#[derive(Default)]
struct State_ {
    data: Mutex<Option<Value>>,
}

fn cache_dir(app: &AppHandle) -> PathBuf {
    app.path().app_local_data_dir().expect("no local data dir")
}

fn load_config(app: &AppHandle) -> Config {
    fs::read_to_string(cache_dir(app).join("config.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Скачивает картинки блоков в кэш на диск (один раз), в элементы пишет `imageLocal`.
/// Файлы, которых нет в текущих данных, удаляются.
async fn localize_images(client: &reqwest::Client, dir: &Path, data: &mut Value) {
    let img_dir = dir.join("img");
    let _ = fs::create_dir_all(&img_dir);
    let mut keep = Vec::new();
    for block in IMAGE_BLOCKS {
        let Some(items) = data.get_mut(block).and_then(Value::as_array_mut) else { continue };
        for item in items {
            let Some(url) = item.get("image").and_then(Value::as_str).filter(|u| u.starts_with("http")).map(str::to_owned) else { continue };
            let ext = url.split('?').next().and_then(|p| p.rsplit('.').next()).filter(|e| e.len() <= 4).unwrap_or("img");
            let path = img_dir.join(format!("{}.{ext}", fnv(&url)));
            if !path.exists() {
                let bytes = async { client.get(&url).send().await?.error_for_status()?.bytes().await }.await;
                match bytes {
                    Ok(b) => { let _ = fs::write(&path, &b); }
                    Err(_) => continue, // останется удалённая ссылка из `image`
                }
            }
            item["imageLocal"] = Value::String(path.to_string_lossy().into_owned());
            keep.push(path);
        }
    }
    if let Ok(rd) = fs::read_dir(&img_dir) {
        for e in rd.flatten() {
            if !keep.contains(&e.path()) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

async fn publish(app: &AppHandle, mut data: Value) {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).pool_max_idle_per_host(0).build().unwrap();
    localize_images(&client, &cache_dir(app), &mut data).await;
    *app.state::<State_>().data.lock().unwrap() = Some(data.clone());
    let _ = app.emit("data", data);
}

/// Копия data.json с GitHub (запасной источник).
async fn fetch_remote(c: &reqwest::Client, url: &str) -> Result<Value, String> {
    let text = c.get(url).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn updated(v: &Value) -> &str {
    v["updated"].as_str().unwrap_or("")
}

/// Собирает все четыре блока локально, прямо с сайтов. Блок, который не удалось собрать, берётся из того,
/// что свежее: копия с GitHub или прошлый кэш на диске; для Product Hunt без ключей и без копий — публичная
/// лента. Если не удалось вообще ничего (нет сети), ничего не меняется, повтор через 30 минут.
async fn refresh(app: &AppHandle, cfg: &Config) -> Result<(), String> {
    let dir = cache_dir(app);
    let c = collect::client();
    let got = collect::collect_all(&c, &cfg.ph_keys()).await;
    let results = [("awwwards", got.awwwards), ("behance", got.behance), ("news", got.news), ("producthunt", got.producthunt)];
    let failed = results.iter().filter(|r| r.1.is_err()).count();
    if failed == results.len() {
        let first = results.iter().find_map(|r| r.1.as_ref().err()).cloned().unwrap_or_default();
        return Err(format!("all sources failed, first error: {first}"));
    }

    let prev: Option<Value> = fs::read_to_string(dir.join("data.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
    let mut remote: Option<Value> = None;
    let mut remote_tried = false;
    let mut data = json!({ "updated": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true) });
    for (key, res) in results {
        match res {
            Ok(items) => data[key] = Value::Array(items),
            Err(e) => {
                eprintln!("{key}: {e}");
                if !remote_tried {
                    remote_tried = true;
                    remote = fetch_remote(&c, &cfg.url).await.map_err(|e| eprintln!("remote copy: {e}")).ok();
                }
                let block = [remote.as_ref(), prev.as_ref()]
                    .into_iter()
                    .flatten()
                    .filter(|v| v.get(key).and_then(Value::as_array).map_or(false, |a| !a.is_empty()))
                    .max_by(|a, b| updated(a).cmp(updated(b)))
                    .map(|v| v[key].clone());
                data[key] = match block {
                    Some(b) => b,
                    None if key == "producthunt" => collect::producthunt_feed(&c).await.map(Value::Array).unwrap_or_else(|_| json!([])),
                    None => json!([]),
                };
            }
        }
    }

    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("data.json"), serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?);
    publish(app, data).await;
    Ok(())
}

/// `sidewidget.exe --collect-once` (debug-сборка): собрать все источники и вывести результат в консоль.
/// Ключи Product Hunt берутся из переменных PH_CLIENT_ID / PH_CLIENT_SECRET.
fn collect_once() {
    let keys = collect::PhKeys {
        client_id: std::env::var("PH_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("PH_CLIENT_SECRET").unwrap_or_default(),
        token: std::env::var("PH_TOKEN").unwrap_or_default(),
    };
    tauri::async_runtime::block_on(async {
        let c = collect::client();
        let got = collect::collect_all(&c, &keys).await;
        for (name, r) in [("awwwards", got.awwwards), ("behance", got.behance), ("news", got.news), ("producthunt", got.producthunt)] {
            match r {
                Ok(v) => println!("== {name}: ok\n{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
                Err(e) => println!("== {name}: ERROR {e}"),
            }
        }
    });
}

/// Фронтенд подбирает подложку: без системного размытия карточкам нужен более тёмный фон.
#[tauri::command]
fn has_blur() -> bool {
    transparency_enabled()
}

#[tauri::command]
fn get_data(state: State<State_>) -> Option<Value> {
    state.data.lock().unwrap().clone()
}

/// Байты картинки из кэша на диске. Встроенный asset-протокол в релизной сборке отвечал 403,
/// поэтому файл читаем сами. От переданного пути берём только имя файла и ищем его строго
/// в папке кэша картинок, так что прочитать что-то за её пределами нельзя.
#[tauri::command]
fn read_image(app: AppHandle, path: String) -> Result<tauri::ipc::Response, String> {
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .filter(|n| !n.is_empty() && *n != "." && *n != "..")
        .ok_or("bad image name")?;
    fs::read(cache_dir(&app).join("img").join(name))
        .map(tauri::ipc::Response::new)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http(s) links".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "windows"))]
fn transparency_enabled() -> bool {
    true
}

#[cfg(target_os = "windows")]
fn transparency_enabled() -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "EnableTransparency"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).contains("0x0"))
        .unwrap_or(true)
}

/// Колонка у правого края на всю высоту рабочей области (без панели задач).
fn place_window(w: &WebviewWindow) -> tauri::Result<()> {
    let Some(mon) = w.primary_monitor()? else { return Ok(()) };
    let wa = mon.work_area();
    let scale = mon.scale_factor();
    let width = (WIDTH * scale).round() as i32;
    let margin = (MARGIN * scale).round() as i32;
    w.set_size(PhysicalSize::new(width as u32, wa.size.height))?;
    w.set_position(PhysicalPosition::new(wa.position.x + wa.size.width as i32 - width - margin, wa.position.y))?;
    Ok(())
}

fn main() {
    if std::env::args().any(|a| a == "--collect-once") {
        collect_once();
        return;
    }
    tauri::Builder::default()
        // Второй запуск (автозапуск + ручной, установщик) не плодит дубликат окна и процессов WebView2.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(State_::default())
        .invoke_handler(tauri::generate_handler![get_data, open_url, has_blur, read_image])
        .on_window_event(|window, event| {
            if let WindowEvent::ScaleFactorChanged { .. } = event {
                if let Some(w) = window.app_handle().get_webview_window("main") {
                    let _ = place_window(&w);
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let w = app.get_webview_window("main").expect("main window");
            place_window(&w)?;
            #[cfg(target_os = "windows")]
            {
                // Если в Windows выключены «Эффекты прозрачности», акрил превращается в глухую серую
                // заливку — тогда его не применяем, окно остаётся прозрачным (без размытия).
                if transparency_enabled() {
                    if window_vibrancy::apply_acrylic(&w, Some((20, 20, 24, 55))).is_err() {
                        let _ = window_vibrancy::apply_mica(&w, Some(true));
                    }
                }
            }
            w.show()?;

            // Автозапуск при входе в Windows (ключ HKCU\...\Run), без служб.
            // enable() каждый раз перезаписывает путь, чтобы он вёл на текущий exe (а не на старую dev-сборку).
            let _ = handle.autolaunch().enable();

            // Один цикл: показать кэш с диска сразу, затем раз в N часов собрать данные локально.
            tauri::async_runtime::spawn(async move {
                ensure_config(&handle);
                if let Ok(s) = fs::read_to_string(cache_dir(&handle).join("data.json")) {
                    if let Ok(v) = serde_json::from_str(&s) {
                        publish(&handle, v).await;
                    }
                }
                // Раз в 30 минут сверяем настоящие часы: длинный sleep «замирает» во сне ноутбука,
                // а так обновление случится вскоре после пробуждения. При ошибке сети повтор через 30 минут.
                // config.json перечитывается каждый раз: ключи и интервал меняются без перезапуска.
                let mut last_ok: Option<std::time::SystemTime> = None;
                loop {
                    let cfg = load_config(&handle);
                    let interval = Duration::from_secs(cfg.interval_hours.max(1) * 3600);
                    if last_ok.map_or(true, |t| t.elapsed().map_or(true, |e| e >= interval)) {
                        match refresh(&handle, &cfg).await {
                            Ok(()) => last_ok = Some(std::time::SystemTime::now()),
                            Err(e) => eprintln!("refresh failed: {e}"),
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(30 * 60)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running widget");
}
