"use strict";
const IN_TAURI = !!window.__TAURI__;
if (!IN_TAURI) document.body.classList.add("preview");

const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

// Rust кладёт скачанный файл в imageLocal; без него (или в предпросмотре) — удалённая ссылка
const imgSrc = (it) => (it.imageLocal && IN_TAURI ? window.__TAURI__.core.convertFileSrc(it.imageLocal) : it.image);

function ago(iso) {
  const h = Math.round((Date.now() - new Date(iso)) / 36e5);
  return h < 1 ? "только что" : h < 24 ? `${h} ч` : `${Math.round(h / 24)} дн`;
}

const gridCard = (it, caption = true) =>
  `<a class="card" href="${esc(it.url)}" title="${esc(it.title)}">${imgSrc(it) ? `<img src="${esc(imgSrc(it))}" alt="${esc(it.title)}">` : ""}` +
  `${caption ? `<div class="cap"><b>${esc(it.title)}</b><small>${esc(it.meta)}</small></div>` : ""}</a>`;

// строка: превью + заголовок + описание + мета
const row = (it, side = "") =>
  `<a class="card row" href="${esc(it.url)}">${imgSrc(it) ? `<img src="${esc(imgSrc(it))}" alt="">` : ""}` +
  `<div class="t"><b>${esc(it.title)}</b>${it.text ? `<p>${esc(it.text)}</p>` : ""}` +
  `<small>${esc(it.meta)}${it.date ? " · " + ago(it.date) : ""}</small></div>${side}</a>`;

const phRow = (it) => row({ ...it, meta: "" }, `<span class="votes">${esc(it.meta)}</span>`);
const newsRow = (it) => row({ ...it, meta: "", date: null });

function render(d) {
  const put = (id, fn, items) => {
    document.querySelector(`#${id} > div`).innerHTML = (items || []).slice(0, 3).map(fn).join("");
  };
  put("awwwards", (it) => gridCard(it, false), d.awwwards);
  put("behance", (it) => gridCard(it, false), d.behance);
  put("producthunt", phRow, d.producthunt);
  put("news", newsRow, d.news);
}

// Клик: в Tauri открываем в браузере по умолчанию, в предпросмотре — новая вкладка
document.addEventListener("click", (e) => {
  const a = e.target.closest("a[href]");
  if (!a) return;
  e.preventDefault();
  if (IN_TAURI) window.__TAURI__.core.invoke("open_url", { url: a.href });
  else window.open(a.href, "_blank", "noopener");
});

// Данные: Rust отдаёт кэш сразу и шлёт событие "data" после обновления; без Tauri — mock.json
async function load() {
  if (IN_TAURI) {
    const { invoke } = window.__TAURI__.core;
    invoke("has_blur").then((v) => document.documentElement.classList.toggle("no-blur", !v));
    window.__TAURI__.event.listen("data", (ev) => render(ev.payload));
    const cached = await invoke("get_data");
    if (cached) render(cached);
  } else {
    render(await (await fetch("mock.json")).json());
  }
}
load();
