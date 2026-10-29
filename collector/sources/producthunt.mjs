import { XMLParser } from "fast-xml-parser";
import { fetchText, clean } from "../lib.mjs";

// 1) Официальный GraphQL API v2 — топ дня по голосам среди featured-запусков (клиентские ключи, см. ниже).
// 2) Без ключей — запасной вариант: публичная Atom-лента (3 самых свежих запуска, БЕЗ рейтинга и голосов).
//
// Нюансы API (проверено): день PH начинается в 00:00 по Тихоокеанскому времени (07:00 UTC летом),
// а голоса запусков текущего дня первые часы приходят как 0. Поэтому берём текущий день, если в нём
// уже есть голоса, иначе итоговый топ вчерашнего дня. Фильтр featured отсекает неотобранные посты.
const QUERY = `query($after: DateTime!, $before: DateTime!) {
  posts(featured: true, order: VOTES, postedAfter: $after, postedBefore: $before, first: 3) {
    edges { node { name tagline url votesCount thumbnail { url } } }
  }
}`;

// Начало текущих суток по Тихоокеанскому времени (с учётом летнего/зимнего), в UTC.
function ptDayStart(now = new Date()) {
  const p = Object.fromEntries(new Intl.DateTimeFormat("en-US", {
    timeZone: "America/Los_Angeles", hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
  }).formatToParts(now).map((x) => [x.type, x.value]));
  const local = Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second);
  const offset = local - Math.floor(now.getTime() / 1000) * 1000; // отрицательный: PT отстаёт от UTC
  return new Date(Date.UTC(p.year, p.month - 1, p.day) - offset);
}

async function topPosts(token, after, before) {
  const res = await fetch("https://api.producthunt.com/v2/api/graphql", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify({ query: QUERY, variables: { after: after.toISOString(), before: before.toISOString() } }),
    signal: AbortSignal.timeout(30_000),
  });
  const json = await res.json();
  if (!res.ok || json.errors) throw new Error(`Product Hunt API: ${res.status} ${JSON.stringify(json.errors ?? "").slice(0, 200)}`);
  return json.data.posts.edges.map(({ node: n }) => ({
    title: n.name,
    text: n.tagline,
    url: n.url.split("?")[0],
    image: n.thumbnail?.url ?? null,
    votes: n.votesCount,
    meta: `▲ ${n.votesCount}`,
  }));
}

async function viaApi(token) {
  const now = new Date();
  const start = ptDayStart(now);
  let posts = await topPosts(token, start, now);
  if (!posts.some((p) => p.votes > 0)) {
    console.warn("producthunt: today's votes are not available yet, using yesterday's top");
    posts = await topPosts(token, new Date(start.getTime() - 24 * 3600e3), start);
  }
  if (posts.length < 3) throw new Error(`Product Hunt API: only ${posts.length} posts`);
  return posts.map(({ votes, ...rest }) => rest);
}

async function viaFeed() {
  const xml = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: "@" }).parse(await fetchText("https://www.producthunt.com/feed"));
  // страницы продуктов закрыты от ботов (403), поэтому в запасном варианте без картинок
  return [xml.feed?.entry ?? []].flat().slice(0, 3).map((e) => {
    const html = String(e.content?.["#text"] ?? e.content ?? "");
    return {
      title: clean(e.title),
      text: clean(html.match(/<p>([\s\S]*?)<\/p>/)?.[1] ?? ""),
      url: [e.link].flat().find((l) => l["@rel"] === "alternate")?.["@href"],
      image: null,
      meta: "New",
    };
  });
}

// API Key + API Secret приложения -> access token (OAuth client credentials, публичный доступ).
async function clientToken(id, secret) {
  const res = await fetch("https://api.producthunt.com/v2/oauth/token", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ client_id: id, client_secret: secret, grant_type: "client_credentials" }),
    signal: AbortSignal.timeout(30_000),
  });
  const json = await res.json().catch(() => ({}));
  if (!res.ok || !json.access_token) throw new Error(`Product Hunt token: HTTP ${res.status} ${json.error ?? ""}`);
  return json.access_token;
}

export default async function producthunt() {
  const { PH_TOKEN, PH_CLIENT_ID, PH_CLIENT_SECRET } = process.env;
  if (PH_TOKEN) return viaApi(PH_TOKEN);
  if (PH_CLIENT_ID && PH_CLIENT_SECRET) return viaApi(await clientToken(PH_CLIENT_ID, PH_CLIENT_SECRET));
  console.warn("producthunt: no PH_CLIENT_ID/PH_CLIENT_SECRET (or PH_TOKEN), using public feed (latest launches, not top by votes)");
  return viaFeed();
}
