//! Сбор данных прямо в виджете (раньше это был collector/ на Node). Каждый источник возвращает
//! блок в том же формате, что и data.json: [{title, url, image, meta, text?, date?}].
use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Datelike, Days, NaiveDate, SecondsFormat, Utc};
use roxmltree::{Document, Node, ParsingOptions};
use scraper::{Html, Selector};
use serde_json::{json, Value};

pub type Items = Result<Vec<Value>, String>;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";
const TEXT_LEN: usize = 150;

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(0)
        .build()
        .expect("http client")
}

async fn get_text(c: &reqwest::Client, url: &str) -> Result<String, String> {
    let r = c.get(url).send().await.map_err(|e| format!("{url}: {e}"))?;
    let r = r.error_for_status().map_err(|e| format!("{url}: {e}"))?;
    r.text().await.map_err(|e| format!("{url}: {e}"))
}

// ---------- текст ----------

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(j) = rest.find(';').filter(|&j| j <= 10) {
            let ent = &rest[1..j];
            let rep: Option<String> = match ent {
                "amp" => Some("&".into()),
                "lt" => Some("<".into()),
                "gt" => Some(">".into()),
                "quot" => Some("\"".into()),
                "apos" | "#39" => Some("'".into()),
                "nbsp" => Some(" ".into()),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32).map(|c| c.to_string())
                }
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string()),
                _ => None,
            };
            if let Some(r) = rep {
                out.push_str(&r);
                rest = &rest[j + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Без HTML-тегов и сущностей, с одинарными пробелами.
fn clean(s: &str) -> String {
    decode_entities(&strip_tags(s)).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cut(s: &str) -> String {
    if s.chars().count() <= TEXT_LEN {
        return s.to_string();
    }
    let t: String = s.chars().take(TEXT_LEN).collect();
    let t = match t.rfind(' ') {
        Some(i) => t[..i].to_string(),
        None => t,
    };
    format!("{t}…")
}

fn first_img_src(html: &str) -> Option<String> {
    let rest = &html[html.find("<img")?..];
    let after = &rest[rest.find("src=")? + 4..];
    let q = after.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let end = after[1..].find(q)?;
    Some(after[1..1 + end].to_string())
}

fn abs_url(u: String) -> String {
    if let Some(rest) = u.strip_prefix("//") { format!("https://{rest}") } else { u }
}

fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    DateTime::parse_from_rfc2822(s).or_else(|_| DateTime::parse_from_rfc3339(s)).ok().map(|d| d.with_timezone(&Utc))
}

fn iso(d: DateTime<Utc>) -> String {
    d.to_rfc3339_opts(SecondsFormat::Millis, true)
}

// ---------- RSS / Atom ----------

struct Entry {
    title: String,
    url: String,
    text: String,
    html: String,
    image: Option<String>,
    date: Option<DateTime<Utc>>,
}

fn txt(n: Node<'_, '_>) -> String {
    n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect()
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

fn child_txt(n: Node<'_, '_>, name: &str) -> String {
    child(n, name).map(txt).unwrap_or_default()
}

fn first_nonempty(n: Node<'_, '_>, names: &[&str]) -> String {
    names.iter().map(|k| child_txt(n, k)).find(|s| !s.trim().is_empty()).unwrap_or_default()
}

/// Обрезает ленту после n записей: у Smashing это 1,3 МБ, а нужны первые пять-десять.
fn head(xml: &str, n: usize) -> String {
    let (close, tail) = if xml.contains("<rss") { ("</item>", "</channel></rss>") } else { ("</entry>", "</feed>") };
    let mut end = 0;
    for _ in 0..n {
        match xml[end..].find(close) {
            Some(i) => end += i + close.len(),
            None => return xml.to_string(),
        }
    }
    format!("{}{tail}", &xml[..end])
}

fn entries(xml: &str, n: usize) -> Result<Vec<Entry>, String> {
    let xml = head(xml, n);
    let opts = ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = Document::parse_with_options(&xml, opts).map_err(|e| format!("xml: {e}"))?;
    let list = doc
        .descendants()
        .filter(|n| n.is_element() && matches!(n.tag_name().name(), "item" | "entry"))
        .map(|it| {
            let url = it
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "link")
                .find_map(|l| match l.attribute("href") {
                    Some(h) => (l.attribute("rel") != Some("self")).then(|| h.to_string()),
                    None => Some(txt(l).trim().to_string()).filter(|t| !t.is_empty()),
                })
                .unwrap_or_default();
            let html = first_nonempty(it, &["encoded", "content", "description", "summary"]);
            let desc = first_nonempty(it, &["description", "summary", "encoded", "content"]);
            let image = ["thumbnail", "content", "enclosure"]
                .iter()
                .find_map(|k| {
                    it.children()
                        .filter(|c| c.is_element() && c.tag_name().name() == *k)
                        .find_map(|c| c.attribute("url"))
                        .map(String::from)
                })
                .or_else(|| first_img_src(&html))
                .map(abs_url);
            let date = parse_date(&first_nonempty(it, &["pubDate", "date", "published", "updated"]));
            Entry { title: clean(&child_txt(it, "title")), url, text: cut(&clean(&desc)), html, image, date }
        })
        .collect();
    Ok(list)
}

// ---------- News ----------

const FEEDS: [(&str, &str); 3] = [
    ("Sidebar", "https://sidebar.io/feed.xml"),
    ("Smashing Magazine", "https://www.smashingmagazine.com/feed/"),
    ("NN/g", "https://www.nngroup.com/feed/rss/"),
];

async fn feed(c: &reqwest::Client, url: &str) -> Result<Vec<Entry>, String> {
    entries(&get_text(c, url).await?, 15)
}

/// og:image / og:description со страницы статьи, если в ленте их не было.
async fn page_meta(c: &reqwest::Client, url: &str) -> (Option<String>, Option<String>) {
    let Ok(Ok(html)) = tokio::time::timeout(Duration::from_secs(10), get_text(c, url)).await else { return (None, None) };
    let doc = Html::parse_document(&html);
    let meta = |p: &str| -> Option<String> {
        let sel = Selector::parse(&format!("meta[property=\"{p}\"], meta[name=\"{p}\"]")).ok()?;
        doc.select(&sel).next()?.value().attr("content").map(String::from)
    };
    let image = meta("og:image").or_else(|| meta("twitter:image"));
    let text = meta("og:description").or_else(|| meta("description"));
    (image, text)
}

pub async fn news(c: &reqwest::Client) -> Items {
    let (a, b, d) = tokio::join!(feed(c, FEEDS[0].1), feed(c, FEEDS[1].1), feed(c, FEEDS[2].1));
    let mut all: Vec<(&str, Entry)> = Vec::new();
    for (i, r) in [a, b, d].into_iter().enumerate() {
        match r {
            Ok(v) => all.extend(v.into_iter().map(|e| (FEEDS[i].0, e))),
            Err(e) => eprintln!("news: {}: {e}", FEEDS[i].0),
        }
    }
    all.retain(|(_, e)| !e.title.is_empty() && e.url.starts_with("http") && e.date.is_some());
    all.sort_by(|x, y| y.1.date.cmp(&x.1.date));

    let mut per: HashMap<&str, usize> = HashMap::new();
    let mut picked: Vec<(&str, Entry)> = Vec::new();
    for (src, e) in all {
        let n = per.entry(src).or_insert(0);
        if *n < 2 {
            *n += 1;
            picked.push((src, e));
        }
        if picked.len() == 3 {
            break;
        }
    }
    if picked.is_empty() {
        return Err("news: no items".into());
    }

    let mut out = Vec::new();
    for (src, mut e) in picked {
        if let Some(i) = e.url.find("?utm_") {
            e.url.truncate(i);
        }
        if e.image.is_none() || e.text.is_empty() {
            let (img, text) = page_meta(c, &e.url).await;
            if e.image.is_none() {
                e.image = img.map(|u| abs_url(u));
            }
            if e.text.is_empty() {
                e.text = cut(&clean(&text.unwrap_or_default()));
            }
        }
        out.push(json!({
            "title": e.title, "url": e.url, "text": e.text, "image": e.image,
            "meta": src, "date": e.date.map(iso),
        }));
    }
    Ok(out)
}

// ---------- Behance ----------

/// Публичный RSS «Featured Projects» — это и есть Curated-галерея Behance.
pub async fn behance(c: &reqwest::Client) -> Items {
    let xml = get_text(c, "https://www.behance.net/feeds/projects").await?;
    let items: Vec<Value> = entries(&xml, 12)?
        .into_iter()
        .filter(|e| !e.title.is_empty() && e.url.starts_with("http"))
        .filter_map(|e| {
            let image = e.image?.replace("/projects/404/", "/projects/808/");
            Some(json!({ "title": e.title, "url": e.url, "image": image, "meta": "Curated" }))
        })
        .take(3)
        .collect();
    if items.len() < 3 {
        return Err(format!("behance: only {} items in feed", items.len()));
    }
    Ok(items)
}

// ---------- Awwwards ----------

/// Лента наград /websites/: сначала Sites of the Day, затем Honorable Mention. Тип награды и дата видны
/// только в тексте карточки. Большая карточка — свежайший SOTD, две маленькие — свежайшие HM.
pub async fn awwwards(c: &reqwest::Client) -> Items {
    const IMG: &str = "https://assets.awwwards.com/awards/media/cache/thumb_880_660/";
    const KINDS: [(&str, &str); 2] = [("site of the day", "Site of the Day"), ("honorable mention", "Honorable Mention")];

    let html = get_text(c, "https://www.awwwards.com/websites/").await?;
    let doc = Html::parse_document(&html);
    let sel = Selector::parse("li.js-collectable[data-collectable-model-value]").map_err(|e| e.to_string())?;

    // (вид награды, createdAt, элемент)
    let mut cards: Vec<(usize, i64, Value)> = Vec::new();
    for li in doc.select(&sel) {
        let Some(raw) = li.value().attr("data-collectable-model-value") else { continue };
        let Ok(m) = serde_json::from_str::<Value>(raw) else { continue };
        let (Some("submission"), Some(slug), Some(thumb)) =
            (m["type"].as_str(), m["slug"].as_str(), m["images"]["thumbnail"].as_str())
        else {
            continue;
        };
        let text = li.text().collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
        let low = text.to_ascii_lowercase();
        let Some(kind) = KINDS.iter().position(|(needle, _)| low.contains(needle)) else { continue };
        let date = date_after(&text, &low, KINDS[kind].0);
        cards.push((
            kind,
            m["createdAt"].as_i64().unwrap_or(0),
            json!({
                "title": m["title"].as_str().unwrap_or(slug),
                "url": format!("https://www.awwwards.com/sites/{slug}"),
                "image": format!("{IMG}{thumb}"),
                "meta": if date.is_empty() { KINDS[kind].1.to_string() } else { format!("{} · {date}", KINDS[kind].1) },
            }),
        ));
    }

    let newest = |kind: usize, n: usize| -> Vec<Value> {
        let mut v: Vec<&(usize, i64, Value)> = cards.iter().filter(|c| c.0 == kind).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.into_iter().take(n).map(|c| c.2.clone()).collect()
    };
    let mut items = newest(0, 1);
    items.extend(newest(1, 2));
    if items.len() < 3 {
        return Err(format!("awwwards: need 1 SOTD + 2 HM, got {} of {} cards, markup changed?", items.len(), cards.len()));
    }
    Ok(items)
}

/// «Oct 29, 2026» сразу после названия награды в тексте карточки.
fn date_after(text: &str, low: &str, needle: &str) -> String {
    let Some(i) = low.find(needle) else { return String::new() };
    let rest = text.get(i + needle.len()..).unwrap_or("").trim_start();
    let d: Vec<char> = rest.chars().take(12).collect();
    if d.len() == 12 && d[0].is_ascii_uppercase() && d[3] == ' ' && d[6] == ',' && d[7] == ' ' {
        d.into_iter().collect()
    } else {
        String::new()
    }
}

// ---------- Product Hunt ----------

pub struct PhKeys {
    pub client_id: String,
    pub client_secret: String,
    pub token: String,
}

const PH_QUERY: &str = "query($after: DateTime!, $before: DateTime!) {
  posts(featured: true, order: VOTES, postedAfter: $after, postedBefore: $before, first: 3) {
    edges { node { name tagline url votesCount thumbnail { url } } }
  }
}";

fn nth_sunday(year: i32, month: u32, n: u64) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid date");
    let to_sunday = (7 - first.weekday().num_days_from_sunday() as u64) % 7;
    first + Days::new(to_sunday + 7 * (n - 1))
}

/// Начало текущих суток по Тихоокеанскому времени (день Product Hunt) в UTC.
/// Летнее время США: со 2-го воскресенья марта до 1-го воскресенья ноября.
fn pt_day_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let y = now.year();
    let dst_from = nth_sunday(y, 3, 2).and_hms_opt(10, 0, 0).expect("time").and_utc();
    let dst_to = nth_sunday(y, 11, 1).and_hms_opt(9, 0, 0).expect("time").and_utc();
    let offset = chrono::Duration::hours(if now >= dst_from && now < dst_to { 7 } else { 8 });
    (now - offset).date_naive().and_hms_opt(0, 0, 0).expect("time").and_utc() + offset
}

async fn ph_token(c: &reqwest::Client, keys: &PhKeys) -> Result<String, String> {
    if !keys.token.is_empty() {
        return Ok(keys.token.clone());
    }
    if keys.client_id.is_empty() || keys.client_secret.is_empty() {
        return Err("producthunt: no keys in config.json".into());
    }
    let r = c
        .post("https://api.producthunt.com/v2/oauth/token")
        .json(&json!({"client_id": keys.client_id, "client_secret": keys.client_secret, "grant_type": "client_credentials"}))
        .send()
        .await
        .map_err(|e| format!("producthunt token: {e}"))?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    v["access_token"].as_str().map(String::from).ok_or_else(|| format!("producthunt token: HTTP {status} {}", v["error"]))
}

async fn ph_posts(c: &reqwest::Client, token: &str, after: DateTime<Utc>, before: DateTime<Utc>) -> Result<Vec<(i64, Value)>, String> {
    let body = json!({"query": PH_QUERY, "variables": {
        "after": after.to_rfc3339_opts(SecondsFormat::Secs, true),
        "before": before.to_rfc3339_opts(SecondsFormat::Secs, true),
    }});
    let r = c
        .post("https://api.producthunt.com/v2/api/graphql")
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("producthunt api: {e}"))?;
    let status = r.status();
    let v: Value = r.json().await.map_err(|e| format!("producthunt api: {e}"))?;
    if !status.is_success() || !v["errors"].is_null() {
        return Err(format!("producthunt api: HTTP {status} {}", v["errors"].to_string().chars().take(200).collect::<String>()));
    }
    Ok(v["data"]["posts"]["edges"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|e| {
            let n = &e["node"];
            let votes = n["votesCount"].as_i64().unwrap_or(0);
            let url = n["url"].as_str().unwrap_or("").split('?').next().unwrap_or("").to_string();
            (votes, json!({"title": n["name"], "text": n["tagline"], "url": url, "image": n["thumbnail"]["url"], "meta": format!("▲ {votes}")}))
        })
        .collect())
}

/// Топ дня по голосам среди featured. Голоса запусков текущего дня первые часы приходят нулями,
/// поэтому без голосов берём итоговый топ вчерашнего дня.
pub async fn producthunt(c: &reqwest::Client, keys: &PhKeys) -> Items {
    let token = ph_token(c, keys).await?;
    let now = Utc::now();
    let start = pt_day_start(now);
    let mut posts = ph_posts(c, &token, start, now).await?;
    if !posts.iter().any(|p| p.0 > 0) {
        posts = ph_posts(c, &token, start - chrono::Duration::hours(24), start).await?;
    }
    if posts.len() < 3 {
        return Err(format!("producthunt api: only {} posts", posts.len()));
    }
    Ok(posts.into_iter().map(|p| p.1).collect())
}

/// Запасной вариант без ключей: публичная Atom-лента (3 свежих запуска, без голосов и картинок).
pub async fn producthunt_feed(c: &reqwest::Client) -> Items {
    let xml = get_text(c, "https://www.producthunt.com/feed").await?;
    let items: Vec<Value> = entries(&xml, 3)?
        .into_iter()
        .map(|e| {
            let text = e.html.find("<p>").map(|i| clean(e.html[i + 3..].split("</p>").next().unwrap_or(""))).unwrap_or_default();
            json!({"title": e.title, "url": e.url, "text": text, "image": Value::Null, "meta": "New"})
        })
        .collect();
    if items.len() < 3 {
        return Err(format!("producthunt feed: only {} entries", items.len()));
    }
    Ok(items)
}

// ---------- всё сразу ----------

pub struct Collected {
    pub awwwards: Items,
    pub behance: Items,
    pub news: Items,
    pub producthunt: Items,
}

pub async fn collect_all(c: &reqwest::Client, keys: &PhKeys) -> Collected {
    let (awwwards, behance, news, producthunt) = tokio::join!(awwwards(c), behance(c), news(c), producthunt(c, keys));
    Collected { awwwards, behance, news, producthunt }
}
