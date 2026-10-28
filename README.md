# SideWidget

Колонка у правого края Windows 11 (Tauri 2): Awwwards, Behance Curated, Product Hunt, News.

- `src/` — вёрстка без фреймворков. Предпросмотр в браузере: любой статический сервер на `src/` (данные из `mock.json`).
- `src-tauri/` — окно (без рамки, без кнопки на панели задач, под остальными окнами, Acrylic), загрузка `data.json` по ETag раз в 6 ч, кэш картинок в `%LOCALAPPDATA%\com.evgen.sidewidget\img`.
- `collector/` — сборщик: `cd collector && npm i && PH_TOKEN=... node index.mjs` → `out/data.json`.
- `.github/workflows/collect.yml` — раз в сутки собирает и пушит `data.json` в ветку `data`.

## Запуск виджета
Нужны Rust (rustup), MSVC Build Tools (Desktop development with C++), WebView2 (есть в Win11) и `cargo install tauri-cli --version "^2"`.

```
cargo tauri icon path/to/icon.png     # один раз, создаёт src-tauri/icons
set WIDGET_DATA_URL=https://raw.githubusercontent.com/<user>/<repo>/data/data.json
cargo tauri dev        # или: cargo tauri build
```
URL можно переопределить без пересборки: `%LOCALAPPDATA%\com.evgen.sidewidget\config.json` → `{"url": "...", "interval_hours": 6}`.
При первом запуске включается автозапуск (ключ Run в реестре текущего пользователя).

## Состояние источников
| Блок | Статус |
|---|---|
| News (RSS Sidebar, Smashing, NN/g) | работает, проверено |
| Awwwards (Sites of the Day) | работает, проверено |
| Product Hunt (GraphQL) | написан, нужен `PH_TOKEN` (не проверен) |
| Behance | **не реализован**: сайт отвечает 403 с JS-challenge (защита от ботов) |
