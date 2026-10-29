"use strict";
const IN_TAURI = !!window.__TAURI__;
if (!IN_TAURI) document.body.classList.add("preview");

const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

// Rust кладёт скачанный файл в imageLocal. В Tauri файл читаем командой read_image и показываем
// через blob:-URL (hydrateImages); без imageLocal или в предпросмотре — обычная удалённая ссылка.
const imgTag = (it, alt = "") =>
  it.imageLocal && IN_TAURI
    ? `<img data-local="${esc(it.imageLocal)}" alt="${esc(alt)}">`
    : it.image ? `<img src="${esc(it.image)}" alt="${esc(alt)}">` : "";

const MIME = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp", gif: "image/gif", svg: "image/svg+xml" };
let blobUrls = [];

async function hydrateImages() {
  const { invoke } = window.__TAURI__.core;
  await Promise.all([...document.querySelectorAll("img[data-local]")].map(async (img) => {
    try {
      const path = img.dataset.local;
      const bytes = await invoke("read_image", { path });
      if (!img.isConnected) return; // страницу успели перерисовать
      const type = MIME[path.split(".").pop().toLowerCase()] ?? "";
      const url = URL.createObjectURL(new Blob([bytes], { type }));
      blobUrls.push(url);
      img.src = url;
    } catch (e) {
      console.warn("image:", e);
    }
  }));
}

function ago(iso) {
  const h = Math.round((Date.now() - new Date(iso)) / 36e5);
  return h < 1 ? "только что" : h < 24 ? `${h} ч` : `${Math.round(h / 24)} дн`;
}

const gridCard = (it, caption = true) =>
  `<a class="card" href="${esc(it.url)}" title="${esc(it.title)}">${imgTag(it, it.title)}` +
  `${caption ? `<div class="cap"><b>${esc(it.title)}</b><small>${esc(it.meta)}</small></div>` : ""}</a>`;

// строка: превью + заголовок + описание + мета
const row = (it, side = "") =>
  `<a class="card row" href="${esc(it.url)}">${imgTag(it)}` +
  `<div class="t"><b>${esc(it.title)}</b>${it.text ? `<p>${esc(it.text)}</p>` : ""}` +
  `<small>${esc(it.meta)}${it.date ? " · " + ago(it.date) : ""}</small></div>${side}</a>`;

const phRow = (it) => row({ ...it, meta: "" }, `<span class="votes">${esc(it.meta)}</span>`);
const newsRow = (it) => row({ ...it, meta: "", date: null });

function render(d) {
  blobUrls.forEach((u) => URL.revokeObjectURL(u));
  blobUrls = [];
  const put = (id, fn, items) => {
    document.querySelector(`#${id} > div`).innerHTML = (items || []).slice(0, 3).map(fn).join("");
  };
  put("awwwards", (it) => gridCard(it, false), d.awwwards);
  put("behance", (it) => gridCard(it, false), d.behance);
  put("producthunt", phRow, d.producthunt);
  put("news", newsRow, d.news);
  if (IN_TAURI) hydrateImages();
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
