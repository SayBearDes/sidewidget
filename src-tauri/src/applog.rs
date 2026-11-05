//! Журнал виджета: `%LOCALAPPDATA%\com.evgen.sidewidget\widget.log`. У окна без консоли ошибки
//! больше некуда писать, а без них не понять, почему не обновились данные. Хранятся последние ~300 строк.
use std::{fs, io::Write, path::PathBuf, sync::OnceLock};

static PATH: OnceLock<PathBuf> = OnceLock::new();
const KEEP_LINES: usize = 300;

pub fn init(dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    let _ = PATH.set(dir.join("widget.log"));
    // Паника в релизной сборке завершает процесс без следа: пишем её в журнал до выхода.
    std::panic::set_hook(Box::new(|info| log(&format!("PANIC: {info}"))));
}

pub fn log(msg: &str) {
    eprintln!("{msg}");
    let Some(path) = PATH.get() else { return };
    if fs::metadata(path).map_or(false, |m| m.len() > 100_000) {
        if let Ok(s) = fs::read_to_string(path) {
            let lines: Vec<&str> = s.lines().collect();
            let tail = lines[lines.len().saturating_sub(KEEP_LINES)..].join("\n");
            let _ = fs::write(path, format!("{tail}\n"));
        }
    }
    let line = format!("{} {msg}\n", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"));
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}
