import { XMLParser } from "fast-xml-parser";
import { fetchText, clean } from "../lib.mjs";

// 1) Официальный GraphQL API v2 — топ дня по голосам. Нужен developer-токен в PH_TOKEN
//    (https://www.producthunt.com/v2/oauth/applications -> create app -> "Developer Token").
// 2) Без токена — запасной вариант: публичная Atom-лента (3 самых свежих запуска, БЕЗ рейтинга и голосов).
const QUERY = `query($after: DateTime!) {
  posts(order: VOTES, postedAfter: $after, first: 3) {
    edges { node { name tagline url votesCount thumbnail { url } } }
  }
}`;

async function viaApi(token) {
  const after = new Date(Date.now() - 24 * 3600 * 1000).toISOString();
  const res = await fetch("https://api.producthunt.com/v2/api/graphql", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify({ query: QUERY, variables: { after } }),
    signal: AbortSignal.timeout(30_000),
  });
  const json = await res.json();
  if (!res.ok || json.errors) throw new Error(`Product Hunt API: ${res.status} ${JSON.stringify(json.errors ?? "").slice(0, 200)}`);
  return json.data.posts.edges.map(({ node: n }) => ({
    title: n.name,
    text: n.tagline,
    url: n.url.split("?")[0],
    image: n.thumbnail?.url ?? null,
    meta: `▲ ${n.votesCount}`,
  }));
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
