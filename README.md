# SideWidget

Колонка у правого края Windows 11 (Tauri 2): Awwwards, Behance Curated, Product Hunt, News. Лента без заголовков, картинки растягиваются под высоту экрана.

Данные виджет собирает **сам**, прямо с сайтов, раз в 3 часа. GitHub нужен только как запасной источник.

- `src/` — вёрстка без фреймворков. Предпросмотр в браузере: любой статический сервер на `src/` (данные из `mock.json`).
- `src-tauri/` — окно (без рамки, без кнопки на панели задач, под остальными окнами, Acrylic) и сбор данных:
  - `src/collect.rs` — четыре источника (Awwwards, Behance, Product Hunt, News);
  - `src/main.rs` — окно, цикл обновления, кэш картинок в `%LOCALAPPDATA%\com.evgen.sidewidget\img`, запасной источник.
- `collector/` — тот же сборщик на Node для GitHub Actions: он публикует `data.json` в ветку `data`, откуда виджет берёт блок, если локально его собрать не удалось.
- `.github/workflows/collect.yml` — каждые 3 часа запускает `collector/` (плановые запуски GitHub нередко опаздывают или пропускаются).

## Как обновляются данные
1. Раз в 3 часа (проверка часов каждые 30 минут, поэтому после сна ноутбука обновление случится вскоре) виджет параллельно запрашивает все четыре источника.
2. Блок, который не удалось собрать, берётся из того, что свежее: копия `data.json` с GitHub или прошлый кэш на диске. Для Product Hunt без ключей и без копий — публичная лента.
3. Если не удалось вообще ничего (нет сети), ничего не меняется, повтор через 30 минут.

## Настройка: config.json
При первом запуске виджет создаёт `%LOCALAPPDATA%\com.evgen.sidewidget\config.json`. Файл перечитывается при каждом обновлении, перезапуск не нужен.

```json
{
  "interval_hours": 3,
  "ph_client_id": "",
  "ph_client_secret": "",
  "url": "https://raw.githubusercontent.com/SayBearDes/sidewidget/data/data.json"
}
```

- `ph_client_id` / `ph_client_secret` — API Key и API Secret приложения с producthunt.com/v2/oauth/applications. С ними блок Product Hunt показывает топ дня по голосам. Без них берётся копия с GitHub, а если и её нет, публичная лента (3 свежих запуска, без голосов и картинок). Ключи хранятся только в этом файле на вашем компьютере.
- `ph_token` — необязательный готовый Developer Token вместо пары ключей.
- `url` — запасной `data.json` (по умолчанию ветка `data` этого репозитория).
- `interval_hours` — как часто собирать данные.

## Запуск виджета
Нужны Rust (rustup), MSVC Build Tools (Desktop development with C++), WebView2 (есть в Win11) и `cargo install tauri-cli --version "^2"`.

```
cargo tauri icon icon.png     # один раз, создаёт src-tauri/icons
cargo tauri dev               # или: cargo tauri build
```
При первом запуске включается автозапуск (ключ Run в реестре текущего пользователя). Второй запуск не создаёт дубликат окна.

Проверить сбор данных без окна (debug-сборка): `sidewidget.exe --collect-once` выводит результат всех источников в консоль; ключи Product Hunt берутся из переменных `PH_CLIENT_ID` / `PH_CLIENT_SECRET`.

Если в Windows выключены «Эффекты прозрачности», окно остаётся прозрачным без размытия (Acrylic в этом случае превращается в серую заливку).

## Секреты GitHub Actions (для запасного источника)
В репозитории (Settings → Secrets and variables → Actions) нужны `PH_CLIENT_ID` и `PH_CLIENT_SECRET`. Без них `collector/` берёт публичную ленту Product Hunt.

## Источники
| Блок | Источник |
|---|---|
| Awwwards | страница наград `awwwards.com/websites/`: свежий Site of the Day и два свежих Honorable Mention |
| Behance | публичный RSS «Featured Projects» (`behance.net/feeds/projects`) |
| Product Hunt | официальный GraphQL API v2, featured-запуски текущего дня по Тихоокеанскому времени (если голоса дня ещё скрыты, топ вчерашнего дня) |
| News | RSS Sidebar, Smashing Magazine, NN/g (не больше 2 заголовков с одного источника) |
