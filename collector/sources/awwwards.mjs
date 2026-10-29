import * as cheerio from "cheerio";
import { fetchText } from "../lib.mjs";

// Лента наград /websites/: сначала Sites of the Day, затем Honorable Mention.
// Каждая карточка — <li data-collectable-model-value="{json: slug, title, images, createdAt}">,
// тип награды и её дата видны только в тексте карточки («Site Of The Day Oct 29, 2026»).
// Раскладка виджета: большая карточка — свежайший SOTD, две маленькие — свежайшие Honorable Mention.
const PAGE = "https://www.awwwards.com/websites/";
const IMG = "https://assets.awwwards.com/awards/media/cache/thumb_880_660/";
const KINDS = [
  ["sotd", /Site Of The Day/i, "Site of the Day"],
  ["hm", /Honorable Mention/i, "Honorable Mention"],
];

export default async function awwwards() {
  const $ = cheerio.load(await fetchText(PAGE));
  const cards = [];
  $("li.js-collectable[data-collectable-model-value]").each((_, li) => {
    try {
      const m = JSON.parse($(li).attr("data-collectable-model-value"));
      if (m.type !== "submission" || !m.slug || !m.images?.thumbnail) return;
      const text = $(li).text().replace(/\s+/g, " ");
      const kind = KINDS.find(([, re]) => re.test(text));
      if (!kind) return;
      const date = text.match(/([A-Z][a-z]{2} \d{2}, \d{4})/)?.[1] ?? "";
      cards.push({
        kind: kind[0],
        createdAt: m.createdAt ?? 0,
        title: m.title,
        url: `https://www.awwwards.com/sites/${m.slug}`,
        image: IMG + m.images.thumbnail,
        meta: `${kind[2]}${date ? " · " + date : ""}`,
      });
    } catch { /* пропускаем битую карточку */ }
  });

  const newest = (kind, n) => cards.filter((c) => c.kind === kind).sort((a, b) => b.createdAt - a.createdAt).slice(0, n);
  const items = [...newest("sotd", 1), ...newest("hm", 2)];
  if (items.length < 3) throw new Error(`awwwards: need 1 SOTD + 2 HM, found ${items.length} of ${cards.length} cards, markup changed?`);
  return items.map(({ kind, createdAt, ...rest }) => rest);
}
