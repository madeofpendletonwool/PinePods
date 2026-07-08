// Podcast recommendation engine for the Discover page (#103).
//
// Design: WE own the math; PodcastIndex is only a candidate firehose (it has no
// "similar/recommended" endpoint). We build a per-user *taste profile* from the user's
// subscriptions weighted by engagement, generate candidates via category-filtered trending,
// and rank them with TF-IDF cosine similarity + a category-overlap boost + light recency/size
// priors. Fully explainable, no LLM. Results are cached per user (see migration 058); this
// module only computes — caching/serving lives in the handler and scheduler.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::models::{RecommendationTasteInput, RecommendedPodcast};
use std::collections::{HashMap, HashSet};

// How many of the user's top categories to pull trending candidates from.
const TOP_CATEGORIES: usize = 4;
// Candidates requested per category from PodcastIndex trending.
const CANDIDATES_PER_CATEGORY: u32 = 40;

// Blended-score weights (sum ~= 1.0). All component scores are normalized to ~0..1.
const W_COSINE: f64 = 0.55; // title+author+description TF-IDF cosine vs. taste vector
const W_CATEGORY: f64 = 0.30; // overlap with the user's weighted category profile
const W_RECENCY: f64 = 0.10; // is the show still active?
const W_SIZE: f64 = 0.05; // tiny "is this a real, established show" prior

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "you", "your", "our", "are", "was", "this", "that", "from",
    "have", "has", "had", "not", "but", "all", "can", "will", "get", "out", "about", "how",
    "why", "who", "what", "when", "where", "podcast", "podcasts", "show", "episode", "episodes",
    "new", "his", "her", "its", "their", "they", "them", "she", "him", "each", "every", "more",
    "most", "some", "any", "one", "two", "into", "over", "than", "then", "now", "just", "like",
];

// Split text into lowercase alphanumeric tokens, dropping short tokens and stopwords.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && !STOPWORDS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

// Term-frequency map for one document.
fn term_freq(tokens: &[String]) -> HashMap<String, f64> {
    let mut tf: HashMap<String, f64> = HashMap::new();
    for t in tokens {
        *tf.entry(t.clone()).or_insert(0.0) += 1.0;
    }
    tf
}

// Normalize a feed URL for identity comparison (case + trailing slash insensitive).
fn norm_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_lowercase()
}

// One PodcastIndex trending candidate, parsed from the search-service JSON.
struct Candidate {
    podcastindexid: Option<i64>,
    title: String,
    author: Option<String>,
    image: Option<String>,
    description: Option<String>,
    feedurl: Option<String>,
    categories: HashMap<String, String>,
    newest_item_pubdate: Option<i64>,
    episode_count: i64,
    tf: HashMap<String, f64>,
}

// Fetch category-filtered trending feeds from the search service. Degrades to an empty
// list on any failure so recommendations never hard-fail on an upstream hiccup.
async fn fetch_trending(cat: &str) -> Vec<serde_json::Value> {
    let url = crate::handlers::podcasts::search_service_url("/api/trending");
    let params = [("cat", cat.to_string()), ("max", CANDIDATES_PER_CATEGORY.to_string())];
    match reqwest::Client::new().get(&url).query(&params).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(v) => v
                .get("feeds")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default(),
            Err(e) => {
                tracing::warn!("Failed to parse trending response for '{}': {}", cat, e);
                Vec::new()
            }
        },
        Ok(resp) => {
            tracing::warn!("Trending request for '{}' returned {}", cat, resp.status());
            Vec::new()
        }
        Err(e) => {
            tracing::warn!("Trending request for '{}' failed: {}", cat, e);
            Vec::new()
        }
    }
}

// Parse PodcastIndex feed JSON into a Candidate (categories object -> name map).
fn parse_candidate(feed: &serde_json::Value) -> Option<Candidate> {
    let title = feed.get("title").and_then(|v| v.as_str())?.to_string();
    if title.is_empty() {
        return None;
    }
    let author = feed
        .get("author")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let description = feed
        .get("description")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let image = feed
        .get("image")
        .or_else(|| feed.get("artwork"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let feedurl = feed
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let podcastindexid = feed.get("id").and_then(|v| v.as_i64());

    let mut categories: HashMap<String, String> = HashMap::new();
    if let Some(obj) = feed.get("categories").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(name) = v.as_str() {
                if !name.is_empty() {
                    categories.insert(k.clone(), name.to_string());
                }
            }
        }
    }

    let newest_item_pubdate = feed.get("newestItemPubdate").and_then(|v| v.as_i64());
    let episode_count = feed.get("episodeCount").and_then(|v| v.as_i64()).unwrap_or(0);

    let mut text = title.clone();
    if let Some(a) = &author {
        text.push(' ');
        text.push_str(a);
    }
    if let Some(d) = &description {
        text.push(' ');
        text.push_str(d);
    }
    let tf = term_freq(&tokenize(&text));

    Some(Candidate {
        podcastindexid,
        title,
        author,
        image,
        description,
        feedurl,
        categories,
        newest_item_pubdate,
        episode_count,
        tf,
    })
}

// Build the recommendation list for a user. Returns an empty vec when the user has no
// subscriptions (nothing to personalize from) or when candidate generation yields nothing
// new — callers should fall back to plain trending in that case.
pub async fn generate_recommendations(
    db: &DatabasePool,
    user_id: i32,
    limit: usize,
) -> AppResult<Vec<RecommendedPodcast>> {
    let subs: Vec<RecommendationTasteInput> = db.get_recommendation_taste_inputs(user_id).await?;
    if subs.is_empty() {
        return Ok(Vec::new());
    }

    // --- Taste profile from subscriptions, weighted by engagement ---
    // e_p = (play_count + 1) * favorite_boost. play_count already reflects listening depth.
    let engagement = |p: &RecommendationTasteInput| -> f64 {
        (p.play_count as f64 + 1.0) * if p.is_favorite { 2.0 } else { 1.0 }
    };

    // Category weights (display name -> weight), plus a lowercase lookup for matching.
    let mut cat_weight: HashMap<String, f64> = HashMap::new();
    for p in &subs {
        let e = engagement(p);
        if let Some(cats) = &p.categories {
            for name in cats.values() {
                if !name.is_empty() {
                    *cat_weight.entry(name.clone()).or_insert(0.0) += e;
                }
            }
        }
    }
    // Normalize category weights so the top category == 1.0.
    let max_cat = cat_weight.values().cloned().fold(0.0_f64, f64::max);
    if max_cat > 0.0 {
        for w in cat_weight.values_mut() {
            *w /= max_cat;
        }
    }
    let cat_weight_lc: HashMap<String, f64> = cat_weight
        .iter()
        .map(|(k, v)| (k.to_lowercase(), *v))
        .collect();

    // Identity of already-subscribed feeds, so we never recommend what the user has.
    let mut sub_ids: HashSet<i64> = HashSet::new();
    let mut sub_urls: HashSet<String> = HashSet::new();
    for p in &subs {
        if let Some(id) = p.podcastindexid {
            if id > 0 {
                sub_ids.insert(id);
            }
        }
        sub_urls.insert(norm_url(&p.feedurl));
    }

    // Weighted taste term-frequency vector over sub title+author+description, and per-sub
    // token sets for document-frequency (used to derive IDF over the whole corpus below).
    let mut taste_tf: HashMap<String, f64> = HashMap::new();
    let mut doc_token_sets: Vec<HashSet<String>> = Vec::new();
    for p in &subs {
        let e = engagement(p);
        let mut text = p.podcastname.clone();
        if let Some(a) = &p.author {
            text.push(' ');
            text.push_str(a);
        }
        if let Some(d) = &p.description {
            text.push(' ');
            text.push_str(d);
        }
        let tokens = tokenize(&text);
        for (t, c) in term_freq(&tokens) {
            *taste_tf.entry(t).or_insert(0.0) += c * e;
        }
        doc_token_sets.push(tokens.into_iter().collect());
    }

    // --- Candidate generation: category-filtered trending for the user's top categories ---
    let mut top_cats: Vec<(String, f64)> = cat_weight.clone().into_iter().collect();
    top_cats.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    top_cats.truncate(TOP_CATEGORIES);

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new(); // dedup key across category fetches
    for (cat, _) in &top_cats {
        for feed in fetch_trending(cat).await {
            if let Some(c) = parse_candidate(&feed) {
                // Skip already-subscribed feeds.
                if let Some(id) = c.podcastindexid {
                    if id > 0 && sub_ids.contains(&id) {
                        continue;
                    }
                }
                if let Some(url) = &c.feedurl {
                    if sub_urls.contains(&norm_url(url)) {
                        continue;
                    }
                }
                // Dedup candidates appearing under multiple categories.
                let key = c
                    .podcastindexid
                    .map(|id| format!("id:{}", id))
                    .or_else(|| c.feedurl.as_ref().map(|u| format!("url:{}", norm_url(u))))
                    .unwrap_or_else(|| format!("title:{}", c.title.to_lowercase()));
                if seen.insert(key) {
                    candidates.push(c);
                }
            }
        }
    }

    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // --- IDF over the corpus of subs + candidates ---
    for c in &candidates {
        doc_token_sets.push(c.tf.keys().cloned().collect());
    }
    let n_docs = doc_token_sets.len() as f64;
    let mut df: HashMap<String, f64> = HashMap::new();
    for set in &doc_token_sets {
        for t in set {
            *df.entry(t.clone()).or_insert(0.0) += 1.0;
        }
    }
    let idf = |term: &str| -> f64 {
        let d = df.get(term).copied().unwrap_or(0.0);
        ((n_docs + 1.0) / (d + 1.0)).ln() + 1.0
    };

    // Taste TF-IDF vector + its norm.
    let taste_vec: HashMap<String, f64> = taste_tf
        .iter()
        .map(|(t, w)| (t.clone(), w * idf(t)))
        .collect();
    let taste_norm = taste_vec.values().map(|v| v * v).sum::<f64>().sqrt();

    // --- Score each candidate ---
    let now = chrono::Utc::now().timestamp();
    let mut scored: Vec<RecommendedPodcast> = Vec::new();
    for c in &candidates {
        // Cosine similarity of TF-IDF vectors.
        let cosine = if taste_norm > 0.0 {
            let mut dot = 0.0;
            let mut cand_norm_sq = 0.0;
            for (t, tf) in &c.tf {
                let w = tf * idf(t);
                cand_norm_sq += w * w;
                if let Some(tw) = taste_vec.get(t) {
                    dot += w * tw;
                }
            }
            let cand_norm = cand_norm_sq.sqrt();
            if cand_norm > 0.0 {
                dot / (taste_norm * cand_norm)
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Category overlap: best-matching category weight, and remember it for the reason.
        let mut cat_score = 0.0;
        let mut best_cat: Option<String> = None;
        for name in c.categories.values() {
            if let Some(w) = cat_weight_lc.get(&name.to_lowercase()) {
                if *w > cat_score {
                    cat_score = *w;
                    best_cat = Some(name.clone());
                }
            }
        }

        // Recency: linear decay over the last year of the newest episode.
        let recency = match c.newest_item_pubdate {
            Some(ts) if ts > 0 => {
                let age_days = (now - ts) as f64 / 86400.0;
                (1.0 - age_days / 365.0).clamp(0.0, 1.0)
            }
            _ => 0.5,
        };

        // Size prior: log-scaled episode count, saturating around ~1000 episodes.
        let size = ((c.episode_count.max(0) as f64 + 1.0).log10() / 3.0).clamp(0.0, 1.0);

        let score = W_COSINE * cosine + W_CATEGORY * cat_score + W_RECENCY * recency + W_SIZE * size;

        let reason = match &best_cat {
            Some(cat) => format!("Because you listen to {}", cat),
            None => "Popular right now".to_string(),
        };

        scored.push(RecommendedPodcast {
            podcastindexid: c.podcastindexid,
            title: c.title.clone(),
            author: c.author.clone(),
            image: c.image.clone(),
            description: c.description.clone(),
            feedurl: c.feedurl.clone(),
            categories: c.categories.clone(),
            score,
            reason,
        });
    }

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored)
}
