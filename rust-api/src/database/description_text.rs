//! `DescriptionText`: the visible words of an episode or video description, which episode search
//! matches. Written with each episode; this fills it for rows stored before the column existed.

use super::*;
use crate::services::search_text::plain_text;

const BACKFILL_BATCH: i64 = 500;

impl DatabasePool {
    /// Fill `DescriptionText` on every episode and video that lacks it. Returns the rows filled.
    pub async fn backfill_description_text(&self) -> AppResult<usize> {
        let mut filled = 0;
        for videos in [false, true] {
            loop {
                let rows = self.description_text_missing(videos).await?;
                let done = (rows.len() as i64) < BACKFILL_BATCH;
                if !rows.is_empty() {
                    let texts: Vec<(i32, String)> = rows
                        .into_iter()
                        .map(|(id, d)| (id, plain_text(d.as_deref().unwrap_or(""))))
                        .collect();
                    self.store_description_text(videos, &texts).await?;
                    filled += texts.len();
                }
                if done {
                    break;
                }
            }
        }
        Ok(filled)
    }

    async fn description_text_missing(&self, videos: bool) -> AppResult<Vec<(i32, Option<String>)>> {
        let rows = dialect_match!(self, {
            pg(pool) => {
                let sql = if videos {
                    r#"SELECT videoid AS id, videodescription AS description FROM "YouTubeVideos"
                       WHERE descriptiontext IS NULL ORDER BY videoid LIMIT $1"#
                } else {
                    r#"SELECT episodeid AS id, episodedescription AS description FROM "Episodes"
                       WHERE descriptiontext IS NULL ORDER BY episodeid LIMIT $1"#
                };
                let mut out = Vec::new();
                for row in q::query(sql).bind(BACKFILL_BATCH).fetch_all(pool).await? {
                    out.push((row.try_get("id")?, row.try_get("description")?));
                }
                out
            },
            mysql(pool) => {
                let sql = if videos {
                    "SELECT VideoID AS id, VideoDescription AS description FROM YouTubeVideos
                     WHERE DescriptionText IS NULL ORDER BY VideoID LIMIT ?"
                } else {
                    "SELECT EpisodeID AS id, EpisodeDescription AS description FROM Episodes
                     WHERE DescriptionText IS NULL ORDER BY EpisodeID LIMIT ?"
                };
                let mut out = Vec::new();
                for row in sqlx::query(sql).bind(BACKFILL_BATCH).fetch_all(pool).await? {
                    out.push((row.try_get("id")?, row.try_get("description")?));
                }
                out
            },
        });
        Ok(rows)
    }

    async fn store_description_text(&self, videos: bool, texts: &[(i32, String)]) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                let sql = if videos {
                    r#"UPDATE "YouTubeVideos" SET descriptiontext = $1 WHERE videoid = $2"#
                } else {
                    r#"UPDATE "Episodes" SET descriptiontext = $1 WHERE episodeid = $2"#
                };
                let mut tx = q::begin(pool).await?;
                for (id, text) in texts {
                    q::query(sql).bind(text).bind(*id).execute(&mut *tx).await?;
                }
                tx.commit().await?;
            },
            mysql(pool) => {
                let sql = if videos {
                    "UPDATE YouTubeVideos SET DescriptionText = ? WHERE VideoID = ?"
                } else {
                    "UPDATE Episodes SET DescriptionText = ? WHERE EpisodeID = ?"
                };
                let mut tx = pool.begin().await?;
                for (id, text) in texts {
                    sqlx::query(sql).bind(text).bind(*id).execute(&mut *tx).await?;
                }
                tx.commit().await?;
            },
        });
        Ok(())
    }
}
