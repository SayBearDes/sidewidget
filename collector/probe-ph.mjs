// Временная проба вариантов запроса к Product Hunt (запускается из probe.yml, потом удаляется).
const { PH_CLIENT_ID, PH_CLIENT_SECRET } = process.env;
const tok = await (await fetch("https://api.producthunt.com/v2/oauth/token", {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ client_id: PH_CLIENT_ID, client_secret: PH_CLIENT_SECRET, grant_type: "client_credentials" }),
})).json();

const gql = async (query) => {
  const r = await fetch("https://api.producthunt.com/v2/api/graphql", {
    method: "POST", headers: { "content-type": "application/json", authorization: `Bearer ${tok.access_token}` },
    body: JSON.stringify({ query }),
  });
  return r.json();
};
const F = "name votesCount createdAt featuredAt";
const now = new Date();
const variants = {
  "A featured RANKING": `{ posts(featured: true, order: RANKING, first: 5) { edges { node { ${F} } } } }`,
  "B featured VOTES 36h": `{ posts(featured: true, order: VOTES, postedAfter: "${new Date(now - 36 * 3600e3).toISOString()}", first: 5) { edges { node { ${F} } } } }`,
  "C current (VOTES 24h)": `{ posts(order: VOTES, postedAfter: "${new Date(now - 24 * 3600e3).toISOString()}", first: 5) { edges { node { ${F} } } } }`,
  "D featured FEATURED_AT": `{ posts(featured: true, order: FEATURED_AT, first: 5) { edges { node { ${F} } } } }`,
};
console.log("now", now.toISOString());
for (const [name, q] of Object.entries(variants)) {
  const j = await gql(q);
  console.log("==", name);
  if (j.errors) { console.log("ERR", JSON.stringify(j.errors).slice(0, 300)); continue; }
  for (const { node: n } of j.data.posts.edges) console.log(`  ${n.votesCount}\t${n.name}\tcreated ${n.createdAt}\tfeatured ${n.featuredAt}`);
}
