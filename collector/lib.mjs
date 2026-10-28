export const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

export async function fetchText(url, init = {}) {
  const res = await fetch(url, {
    ...init,
    headers: { "user-agent": UA, "accept-language": "en-US,en;q=0.8", ...init.headers },
    signal: AbortSignal.timeout(30_000),
  });
  if (!res.ok) throw new Error(`${url} -> HTTP ${res.status}`);
  return res.text();
}

const ENT = { "&quot;": "\"", "&#39;": "'", "&apos;": "'", "&lt;": "<", "&gt;": ">", "&nbsp;": " " };
export const clean = (s) =>
  String(s ?? "").replace(/<[^>]+>/g, "").replace(/&(quot|#39|apos|lt|gt|nbsp);/g, (m) => ENT[m]).replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
