//! Per-user preferences: theme, timezone, start page, language, playback defaults.

use super::*;

#[derive(Debug, Clone)]
pub struct UserAutoComplete {
    pub user_id: i32,
    pub auto_complete_seconds: i32,
}

impl DatabasePool {
    // Get theme - matches Python get_theme function
    pub async fn get_theme(&self, user_id: i32) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT theme FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("theme").unwrap_or_else(|_| "Nordic".to_string())),
                    None => Ok("Nordic".to_string()),
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Theme FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("Theme").unwrap_or_else(|_| "Nordic".to_string())),
                    None => Ok("Nordic".to_string()),
                }
            },
        })
    }

    // Setup timezone info - matches Python setup_timezone_info function
    pub async fn setup_timezone_info(&self, user_id: i32, timezone: &str, hour_pref: i32, date_format: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET timezone = $1, timeformat = $2, dateformat = $3, firstlogin = $4 WHERE userid = $5"#)
                    .bind(timezone)
                    .bind(hour_pref)
                    .bind(date_format)
                    .bind(true)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET Timezone = ?, TimeFormat = ?, DateFormat = ?, FirstLogin = ? WHERE UserID = ?")
                    .bind(timezone)
                    .bind(hour_pref)
                    .bind(date_format)
                    .bind(1)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Update user timezone
    pub async fn update_user_timezone(&self, user_id: i32, timezone: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET timezone = $1 WHERE userid = $2"#)
                    .bind(timezone)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET Timezone = ? WHERE UserID = ?")
                    .bind(timezone)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Update user date format
    pub async fn update_user_date_format(&self, user_id: i32, date_format: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET dateformat = $1 WHERE userid = $2"#)
                    .bind(date_format)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET DateFormat = ? WHERE UserID = ?")
                    .bind(date_format)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Update user time format (hour preference)
    pub async fn update_user_time_format(&self, user_id: i32, hour_pref: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET timeformat = $1 WHERE userid = $2"#)
                    .bind(hour_pref)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET TimeFormat = ? WHERE UserID = ?")
                    .bind(hour_pref)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Extract audio URL from description/content HTML - matches Python logic
    // fn extract_audio_url_from_description(&self, data: &std::collections::HashMap<String, String>) -> Option<String> {
    //     // Check various description fields for audio URLs
    //     for field in ["content:encoded", "description", "summary", "itunes:summary"] {
    //         if let Some(content) = data.get(field) {
    //             if let Some(url) = self.find_audio_url_in_text(content) {
    //                 return Some(url.to_string());
    //             }
    //         }
    //     }
    //     None
    // }



    // Set user theme - matches Python set_theme function exactly
    pub async fn set_theme(&self, user_id: i32, theme: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "UserSettings" SET theme = $1 WHERE userid = $2"#)
                    .bind(theme)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE UserSettings SET Theme = ? WHERE UserID = ?")
                    .bind(theme)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    pub async fn get_custom_themes(&self, user_id: i32) -> AppResult<Vec<crate::models::CustomTheme>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT themeid, name, background_color, button_color, container_button_color,
                       button_text_color, text_color, text_secondary_color, border_color, accent_color,
                       prog_bar_color, error_color, bonus_color, secondary_background, container_background,
                       standout_color, hover_color, link_color, thumb_color, unfilled_color, check_box_color
                       FROM "CustomThemes" WHERE userid = $1 ORDER BY name"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut themes = Vec::new();
                for row in rows {
                    themes.push(crate::models::CustomTheme {
                        themeid: row.try_get("themeid")?,
                        name: row.try_get("name")?,
                        background_color: row.try_get("background_color")?,
                        button_color: row.try_get("button_color")?,
                        container_button_color: row.try_get("container_button_color")?,
                        button_text_color: row.try_get("button_text_color")?,
                        text_color: row.try_get("text_color")?,
                        text_secondary_color: row.try_get("text_secondary_color")?,
                        border_color: row.try_get("border_color")?,
                        accent_color: row.try_get("accent_color")?,
                        prog_bar_color: row.try_get("prog_bar_color")?,
                        error_color: row.try_get("error_color")?,
                        bonus_color: row.try_get("bonus_color")?,
                        secondary_background: row.try_get("secondary_background")?,
                        container_background: row.try_get("container_background")?,
                        standout_color: row.try_get("standout_color")?,
                        hover_color: row.try_get("hover_color")?,
                        link_color: row.try_get("link_color")?,
                        thumb_color: row.try_get("thumb_color")?,
                        unfilled_color: row.try_get("unfilled_color")?,
                        check_box_color: row.try_get("check_box_color")?,
                    });
                }
                Ok(themes)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    r#"SELECT ThemeID as themeid, Name as name, BackgroundColor as background_color,
                       ButtonColor as button_color, ContainerButtonColor as container_button_color,
                       ButtonTextColor as button_text_color, TextColor as text_color,
                       TextSecondaryColor as text_secondary_color, BorderColor as border_color,
                       AccentColor as accent_color, ProgBarColor as prog_bar_color,
                       ErrorColor as error_color, BonusColor as bonus_color,
                       SecondaryBackground as secondary_background, ContainerBackground as container_background,
                       StandoutColor as standout_color, HoverColor as hover_color,
                       LinkColor as link_color, ThumbColor as thumb_color,
                       UnfilledColor as unfilled_color, CheckBoxColor as check_box_color
                       FROM CustomThemes WHERE UserID = ? ORDER BY Name"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut themes = Vec::new();
                for row in rows {
                    themes.push(crate::models::CustomTheme {
                        themeid: row.try_get("themeid")?,
                        name: row.try_get("name")?,
                        background_color: row.try_get("background_color")?,
                        button_color: row.try_get("button_color")?,
                        container_button_color: row.try_get("container_button_color")?,
                        button_text_color: row.try_get("button_text_color")?,
                        text_color: row.try_get("text_color")?,
                        text_secondary_color: row.try_get("text_secondary_color")?,
                        border_color: row.try_get("border_color")?,
                        accent_color: row.try_get("accent_color")?,
                        prog_bar_color: row.try_get("prog_bar_color")?,
                        error_color: row.try_get("error_color")?,
                        bonus_color: row.try_get("bonus_color")?,
                        secondary_background: row.try_get("secondary_background")?,
                        container_background: row.try_get("container_background")?,
                        standout_color: row.try_get("standout_color")?,
                        hover_color: row.try_get("hover_color")?,
                        link_color: row.try_get("link_color")?,
                        thumb_color: row.try_get("thumb_color")?,
                        unfilled_color: row.try_get("unfilled_color")?,
                        check_box_color: row.try_get("check_box_color")?,
                    });
                }
                Ok(themes)
            },
        })
    }

    pub async fn create_custom_theme(&self, user_id: i32, req: &crate::models::CreateCustomThemeRequest) -> AppResult<crate::models::CustomTheme> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"INSERT INTO "CustomThemes"
                       (userid, name, background_color, button_color, container_button_color,
                        button_text_color, text_color, text_secondary_color, border_color, accent_color,
                        prog_bar_color, error_color, bonus_color, secondary_background, container_background,
                        standout_color, hover_color, link_color, thumb_color, unfilled_color, check_box_color)
                       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)
                       RETURNING themeid, name, background_color, button_color, container_button_color,
                       button_text_color, text_color, text_secondary_color, border_color, accent_color,
                       prog_bar_color, error_color, bonus_color, secondary_background, container_background,
                       standout_color, hover_color, link_color, thumb_color, unfilled_color, check_box_color"#
                )
                .bind(user_id)
                .bind(&req.name)
                .bind(&req.background_color)
                .bind(&req.button_color)
                .bind(&req.container_button_color)
                .bind(&req.button_text_color)
                .bind(&req.text_color)
                .bind(&req.text_secondary_color)
                .bind(&req.border_color)
                .bind(&req.accent_color)
                .bind(&req.prog_bar_color)
                .bind(&req.error_color)
                .bind(&req.bonus_color)
                .bind(&req.secondary_background)
                .bind(&req.container_background)
                .bind(&req.standout_color)
                .bind(&req.hover_color)
                .bind(&req.link_color)
                .bind(&req.thumb_color)
                .bind(&req.unfilled_color)
                .bind(&req.check_box_color)
                .fetch_one(pool)
                .await?;

                Ok(crate::models::CustomTheme {
                    themeid: row.try_get("themeid")?,
                    name: row.try_get("name")?,
                    background_color: row.try_get("background_color")?,
                    button_color: row.try_get("button_color")?,
                    container_button_color: row.try_get("container_button_color")?,
                    button_text_color: row.try_get("button_text_color")?,
                    text_color: row.try_get("text_color")?,
                    text_secondary_color: row.try_get("text_secondary_color")?,
                    border_color: row.try_get("border_color")?,
                    accent_color: row.try_get("accent_color")?,
                    prog_bar_color: row.try_get("prog_bar_color")?,
                    error_color: row.try_get("error_color")?,
                    bonus_color: row.try_get("bonus_color")?,
                    secondary_background: row.try_get("secondary_background")?,
                    container_background: row.try_get("container_background")?,
                    standout_color: row.try_get("standout_color")?,
                    hover_color: row.try_get("hover_color")?,
                    link_color: row.try_get("link_color")?,
                    thumb_color: row.try_get("thumb_color")?,
                    unfilled_color: row.try_get("unfilled_color")?,
                    check_box_color: row.try_get("check_box_color")?,
                })
            },
            mysql(pool) => {
                sqlx::query(
                    r#"INSERT INTO CustomThemes
                       (UserID, Name, BackgroundColor, ButtonColor, ContainerButtonColor,
                        ButtonTextColor, TextColor, TextSecondaryColor, BorderColor, AccentColor,
                        ProgBarColor, ErrorColor, BonusColor, SecondaryBackground, ContainerBackground,
                        StandoutColor, HoverColor, LinkColor, ThumbColor, UnfilledColor, CheckBoxColor)
                       VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#
                )
                .bind(user_id)
                .bind(&req.name)
                .bind(&req.background_color)
                .bind(&req.button_color)
                .bind(&req.container_button_color)
                .bind(&req.button_text_color)
                .bind(&req.text_color)
                .bind(&req.text_secondary_color)
                .bind(&req.border_color)
                .bind(&req.accent_color)
                .bind(&req.prog_bar_color)
                .bind(&req.error_color)
                .bind(&req.bonus_color)
                .bind(&req.secondary_background)
                .bind(&req.container_background)
                .bind(&req.standout_color)
                .bind(&req.hover_color)
                .bind(&req.link_color)
                .bind(&req.thumb_color)
                .bind(&req.unfilled_color)
                .bind(&req.check_box_color)
                .execute(pool)
                .await?;

                let row = sqlx::query(
                    r#"SELECT ThemeID as themeid, Name as name, BackgroundColor as background_color,
                       ButtonColor as button_color, ContainerButtonColor as container_button_color,
                       ButtonTextColor as button_text_color, TextColor as text_color,
                       TextSecondaryColor as text_secondary_color, BorderColor as border_color,
                       AccentColor as accent_color, ProgBarColor as prog_bar_color,
                       ErrorColor as error_color, BonusColor as bonus_color,
                       SecondaryBackground as secondary_background, ContainerBackground as container_background,
                       StandoutColor as standout_color, HoverColor as hover_color,
                       LinkColor as link_color, ThumbColor as thumb_color,
                       UnfilledColor as unfilled_color, CheckBoxColor as check_box_color
                       FROM CustomThemes WHERE UserID = ? AND Name = ? ORDER BY ThemeID DESC LIMIT 1"#
                )
                .bind(user_id)
                .bind(&req.name)
                .fetch_one(pool)
                .await?;

                Ok(crate::models::CustomTheme {
                    themeid: row.try_get("themeid")?,
                    name: row.try_get("name")?,
                    background_color: row.try_get("background_color")?,
                    button_color: row.try_get("button_color")?,
                    container_button_color: row.try_get("container_button_color")?,
                    button_text_color: row.try_get("button_text_color")?,
                    text_color: row.try_get("text_color")?,
                    text_secondary_color: row.try_get("text_secondary_color")?,
                    border_color: row.try_get("border_color")?,
                    accent_color: row.try_get("accent_color")?,
                    prog_bar_color: row.try_get("prog_bar_color")?,
                    error_color: row.try_get("error_color")?,
                    bonus_color: row.try_get("bonus_color")?,
                    secondary_background: row.try_get("secondary_background")?,
                    container_background: row.try_get("container_background")?,
                    standout_color: row.try_get("standout_color")?,
                    hover_color: row.try_get("hover_color")?,
                    link_color: row.try_get("link_color")?,
                    thumb_color: row.try_get("thumb_color")?,
                    unfilled_color: row.try_get("unfilled_color")?,
                    check_box_color: row.try_get("check_box_color")?,
                })
            },
        })
    }

    pub async fn delete_custom_theme(&self, theme_id: i32, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "CustomThemes" WHERE themeid = $1 AND userid = $2"#)
                    .bind(theme_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM CustomThemes WHERE ThemeID = ? AND UserID = ?")
                    .bind(theme_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Get user start page - matches Python get_user_startpage function exactly
    pub async fn get_user_startpage(&self, user_id: i32) -> AppResult<String> {
        info!("Getting start page for user {}", user_id);
        
        let startpage = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT startpage FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<Option<String>, _>("startpage")?.unwrap_or_else(|| "home".to_string())
                } else {
                    "home".to_string()
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT StartPage FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<Option<String>, _>("StartPage")?.unwrap_or_else(|| "home".to_string())
                } else {
                    "home".to_string()
                }
            },
        });
        
        Ok(startpage)
    }

    // Set user start page - matches Python set_user_startpage function exactly
    pub async fn set_user_startpage(&self, user_id: i32, startpage: &str) -> AppResult<bool> {
        info!("Setting start page for user {} to {}", user_id, startpage);
        
        // Check if user settings exist and perform update/insert
        let success = dialect_match!(self, {
            pg(pool) => {
                let existing = q::query(r#"SELECT COUNT(*) as count FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = existing.try_get("count")?;
                
                if count > 0 {
                    // Update existing record
                    let result = q::query(r#"UPDATE "UserSettings" SET startpage = $2 WHERE userid = $1"#)
                        .bind(user_id)
                        .bind(startpage)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    // Insert new record
                    let result = q::query(r#"INSERT INTO "UserSettings" (userid, startpage) VALUES ($1, $2)"#)
                        .bind(user_id)
                        .bind(startpage)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
            mysql(pool) => {
                let existing = sqlx::query("SELECT COUNT(*) as count FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = existing.try_get("count")?;
                
                if count > 0 {
                    // Update existing record
                    let result = sqlx::query("UPDATE UserSettings SET StartPage = ? WHERE UserID = ?")
                        .bind(startpage)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    // Insert new record
                    let result = sqlx::query("INSERT INTO UserSettings (UserID, StartPage) VALUES (?, ?)")
                        .bind(user_id)
                        .bind(startpage)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
        });
        
        info!("Successfully set start page for user {} to {}: {}", user_id, startpage, success);
        Ok(success)
    }

    // Update startpage wrapper function for compatibility
    pub async fn update_startpage(&self, user_id: i32, startpage: &str) -> AppResult<bool> {
        self.set_user_startpage(user_id, startpage).await
    }

    // Get startpage wrapper function for compatibility 
    pub async fn get_startpage(&self, user_id: i32) -> AppResult<String> {
        self.get_user_startpage(user_id).await
    }

    // Get user auto complete seconds setting
    pub async fn get_user_auto_complete_seconds(&self, user_id: i32) -> AppResult<i32> {
        info!("Getting auto complete seconds for user {}", user_id);
        
        let auto_complete_seconds = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT autocompleteseconds FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<Option<i32>, _>("autocompleteseconds")?.unwrap_or(0)
                } else {
                    0
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT AutoCompleteSeconds FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<Option<i32>, _>("AutoCompleteSeconds")?.unwrap_or(0)
                } else {
                    0
                }
            },
        });
        
        Ok(auto_complete_seconds)
    }

    // Set user auto complete seconds setting
    pub async fn set_user_auto_complete_seconds(&self, user_id: i32, seconds: i32) -> AppResult<bool> {
        info!("Setting auto complete seconds for user {} to {}", user_id, seconds);
        
        // Check if user settings exist and perform update/insert
        let success = dialect_match!(self, {
            pg(pool) => {
                let existing = q::query(r#"SELECT COUNT(*) as count FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = existing.try_get("count")?;
                
                if count > 0 {
                    // Update existing record
                    let result = q::query(r#"UPDATE "UserSettings" SET autocompleteseconds = $2 WHERE userid = $1"#)
                        .bind(user_id)
                        .bind(seconds)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    // Insert new record with default theme
                    let result = q::query(r#"INSERT INTO "UserSettings" (userid, autocompleteseconds, theme) VALUES ($1, $2, 'Nordic')"#)
                        .bind(user_id)
                        .bind(seconds)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
            mysql(pool) => {
                let existing = sqlx::query("SELECT COUNT(*) as count FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = existing.try_get("count")?;
                
                if count > 0 {
                    // Update existing record
                    let result = sqlx::query("UPDATE UserSettings SET AutoCompleteSeconds = ? WHERE UserID = ?")
                        .bind(seconds)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    // Insert new record with default theme
                    let result = sqlx::query("INSERT INTO UserSettings (UserID, AutoCompleteSeconds, Theme) VALUES (?, ?, 'Nordic')")
                        .bind(user_id)
                        .bind(seconds)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
        });
        
        info!("Successfully set auto complete seconds for user {} to {}: {}", user_id, seconds, success);
        Ok(success)
    }

    /// Where Add to queue puts an episode when the client doesn't say. Bottom unless set.
    pub async fn get_queue_add_position(&self, user_id: i32) -> AppResult<crate::models::QueueAddPosition> {
        let value: Option<String> = dialect_match!(self, {
            pg(pool) => {
                q::query(r#"SELECT queueaddposition FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .map(|row| row.try_get::<Option<String>, _>("queueaddposition"))
                    .transpose()?
                    .flatten()
            },
            mysql(pool) => {
                sqlx::query("SELECT QueueAddPosition FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .map(|row| row.try_get::<Option<String>, _>("QueueAddPosition"))
                    .transpose()?
                    .flatten()
            },
        });
        Ok(crate::models::QueueAddPosition::from_setting(value.as_deref()))
    }

    pub async fn set_queue_add_position(&self, user_id: i32, position: crate::models::QueueAddPosition) -> AppResult<()> {
        let value = position.as_setting();
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"INSERT INTO "UserSettings" (userid, queueaddposition, theme) VALUES ($1, $2, 'Nordic')
                            ON CONFLICT (userid) DO UPDATE SET queueaddposition = EXCLUDED.queueaddposition"#)
                    .bind(user_id)
                    .bind(value)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("INSERT INTO UserSettings (UserID, QueueAddPosition, Theme) VALUES (?, ?, 'Nordic')
                             ON DUPLICATE KEY UPDATE QueueAddPosition = VALUES(QueueAddPosition)")
                    .bind(user_id)
                    .bind(value)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Get playback speed - matches Python get_playback_speed function exactly
    pub async fn get_playback_speed(&self, user_id: i32, _is_youtube: bool, podcast_id: Option<i32>) -> AppResult<f64> {
        dialect_match!(self, {
            pg(pool) => {
                let query = if let Some(_pod_id) = podcast_id {
                    r#"SELECT PlaybackSpeed FROM "Podcasts" WHERE PodcastID = $1"#
                } else {
                    r#"SELECT PlaybackSpeed FROM "Users" WHERE UserID = $1"#
                };
                
                let param = podcast_id.unwrap_or(user_id);
                let row = q::query(query)
                    .bind(param)
                    .fetch_one(pool)
                    .await?;

                // PlaybackSpeed is a NUMERIC column in Postgres, which sqlx
                // decodes to BigDecimal (not f64). The column comes back
                // lowercased because it was selected unquoted.
                if let Ok(Some(speed)) = q::decimal(&row, "playbackspeed") {
                    Ok(speed)
                } else {
                    Ok(1.0)
                }
            },
            mysql(pool) => {
                let query = if let Some(_pod_id) = podcast_id {
                    "SELECT PlaybackSpeed FROM Podcasts WHERE PodcastID = ?"
                } else {
                    "SELECT PlaybackSpeed FROM Users WHERE UserID = ?"
                };
                
                let param = podcast_id.unwrap_or(user_id);
                let row = sqlx::query(query)
                    .bind(param)
                    .fetch_one(pool)
                    .await?;

                if let Ok(speed) = row.try_get::<bigdecimal::BigDecimal, _>("PlaybackSpeed") {
                    Ok(speed.to_f64().unwrap_or(1.0))
                } else {
                    Ok(1.0)
                }
            },
        })
    }

    // Get auto-delete-downloads setting (#655). Returns (days, customized).
    // For the user default (podcast_id None) customized is always false. For a podcast, days is
    // the raw per-podcast value and customized indicates whether it overrides the user default.
    pub async fn get_auto_download_delete_days(&self, user_id: i32, podcast_id: Option<i32>) -> AppResult<(i32, bool)> {
        dialect_match!(self, {
            pg(pool) => {
                if let Some(pod_id) = podcast_id {
                    let row = q::query(r#"SELECT autodownloaddeletedays, autodownloaddeletecustomized FROM "Podcasts" WHERE podcastid = $1"#)
                        .bind(pod_id)
                        .fetch_one(pool)
                        .await?;
                    Ok((
                        row.try_get::<i32, _>("autodownloaddeletedays").unwrap_or(0),
                        row.try_get::<bool, _>("autodownloaddeletecustomized").unwrap_or(false),
                    ))
                } else {
                    let row = q::query(r#"SELECT autodownloaddeletedays FROM "Users" WHERE userid = $1"#)
                        .bind(user_id)
                        .fetch_one(pool)
                        .await?;
                    Ok((row.try_get::<i32, _>("autodownloaddeletedays").unwrap_or(0), false))
                }
            },
            mysql(pool) => {
                if let Some(pod_id) = podcast_id {
                    let row = sqlx::query("SELECT AutoDownloadDeleteDays, AutoDownloadDeleteCustomized FROM Podcasts WHERE PodcastID = ?")
                        .bind(pod_id)
                        .fetch_one(pool)
                        .await?;
                    let customized = row.try_get::<i8, _>("AutoDownloadDeleteCustomized").map(|v| v != 0).unwrap_or(false);
                    Ok((row.try_get::<i32, _>("AutoDownloadDeleteDays").unwrap_or(0), customized))
                } else {
                    let row = sqlx::query("SELECT AutoDownloadDeleteDays FROM Users WHERE UserID = ?")
                        .bind(user_id)
                        .fetch_one(pool)
                        .await?;
                    Ok((row.try_get::<i32, _>("AutoDownloadDeleteDays").unwrap_or(0), false))
                }
            },
        })
    }

    // Get per-user default playback volume (0-100). Falls back to 100 if unset (#828).
    pub async fn get_default_volume(&self, user_id: i32) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT defaultvolume FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                Ok(row.try_get::<i32, _>("defaultvolume").unwrap_or(100))
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT DefaultVolume FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                Ok(row.try_get::<i32, _>("DefaultVolume").unwrap_or(100))
            },
        })
    }

    // "Delete downloads when finished": the user's default (podcast_id None), or the podcast's
    // effective value and whether it overrides the default.
    pub async fn get_auto_delete_on_complete(&self, user_id: i32, podcast_id: Option<i32>) -> AppResult<(bool, bool)> {
        let (default, podcast): (bool, Option<bool>) = dialect_match!(self, {
            pg(pool) => {
                let default = q::query(r#"SELECT COALESCE(autodeleteoncomplete, FALSE) AS d FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .and_then(|r| r.try_get::<bool, _>("d").ok())
                    .unwrap_or(false);
                let podcast = match podcast_id {
                    Some(pid) => q::query(r#"SELECT autodeleteoncomplete FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                        .bind(pid)
                        .bind(user_id)
                        .fetch_optional(pool)
                        .await?
                        .and_then(|r| r.try_get::<Option<bool>, _>("autodeleteoncomplete").ok().flatten()),
                    None => None,
                };
                (default, podcast)
            },
            mysql(pool) => {
                let default = sqlx::query("SELECT COALESCE(AutoDeleteOnComplete, 0) AS d FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .and_then(|r| r.try_get::<i8, _>("d").ok())
                    .map(|v| v != 0)
                    .unwrap_or(false);
                let podcast = match podcast_id {
                    Some(pid) => sqlx::query("SELECT AutoDeleteOnComplete FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                        .bind(pid)
                        .bind(user_id)
                        .fetch_optional(pool)
                        .await?
                        .and_then(|r| r.try_get::<Option<i8>, _>("AutoDeleteOnComplete").ok().flatten())
                        .map(|v| v != 0),
                    None => None,
                };
                (default, podcast)
            },
        });
        Ok((podcast.unwrap_or(default), podcast.is_some()))
    }

    pub async fn set_auto_delete_on_complete_user(&self, user_id: i32, enabled: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET autodeleteoncomplete = $1 WHERE userid = $2"#)
                    .bind(enabled)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET AutoDeleteOnComplete = ? WHERE UserID = ?")
                    .bind(enabled)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    pub async fn set_use_gravatar(&self, user_id: i32, enabled: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET usegravatar = $1 WHERE userid = $2"#)
                    .bind(enabled)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET UseGravatar = ? WHERE UserID = ?")
                    .bind(enabled)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set user playback speed - matches Python set_playback_speed_user function
    pub async fn set_playback_speed_user(&self, user_id: i32, playback_speed: f64) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET playbackspeed = $1 WHERE userid = $2"#)
                    .bind(playback_speed)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET PlaybackSpeed = ? WHERE UserID = ?")
                    .bind(playback_speed)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set per-user default playback volume (0-100) (#828) - mirrors set_playback_speed_user
    pub async fn set_default_volume(&self, user_id: i32, volume: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET defaultvolume = $1 WHERE userid = $2"#)
                    .bind(volume)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET DefaultVolume = ? WHERE UserID = ?")
                    .bind(volume)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set per-user default auto-delete-downloads days (#655) - mirrors set_playback_speed_user
    pub async fn set_auto_download_delete_days_user(&self, user_id: i32, days: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET autodownloaddeletedays = $1 WHERE userid = $2"#)
                    .bind(days)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET AutoDownloadDeleteDays = ? WHERE UserID = ?")
                    .bind(days)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set user podcast cover preference - global setting
    pub async fn set_global_podcast_cover_preference(&self, user_id: i32, use_podcast_covers: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET usepodcastcovers = $1 WHERE userid = $2"#)
                    .bind(use_podcast_covers)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET UsePodcastCovers = ? WHERE UserID = ?")
                    .bind(use_podcast_covers as i32)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Get global podcast cover preference - for settings page
    pub async fn get_global_podcast_cover_preference(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query_scalar::<Option<bool>>(r#"SELECT usepodcastcovers FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                Ok(result.unwrap_or(false))
            },
            mysql(pool) => {
                let result = sqlx::query_scalar::<_, Option<i32>>("SELECT UsePodcastCovers FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                Ok(result.unwrap_or(0) != 0)
            },
        })
    }

    // Get user's language preference
    pub async fn get_user_language(&self, user_id: i32) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT language FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.get::<Option<String>, _>("language").unwrap_or_else(|| "en".to_string()))
                } else {
                    Ok("en".to_string())
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Language FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.get::<Option<String>, _>("Language").unwrap_or_else(|| "en".to_string()))
                } else {
                    Ok("en".to_string())
                }
            },
        })
    }

    // Update user's language preference
    pub async fn update_user_language(&self, user_id: i32, language: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET language = $1 WHERE userid = $2"#)
                    .bind(language)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET Language = ? WHERE UserID = ?")
                    .bind(language)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }
}
