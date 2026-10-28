import * as cheerio from "cheerio";
import { fetchText } from "../lib.mjs";

// Каждая карточка — <li data-collectable-model-value="{json со slug, title, thumbnail, tags}">
const PAGE = "https://www.awwwards.com/websites/sites_of_the_day/";
const IMG = "https://assets.awwwards.com/awards/media/cache/thumb_880_660/";

export default async function awwwards() {
  const $ = cheerio.load(await fetchText(PAGE));
  const items = [];
  $("li.js-collectable[data-collectable-model-value]").each((_, li) => {
    if (items.length >= 3) return;
    try {
      const m = JSON.parse($(li).attr("data-collectable-model-value"));
      if (m.type !== "submission" || !m.slug || !m.images?.thumbnail) return;
      items.push({
        title: m.title,
        url: `https://www.awwwards.com/sites/${m.slug}`,
        image: IMG + m.images.thumbnail,
        meta: (m.tags ?? []).slice(0, 2).join(" · ") || "Site of the Day",
      });
    } catch { /* пропускаем битую карточку */ }
  });
  if (items.length < 3) throw new Error(`awwwards: found ${items.length} cards, markup changed?`);
  return items;
}
