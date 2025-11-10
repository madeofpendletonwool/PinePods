package api

import (
	"database/sql"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"strings"
	"time"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"

	"github.com/gin-gonic/gin"
)

// getEpisodeActions handles GET /api/2/episodes/{username}.json
func getEpisodeActions(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {

		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] getEpisodeActions: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Parse query parameters
		sinceStr := c.Query("since")
		podcastURL := c.Query("podcast")
		deviceName := c.Query("device")
		aggregated := c.Query("aggregated") == "true"

		// Get device ID if provided
		var deviceID *int
		if deviceName != "" {
			var deviceIDInt int
			var query string

			if database.IsPostgreSQLDB() {
				query = `
					SELECT DeviceID FROM "GpodderDevices"
					WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
				`
			} else {
				query = `
					SELECT DeviceID FROM GpodderDevices
					WHERE UserID = ? AND DeviceName = ? AND IsActive = true
				`
			}

			err := database.QueryRow(query, userID, deviceName).Scan(&deviceIDInt)

			if err != nil {
				if err != sql.ErrNoRows {
					log.Printf("[ERROR] getEpisodeActions: Error getting device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
					return
				}
				// Device not found is not fatal if querying by device
			} else {
				deviceID = &deviceIDInt
			}
		}

		var since int64 = 0
		if sinceStr != "" {
			var err error
			since, err = strconv.ParseInt(sinceStr, 10, 64)
			if err != nil {
				log.Printf("[ERROR] getEpisodeActions: Invalid since parameter: %s", sinceStr)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid since parameter: must be a Unix timestamp"})
				return
			}
		}

		// Get the latest timestamp for the response
		var latestTimestamp int64
		var timestampQuery string

		if database.IsPostgreSQLDB() {
			timestampQuery = `
				SELECT COALESCE(MAX(Timestamp), EXTRACT(EPOCH FROM NOW())::bigint)
				FROM "GpodderSyncEpisodeActions"
				WHERE UserID = $1
			`
		} else {
			timestampQuery = `
				SELECT COALESCE(MAX(Timestamp), UNIX_TIMESTAMP())
				FROM GpodderSyncEpisodeActions
				WHERE UserID = ?
			`
		}

		err := database.QueryRow(timestampQuery, userID).Scan(&latestTimestamp)

		if err != nil {
			log.Printf("[ERROR] getEpisodeActions: Error getting latest timestamp: %v", err)
			latestTimestamp = time.Now().Unix() // Fallback to current time
		}

		// Performance optimization: Add limits and optimize query structure
		const MAX_EPISODE_ACTIONS = 25000 // Limit raised to 25k to handle power users while preventing DoS

		// Log query performance info
		log.Printf("[DEBUG] getEpisodeActions: Query for user %v with since=%d, device=%s, aggregated=%v",
			userID, since, deviceName, aggregated)
		
		// Build query based on parameters with performance optimizations
		var queryParts []string

		if database.IsPostgreSQLDB() {
			queryParts = []string{
				"SELECT " +
					"e.ActionID, e.UserID, e.DeviceID, e.PodcastURL, e.EpisodeURL, " +
					"e.Action, e.Timestamp, e.Started, e.Position, e.Total, " +
					"COALESCE(d.DeviceName, '') as DeviceName " +
					"FROM \"GpodderSyncEpisodeActions\" e " +
					"LEFT JOIN \"GpodderDevices\" d ON e.DeviceID = d.DeviceID " +
					"WHERE e.UserID = $1",
			}
		} else {
			queryParts = []string{
				"SELECT " +
					"e.ActionID, e.UserID, e.DeviceID, e.PodcastURL, e.EpisodeURL, " +
					"e.Action, e.Timestamp, e.Started, e.Position, e.Total, " +
					"COALESCE(d.DeviceName, '') as DeviceName " +
					"FROM GpodderSyncEpisodeActions e " +
					"LEFT JOIN GpodderDevices d ON e.DeviceID = d.DeviceID " +
					"WHERE e.UserID = ?",
			}
		}

		args := []interface{}{userID}
		paramCount := 2

		// For aggregated results, we need a more complex query
		var query string
		if aggregated {
			if database.IsPostgreSQLDB() {
				// Build conditions for the subquery
				var conditions []string

				if since > 0 {
					conditions = append(conditions, fmt.Sprintf("AND e.Timestamp > $%d", paramCount))
					args = append(args, since)
					paramCount++
				}

				if podcastURL != "" {
					conditions = append(conditions, fmt.Sprintf("AND e.PodcastURL = $%d", paramCount))
					args = append(args, podcastURL)
					paramCount++
				}

				if deviceID != nil {
					conditions = append(conditions, fmt.Sprintf("AND e.DeviceID = $%d", paramCount))
					args = append(args, *deviceID)
					paramCount++
				}

				conditionsStr := strings.Join(conditions, " ")

				query = fmt.Sprintf(`
					WITH latest_actions AS (
						SELECT
							e.PodcastURL,
							e.EpisodeURL,
							MAX(e.Timestamp) as max_timestamp
						FROM "GpodderSyncEpisodeActions" e
						WHERE e.UserID = $1
						%s
						GROUP BY e.PodcastURL, e.EpisodeURL
					)
					SELECT
						e.ActionID, e.UserID, e.DeviceID, e.PodcastURL, e.EpisodeURL,
						e.Action, e.Timestamp, e.Started, e.Position, e.Total,
						d.DeviceName
					FROM "GpodderSyncEpisodeActions" e
					JOIN latest_actions la ON
						e.PodcastURL = la.PodcastURL AND
						e.EpisodeURL = la.EpisodeURL AND
						e.Timestamp = la.max_timestamp
					LEFT JOIN "GpodderDevices" d ON e.DeviceID = d.DeviceID
					WHERE e.UserID = $1
					ORDER BY e.Timestamp ASC
					LIMIT %d
				`, conditionsStr, MAX_EPISODE_ACTIONS)
			} else {
				// For MySQL, we need to use ? placeholders and rebuild the argument list
				args = []interface{}{userID} // Reset args to just include userID for now

				// Build conditions for the subquery
				var conditions []string

				if since > 0 {
					conditions = append(conditions, "AND e.Timestamp > ?")
					args = append(args, since)
				}

				if podcastURL != "" {
					conditions = append(conditions, "AND e.PodcastURL = ?")
					args = append(args, podcastURL)
				}

				if deviceID != nil {
					conditions = append(conditions, "AND e.DeviceID = ?")
					args = append(args, *deviceID)
				}

				conditionsStr := strings.Join(conditions, " ")

				// Need to duplicate userID in args for the second part of the query
				mysqlArgs := make([]interface{}, len(args))
				copy(mysqlArgs, args)
				args = append(args, mysqlArgs...)

				query = fmt.Sprintf(`
					WITH latest_actions AS (
						SELECT
							e.PodcastURL,
							e.EpisodeURL,
							MAX(e.Timestamp) as max_timestamp
						FROM GpodderSyncEpisodeActions e
						WHERE e.UserID = ?
						%s
						GROUP BY e.PodcastURL, e.EpisodeURL
					)
					SELECT
						e.ActionID, e.UserID, e.DeviceID, e.PodcastURL, e.EpisodeURL,
						e.Action, e.Timestamp, e.Started, e.Position, e.Total,
						d.DeviceName
					FROM GpodderSyncEpisodeActions e
					JOIN latest_actions la ON
						e.PodcastURL = la.PodcastURL AND
						e.EpisodeURL = la.EpisodeURL AND
						e.Timestamp = la.max_timestamp
					LEFT JOIN GpodderDevices d ON e.DeviceID = d.DeviceID
					WHERE e.UserID = ?
					ORDER BY e.Timestamp ASC
					LIMIT %d
				`, conditionsStr, MAX_EPISODE_ACTIONS)
			}
		} else {
			// Simple query with ORDER BY
			if database.IsPostgreSQLDB() {
				if since > 0 {
					queryParts = append(queryParts, fmt.Sprintf("AND e.Timestamp > $%d", paramCount))
					args = append(args, since)
					paramCount++
				}

				if podcastURL != "" {
					queryParts = append(queryParts, fmt.Sprintf("AND e.PodcastURL = $%d", paramCount))
					args = append(args, podcastURL)
					paramCount++
				}

				if deviceID != nil {
					queryParts = append(queryParts, fmt.Sprintf("AND e.DeviceID = $%d", paramCount))
					args = append(args, *deviceID)
					paramCount++
				}
			} else {
				if since > 0 {
					queryParts = append(queryParts, "AND e.Timestamp > ?")
					args = append(args, since)
				}

				if podcastURL != "" {
					queryParts = append(queryParts, "AND e.PodcastURL = ?")
					args = append(args, podcastURL)
				}

				if deviceID != nil {
					queryParts = append(queryParts, "AND e.DeviceID = ?")
					args = append(args, *deviceID)
				}
			}

			// ORDER BY ASC (oldest first) for proper pagination with since parameter
			// This ensures that when client uses since={last_timestamp}, they get the next batch chronologically
			queryParts = append(queryParts, "ORDER BY e.Timestamp ASC")

			// Add LIMIT for performance - prevents returning massive datasets
			// Clients should use the 'since' parameter to paginate through results
			if database.IsPostgreSQLDB() {
				queryParts = append(queryParts, fmt.Sprintf("LIMIT %d", MAX_EPISODE_ACTIONS))
			} else {
				queryParts = append(queryParts, fmt.Sprintf("LIMIT %d", MAX_EPISODE_ACTIONS))
			}
			
			query = strings.Join(queryParts, " ")
		}

		// Execute query with timing
		startTime := time.Now()
		rows, err := database.Query(query, args...)
		queryDuration := time.Since(startTime)
		
		if err != nil {
			log.Printf("[ERROR] getEpisodeActions: Error querying episode actions (took %v): %v", queryDuration, err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get episode actions"})
			return
		}
		defer rows.Close()
		
		log.Printf("[DEBUG] getEpisodeActions: Query executed in %v", queryDuration)

		// Build response
		actions := make([]models.EpisodeAction, 0)
		for rows.Next() {
			var action models.EpisodeAction
			var deviceIDInt sql.NullInt64
			var deviceName sql.NullString
			var started sql.NullInt64
			var position sql.NullInt64
			var total sql.NullInt64

			if err := rows.Scan(
				&action.ActionID,
				&action.UserID,
				&deviceIDInt,
				&action.Podcast,
				&action.Episode,
				&action.Action,
				&action.Timestamp,
				&started,
				&position,
				&total,
				&deviceName,
			); err != nil {
				log.Printf("[ERROR] getEpisodeActions: Error scanning action row: %v", err)
				continue
			}

			// Set optional fields if present
			if deviceName.Valid {
				action.Device = deviceName.String
			}

			if started.Valid {
				startedInt := int(started.Int64)
				action.Started = &startedInt
			}

			if position.Valid {
				positionInt := int(position.Int64)
				action.Position = &positionInt
			}

			if total.Valid {
				totalInt := int(total.Int64)
				action.Total = &totalInt
			}

			actions = append(actions, action)
		}

		if err = rows.Err(); err != nil {
			log.Printf("[ERROR] getEpisodeActions: Error iterating rows: %v", err)
			// Continue with what we've got so far
		}

		// Log performance results
		totalDuration := time.Since(startTime)
		log.Printf("[DEBUG] getEpisodeActions: Returning %d actions, total time: %v", len(actions), totalDuration)

		// Return response in gpodder format
		c.JSON(http.StatusOK, models.EpisodeActionsResponse{
			Actions:   actions,
			Timestamp: latestTimestamp,
		})
	}
}

// uploadEpisodeActions handles POST /api/2/episodes/{username}.json
func uploadEpisodeActions(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {

		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] uploadEpisodeActions: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Parse request - try both formats
		var actions []models.EpisodeAction

		// First try parsing as array directly
		if err := c.ShouldBindJSON(&actions); err != nil {
			// If that fails, try parsing as a wrapper object
			var wrappedActions struct {
				Actions []models.EpisodeAction `json:"actions"`
			}
			if err2 := c.ShouldBindJSON(&wrappedActions); err2 != nil {
				log.Printf("[ERROR] uploadEpisodeActions: Error parsing request body: %v", err)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body format"})
				return
			}
			actions = wrappedActions.Actions
		}

		// Begin transaction
		tx, err := database.Begin()
		if err != nil {
			log.Printf("[ERROR] uploadEpisodeActions: Error beginning transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to begin transaction"})
			return
		}
		defer func() {
			if err != nil {
				tx.Rollback()
				return
			}
		}()

		// Process actions
		timestamp := time.Now().Unix()
		updateURLs := make([][]string, 0)

		for _, action := range actions {
			// Validate action
			if action.Podcast == "" || action.Episode == "" || action.Action == "" {
				log.Printf("[WARNING] uploadEpisodeActions: Skipping invalid action: podcast=%s, episode=%s, action=%s",
					action.Podcast, action.Episode, action.Action)
				continue
			}

			// Clean URLs if needed
			cleanPodcastURL, err := sanitizeURL(action.Podcast)
			if err != nil {
				log.Printf("[WARNING] uploadEpisodeActions: Error sanitizing podcast URL %s: %v", action.Podcast, err)
				cleanPodcastURL = action.Podcast // Use original if sanitization fails
			}

			cleanEpisodeURL, err := sanitizeURL(action.Episode)
			if err != nil {
				log.Printf("[WARNING] uploadEpisodeActions: Error sanitizing episode URL %s: %v", action.Episode, err)
				cleanEpisodeURL = action.Episode // Use original if sanitization fails
			}

			// Get or create device ID if provided
			var deviceID sql.NullInt64
			if action.Device != "" {
				var query string

				if database.IsPostgreSQLDB() {
					query = `
                        SELECT DeviceID FROM "GpodderDevices"
                        WHERE UserID = $1 AND DeviceName = $2
                    `
				} else {
					query = `
                        SELECT DeviceID FROM GpodderDevices
                        WHERE UserID = ? AND DeviceName = ?
                    `
				}

				err := tx.QueryRow(query, userID, action.Device).Scan(&deviceID.Int64)

				if err != nil {
					if err == sql.ErrNoRows {
						// Create the device if it doesn't exist

						if database.IsPostgreSQLDB() {
							query = `
                                INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
                                VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
                                RETURNING DeviceID
                            `
							err = tx.QueryRow(query, userID, action.Device).Scan(&deviceID.Int64)
						} else {
							query = `
                                INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
                                VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
                            `
							result, err := tx.Exec(query, userID, action.Device, "other")
							if err != nil {
								log.Printf("[ERROR] uploadEpisodeActions: Error creating device: %v", err)
								c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
								return
							}

							lastID, err := result.LastInsertId()
							if err != nil {
								log.Printf("[ERROR] uploadEpisodeActions: Error getting last insert ID: %v", err)
								c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
								return
							}

							deviceID.Int64 = lastID
						}

						if err != nil {
							log.Printf("[ERROR] uploadEpisodeActions: Error creating device: %v", err)
							c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
							return
						}
						deviceID.Valid = true
					} else {
						log.Printf("[ERROR] uploadEpisodeActions: Error getting device ID: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
						return
					}
				} else {
					deviceID.Valid = true
				}
			}

			// Parse timestamp from interface{} to int64
			actionTimestamp := timestamp
			if action.Timestamp != nil {
				switch t := action.Timestamp.(type) {
				case float64:
					actionTimestamp = int64(t)
				case int64:
					actionTimestamp = t
				case int:
					actionTimestamp = int64(t)
				case string:
					// First try to parse as Unix timestamp
					if ts, err := strconv.ParseInt(t, 10, 64); err == nil {
						actionTimestamp = ts
					} else {
						// Try parsing as ISO date (2025-04-23T12:18:51)
						if parsedTime, err := time.Parse(time.RFC3339, t); err == nil {
							actionTimestamp = parsedTime.Unix()
							log.Printf("[DEBUG] uploadEpisodeActions: Parsed ISO timestamp '%s' to Unix timestamp %d", t, actionTimestamp)
						} else {
							// Try some other common formats
							formats := []string{
								"2006-01-02T15:04:05Z",
								"2006-01-02T15:04:05",
								"2006-01-02 15:04:05",
								"2006-01-02",
							}

							parsed := false
							for _, format := range formats {
								if parsedTime, err := time.Parse(format, t); err == nil {
									actionTimestamp = parsedTime.Unix()
									parsed = true
									break
								}
							}

							if !parsed {
								log.Printf("[WARNING] uploadEpisodeActions: Could not parse timestamp '%s', using current time", t)
							}
						}
					}
				default:
					log.Printf("[WARNING] uploadEpisodeActions: Unknown timestamp type, using current time")
				}
			}

			// Insert action
			var insertQuery string

			if database.IsPostgreSQLDB() {
				insertQuery = `
                    INSERT INTO "GpodderSyncEpisodeActions"
                    (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, Started, Position, Total)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                `
			} else {
				insertQuery = `
                    INSERT INTO GpodderSyncEpisodeActions
                    (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, Started, Position, Total)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                `
			}

			_, err = tx.Exec(insertQuery,
				userID,
				deviceID,
				cleanPodcastURL,
				cleanEpisodeURL,
				action.Action,
				actionTimestamp,
				action.Started,
				action.Position,
				action.Total)

			if err != nil {
				log.Printf("[ERROR] uploadEpisodeActions: Error inserting episode action: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to save episode action"})
				return
			}

			// Add to updateURLs if URLs were cleaned
			if cleanPodcastURL != action.Podcast {
				updateURLs = append(updateURLs, []string{action.Podcast, cleanPodcastURL})
			}
			if cleanEpisodeURL != action.Episode {
				updateURLs = append(updateURLs, []string{action.Episode, cleanEpisodeURL})
			}

			// For play action with position > 0, update episode status in Pinepods database
			if action.Action == "play" && action.Position != nil && *action.Position > 0 {
				// Try to find episode ID in Episodes table
				var episodeID int
				var findEpisodeQuery string

				if database.IsPostgreSQLDB() {
					findEpisodeQuery = `
                        SELECT e.EpisodeID
                        FROM "Episodes" e
                        JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
                        WHERE p.FeedURL = $1 AND e.EpisodeURL = $2 AND p.UserID = $3
                    `
				} else {
					findEpisodeQuery = `
                        SELECT e.EpisodeID
                        FROM Episodes e
                        JOIN Podcasts p ON e.PodcastID = p.PodcastID
                        WHERE p.FeedURL = ? AND e.EpisodeURL = ? AND p.UserID = ?
                    `
				}

				err := tx.QueryRow(findEpisodeQuery, cleanPodcastURL, cleanEpisodeURL, userID).Scan(&episodeID)

				if err == nil { // Episode found
					// Try to update existing history record
					var updateHistoryQuery string

					if database.IsPostgreSQLDB() {
						updateHistoryQuery = `
                            UPDATE "UserEpisodeHistory"
                            SET ListenDuration = $1, ListenDate = $2
                            WHERE UserID = $3 AND EpisodeID = $4
                        `
					} else {
						updateHistoryQuery = `
                            UPDATE UserEpisodeHistory
                            SET ListenDuration = ?, ListenDate = ?
                            WHERE UserID = ? AND EpisodeID = ?
                        `
					}

					result, err := tx.Exec(updateHistoryQuery, action.Position, time.Unix(actionTimestamp, 0), userID, episodeID)

					if err != nil {
						log.Printf("[WARNING] uploadEpisodeActions: Error updating episode history: %v", err)
					} else {
						rowsAffected, _ := result.RowsAffected()
						if rowsAffected == 0 {
							// No history exists, create it
							var insertHistoryQuery string

							if database.IsPostgreSQLDB() {
								insertHistoryQuery = `
                                    INSERT INTO "UserEpisodeHistory"
                                    (UserID, EpisodeID, ListenDuration, ListenDate)
                                    VALUES ($1, $2, $3, $4)
                                    ON CONFLICT (UserID, EpisodeID) DO UPDATE
                                    SET ListenDuration = $3, ListenDate = $4
                                `
							} else {
								insertHistoryQuery = `
                                    INSERT INTO UserEpisodeHistory
                                    (UserID, EpisodeID, ListenDuration, ListenDate)
                                    VALUES (?, ?, ?, ?)
                                    ON DUPLICATE KEY UPDATE
                                    ListenDuration = VALUES(ListenDuration), ListenDate = VALUES(ListenDate)
                                `
							}

							_, err = tx.Exec(insertHistoryQuery, userID, episodeID, action.Position, time.Unix(actionTimestamp, 0))

							if err != nil {
								log.Printf("[WARNING] uploadEpisodeActions: Error creating episode history: %v", err)
							}
						}
					}
				}
			}
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("[ERROR] uploadEpisodeActions: Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return response
		c.JSON(http.StatusOK, models.EpisodeActionResponse{
			Timestamp:  timestamp,
			UpdateURLs: updateURLs,
		})
	}
}

// getFavoriteEpisodes handles GET /api/2/favorites/{username}.json
func getFavoriteEpisodes(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")

		// Query for favorite episodes
		// Here we identify favorites by checking for episodes with the "is_favorite" setting
		var query string
		var rows *sql.Rows
		var err error

		if database.IsPostgreSQLDB() {
			query = `
				SELECT
					e.EpisodeTitle, e.EpisodeURL, e.EpisodeDescription, e.EpisodeArtwork,
					p.PodcastName, p.FeedURL, e.EpisodePubDate
				FROM "Episodes" e
				JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
				JOIN "GpodderSyncSettings" s ON s.UserID = p.UserID
										  AND s.PodcastURL = p.FeedURL
										  AND s.EpisodeURL = e.EpisodeURL
				WHERE s.UserID = $1
				  AND s.Scope = 'episode'
				  AND s.SettingKey = 'is_favorite'
				  AND s.SettingValue = 'true'
				ORDER BY e.EpisodePubDate DESC
			`
			rows, err = database.Query(query, userID)
		} else {
			query = `
				SELECT
					e.EpisodeTitle, e.EpisodeURL, e.EpisodeDescription, e.EpisodeArtwork,
					p.PodcastName, p.FeedURL, e.EpisodePubDate
				FROM Episodes e
				JOIN Podcasts p ON e.PodcastID = p.PodcastID
				JOIN GpodderSyncSettings s ON s.UserID = p.UserID
										  AND s.PodcastURL = p.FeedURL
										  AND s.EpisodeURL = e.EpisodeURL
				WHERE s.UserID = ?
				  AND s.Scope = 'episode'
				  AND s.SettingKey = 'is_favorite'
				  AND s.SettingValue = 'true'
				ORDER BY e.EpisodePubDate DESC
			`
			rows, err = database.Query(query, userID)
		}

		if err != nil {
			log.Printf("Error querying favorite episodes: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get favorite episodes"})
			return
		}
		defer rows.Close()

		// Build response
		favorites := make([]models.Episode, 0)
		for rows.Next() {
			var episode models.Episode
			var pubDate time.Time
			if err := rows.Scan(
				&episode.Title,
				&episode.URL,
				&episode.Description,
				&episode.Website, // Using EpisodeArtwork for Website for now
				&episode.PodcastTitle,
				&episode.PodcastURL,
				&pubDate,
			); err != nil {
				log.Printf("Error scanning favorite episode: %v", err)
				continue
			}
			// Format the publication date in ISO 8601
			episode.Released = pubDate.Format(time.RFC3339)
			// Set MygpoLink (just a placeholder for now)
			episode.MygpoLink = fmt.Sprintf("/episode/%s", episode.URL)
			favorites = append(favorites, episode)
		}
		c.JSON(http.StatusOK, favorites)
	}
}

// getEpisodeData handles GET /api/2/data/episode.json
// getEpisodeData handles GET /api/2/data/episode.json
func getEpisodeData(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Parse query parameters
		podcastURL := c.Query("podcast")
		episodeURL := c.Query("url")
		if podcastURL == "" || episodeURL == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Both podcast and url parameters are required"})
			return
		}

		// Query for episode data
		var episode models.Episode
		var pubDate time.Time
		var query string

		if database.IsPostgreSQLDB() {
			query = `
				SELECT
					e.EpisodeTitle, e.EpisodeURL, e.EpisodeDescription, e.EpisodeArtwork,
					p.PodcastName, p.FeedURL, e.EpisodePubDate
				FROM "Episodes" e
				JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
				WHERE p.FeedURL = $1 AND e.EpisodeURL = $2
				LIMIT 1
			`
		} else {
			query = `
				SELECT
					e.EpisodeTitle, e.EpisodeURL, e.EpisodeDescription, e.EpisodeArtwork,
					p.PodcastName, p.FeedURL, e.EpisodePubDate
				FROM Episodes e
				JOIN Podcasts p ON e.PodcastID = p.PodcastID
				WHERE p.FeedURL = ? AND e.EpisodeURL = ?
				LIMIT 1
			`
		}

		err := database.QueryRow(query, podcastURL, episodeURL).Scan(
			&episode.Title,
			&episode.URL,
			&episode.Description,
			&episode.Website, // Using EpisodeArtwork for Website for now
			&episode.PodcastTitle,
			&episode.PodcastURL,
			&pubDate,
		)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "Episode not found"})
			} else {
				log.Printf("Error querying episode data: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get episode data"})
			}
			return
		}

		// Format the publication date in ISO 8601
		episode.Released = pubDate.Format(time.RFC3339)
		// Set MygpoLink (just a placeholder for now)
		episode.MygpoLink = fmt.Sprintf("/episode/%s", episode.URL)
		c.JSON(http.StatusOK, episode)
	}
}
