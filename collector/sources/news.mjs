import { XMLParser } from "fast-xml-parser";
import * as cheerio from "cheerio";
import { fetchText, clean } from "../lib.mjs";

const FEEDS = [
  { name: "Sidebar", url: "https://sidebar.io/feed.xml" },
  { name: "Smashing Magazine", url: "https://www.smashingmagazine.com/feed/" },
  { name: "NN/g", url: "https://www.nngroup.com/feed/rss/" },
];
const MAX_PER_SOURCE = 2;
const TEXT_LEN = 150;
const arr = (v) => (v == null ? [] : Array.isArray(v) ? v : [v]);
const text = (v) => (typeof v === "object" && v ? v["#text"] ?? "" : v ?? "");
const cut = (s) => (s.length > TEXT_LEN ? s.slice(0, TEXT_LEN).replace(/\s+\S*$/, "") + "…" : s);

// картинка из media:thumbnail / media:content / enclosure / первого <img> в теле
function feedImage(it) {
  for (const k of ["media:thumbnail", "media:content", "enclosure"]) {
    const u = arr(it[k]).map((x) => x?.["@url"]).find(Boolean);
    if (u) return u;
  }
  const html = text(it["content:encoded"]) || text(it.content) || text(it.description);
  return html.match(/<img[^>]+src=["']([^"']+)["']/i)?.[1] ?? null;
}

async function readFeed({ name, url }) {
  const xml = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: "@", cdataPropName: false }).parse(await fetchText(url));
  const items = arr(xml.rss?.channel?.item).concat(arr(xml.feed?.entry));
  return items.map((it) => {
    const link = typeof it.link === "object" ? arr(it.link).find((l) => l["@rel"] !== "self")?.["@href"] : it.link;
    const date = new Date(text(it.pubDate) || text(it["dc:date"]) || text(it.published) || text(it.updated));
    return {
      title: clean(text(it.title)),
      url: String(link ?? "").trim().replace(/\?utm_[^#]*/, ""),
      text: cut(clean(text(it.description) || text(it.summary) || text(it["content:encoded"]))),
      image: feedImage(it),
      meta: name,
      date: date.getTime() ? date.toISOString() : null,
    };
  }).filter((x) => x.title && x.url.startsWith("http") && x.date);
}

// Для выбранных 3 записей без картинки/описания берём og:image / og:description со страницы статьи
async function fillFromPage(item) {
  if (item.image && item.text) return item;
  try {
    const $ = cheerio.load(await fetchText(item.url));
    const og = (p) => $(`meta[property="${p}"], meta[name="${p}"]`).attr("content");
    const img = og("og:image") || og("twitter:image");
    if (!item.image && img) item.image = new URL(img, item.url).href;
    if (!item.text) item.text = cut(clean(og("og:description") || og("description") || ""));
  } catch (e) {
    console.warn(`news: page ${item.url}: ${e.message}`);
  }
  return item;
}

export default async function news() {
  const results = await Promise.allSettled(FEEDS.map(readFeed));
  results.forEach((r, i) => r.status === "rejected" && console.warn(`news: ${FEEDS[i].name}: ${r.reason.message}`));
  const all = results.flatMap((r) => (r.status === "fulfilled" ? r.value : [])).sort((a, b) => b.date.localeCompare(a.date));
  const count = {};
  const picked = all.filter((x) => (count[x.meta] = (count[x.meta] ?? 0) + 1) <= MAX_PER_SOURCE).slice(0, 3);
  if (!picked.length) throw new Error("no news items");
  return Promise.all(picked.map(fillFromPage));
}
