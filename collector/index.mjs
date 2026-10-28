// Собирает все источники в один data.json.
// Запуск: node index.mjs [out=out/data.json] ; PREV_JSON=<путь> — прошлый data.json для подстановки при сбое источника.
import fs from "node:fs";
import path from "node:path";
import awwwards from "./sources/awwwards.mjs";
import behance from "./sources/behance.mjs";
import producthunt from "./sources/producthunt.mjs";
import news from "./sources/news.mjs";

const SOURCES = { awwwards, behance, producthunt, news };
const out = process.argv[2] ?? "out/data.json";
const prevPath = process.env.PREV_JSON;
const prev = prevPath && fs.existsSync(prevPath) ? JSON.parse(fs.readFileSync(prevPath, "utf8")) : {};

const data = { updated: new Date().toISOString() };
let failed = 0;
for (const [key, fn] of Object.entries(SOURCES)) {
  try {
    data[key] = await fn();
    console.log(`ok   ${key}: ${data[key].length}`);
  } catch (e) {
    failed++;
    data[key] = prev[key] ?? [];
    console.warn(`FAIL ${key}: ${e.message}${prev[key] ? " (kept previous)" : ""}`);
  }
}
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(data, null, 1));
console.log(`wrote ${out}`);
if (failed === Object.keys(SOURCES).length) process.exit(1);
