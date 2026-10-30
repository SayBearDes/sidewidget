# SideWidget

Колонка у правого края Windows 11 (Tauri 2): Awwwards, Behance Curated, Product Hunt, News. Лента без заголовков, картинки растягиваются под высоту экрана.

- `src/` — вёрстка без фреймворков. Предпросмотр в браузере: любой статический сервер на `src/` (данные из `mock.json`).
- `src-tauri/` — окно (без рамки, без кнопки на панели задач, под остальными окнами, Acrylic), загрузка `data.json` по ETag раз в 3 ч, кэш картинок в `%LOCALAPPDATA%\com.evgen.sidewidget\img`.
- `collector/` — сборщик: `cd collector && npm i && node index.mjs out/data.json`.
- `.github/workflows/collect.yml` — раз в сутки собирает и пушит `data.json` в ветку `data`.

## Запуск виджета
Нужны Rust (rustup), MSVC Build Tools (Desktop development with C++), WebView2 (есть в Win11) и `cargo install tauri-cli --version "^2"`.

```
cargo tauri icon icon.png     # один раз, создаёт src-tauri/icons
cargo tauri dev               # или: cargo tauri build
```
По умолчанию данные берутся из ветки `data` этого репозитория. Адрес можно сменить без пересборки: `%LOCALAPPDATA%\com.evgen.sidewidget\config.json` → `{"url": "...", "interval_hours": 3}`, либо при сборке переменной `WIDGET_DATA_URL`.
При первом запуске включается автозапуск (ключ Run в реестре текущего пользователя).

Если в Windows выключены «Эффекты прозрачности», окно остаётся прозрачным без размытия (Acrylic в этом случае превращается в серую заливку).

## Секреты для сборщика
Для топа Product Hunt по голосам в репозитории (Settings → Secrets and variables → Actions) нужны `PH_CLIENT_ID` (API Key) и `PH_CLIENT_SECRET` (API Secret) приложения с producthunt.com/v2/oauth/applications. Без них используется публичная лента (3 свежих запуска, без голосов и картинок).

## Источники
| Блок | Источник |
|---|---|
| Awwwards | парсинг страницы Sites of the Day |
| Behance | публичный RSS «Featured Projects» (`behance.net/feeds/projects`) |
| Product Hunt | официальный GraphQL API v2 (запасной вариант: Atom-лента) |
| News | RSS Sidebar, Smashing Magazine, NN/g |
