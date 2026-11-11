//! Searching inside the transcripts of the caller's episodes.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    database::TranscriptSearchEpisode,
    error::AppError,
    handlers::{extract_api_key, require_valid_api_key},
    services::transcript_search::{find_moments, parse_query, wanted, Moment},
    AppState,
};

/// Moments returned per episode.
const MAX_MOMENTS: usize = 12;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct TranscriptSearchQuery {
    /// Words, "quoted phrases" and -excluded words. Every word that isn't excluded must appear.
    pub q: String,
    /// Episodes per page (default 20, at most 50).
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TranscriptSearchHit {
    pub episode: TranscriptSearchEpisode,
    /// `generated` (made on this server) or `feed` (published by the podcast).
    pub source: String,
    /// How many times the search terms appear in the transcript.
    pub mentions: usize,
    /// Where they appear, in order, close mentions merged.
    pub moments: Vec<Moment>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TranscriptSearchResponse {
    pub results: Vec<TranscriptSearchHit>,
    /// Matching episodes in all pages.
    pub total: i64,
    /// The words and phrases clients should highlight, lowercased.
    pub terms: Vec<String>,
    /// How many of your episodes have a searchable transcript.
    pub searchable_episodes: i64,
}

/// Search the transcripts of your episodes, generated and published by the feed, newest
/// episodes first, with the moments each match was said.
#[utoipa::path(
    get,
    path = "/transcripts/search",
    tag = "episodes",
    summary = "Search inside transcripts",
    params(TranscriptSearchQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Matching episodes", body = TranscriptSearchResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn search_transcripts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<TranscriptSearchQuery>,
) -> Result<Json<TranscriptSearchResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    require_valid_api_key(&state, &api_key).await?;
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    let limit = params.limit.unwrap_or(20).clamp(1, 50);
    let offset = params.offset.unwrap_or(0).max(0);
    let terms = parse_query(&params.q);

    let (matches, total) = state.db_pool.search_transcripts(user_id, &terms, limit, offset).await?;
    let searchable_episodes = state.db_pool.searchable_transcript_count(user_id).await?;

    let results = matches
        .into_iter()
        .map(|m| {
            let (moments, mentions) = find_moments(m.segments.as_deref(), m.text.as_deref(), &terms, MAX_MOMENTS);
            TranscriptSearchHit { episode: m.episode, source: m.source, mentions, moments }
        })
        .collect();

    Ok(Json(TranscriptSearchResponse {
        results,
        total,
        terms: wanted(&terms).into_iter().map(|t| t.text.clone()).collect(),
        searchable_episodes,
    }))
}
