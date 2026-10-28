import { XMLParser } from "fast-xml-parser";
import { fetchText, clean } from "../lib.mjs";

// Публичный RSS «Featured Projects» — это и есть Curated-галерея Behance (без парсинга HTML и без защиты от ботов).
const FEED = "https://www.behance.net/feeds/projects";
const text = (v) => (typeof v === "object" && v ? v["#text"] ?? "" : v ?? "");

export default async function behance() {
  const xml = new XMLParser({ cdataPropName: false }).parse(await fetchText(FEED));
  const items = [xml.rss?.channel?.item ?? []].flat().map((it) => {
    const html = text(it["content:encoded"]) || text(it.description);
    const img = html.match(/<img[^>]+src=["']([^"']+)["']/i)?.[1];
    return {
      title: clean(text(it.title)),
      url: String(text(it.link)).trim(),
      image: img?.replace("/projects/404/", "/projects/808/") ?? null,
      meta: "Curated",
    };
  }).filter((x) => x.title && x.url.startsWith("http") && x.image);
  if (items.length < 3) throw new Error(`behance: only ${items.length} items in feed`);
  return items.slice(0, 3);
}
