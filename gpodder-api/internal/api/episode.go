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
func getEpisodeActions(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] getEpisodeActions handling request: %s %s", c.Request.Method, c.Request.URL.Path)

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

		log.Printf("[DEBUG] getEpisodeActions: Parameters - since=%s, podcast=%s, device=%s, aggregated=%v",
			sinceStr, podcastURL, deviceName, aggregated)

		// Get device ID if provided
		var deviceID *int
		if deviceName != "" {
			var deviceIDInt int
			err := database.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
			`, userID, deviceName).Scan(&deviceIDInt)

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
		err := database.QueryRow(`
			SELECT COALESCE(MAX(Timestamp), EXTRACT(EPOCH FROM NOW())::bigint)
			FROM "GpodderSyncEpisodeActions"
			WHERE UserID = $1
		`, userID).Scan(&latestTimestamp)

		if err != nil {
			log.Printf("[ERROR] getEpisodeActions: Error getting latest timestamp: %v", err)
			latestTimestamp = time.Now().Unix() // Fallback to current time
		}

		// Build query based on parameters
		queryParts := []string{
			"SELECT " +
				"e.ActionID, e.UserID, e.DeviceID, e.PodcastURL, e.EpisodeURL, " +
				"e.Action, e.Timestamp, e.Started, e.Position, e.Total, " +
				"d.DeviceName " +
				"FROM \"GpodderSyncEpisodeActions\" e " +
				"LEFT JOIN \"GpodderDevices\" d ON e.DeviceID = d.DeviceID " +
				"WHERE e.UserID = $1",
		}

		args := []interface{}{userID}
		paramCount := 2

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

		// For aggregated results, we need a more complex query
		var query string
		if aggregated {
			// Build subquery to get latest action for each episode
			subQueryParts := make([]string, len(queryParts))
			copy(subQueryParts, queryParts)

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
				ORDER BY e.Timestamp DESC
			`, strings.Join(subQueryParts[1:], " "))
		} else {
			// Simple query with ORDER BY
			queryParts = append(queryParts, "ORDER BY e.Timestamp DESC")
			query = strings.Join(queryParts, " ")
		}

		// Execute query
		rows, err := database.Query(query, args...)
		if err != nil {
			log.Printf("[ERROR] getEpisodeActions: Error querying episode actions: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get episode actions"})
			return
		}
		defer rows.Close()

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

		log.Printf("[DEBUG] getEpisodeActions: Returning %d actions with timestamp %d",
			len(actions), latestTimestamp)

		// Return response in gpodder format
		c.JSON(http.StatusOK, models.EpisodeActionsResponse{
			Actions:   actions,
			Timestamp: latestTimestamp,
		})
	}
}

// uploadEpisodeActions handles POST /api/2/episodes/{username}.json
func uploadEpisodeActions(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] uploadEpisodeActions handling request: %s %s", c.Request.Method, c.Request.URL.Path)

		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] uploadEpisodeActions: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Parse request
		var actions []models.EpisodeAction
		if err := c.ShouldBindJSON(&actions); err != nil {
			log.Printf("[ERROR] uploadEpisodeActions: Error parsing request body: %v", err)
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON array of episode actions"})
			return
		}

		log.Printf("[DEBUG] uploadEpisodeActions: Received %d actions to process", len(actions))

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
				err := tx.QueryRow(`
					SELECT DeviceID FROM "GpodderDevices"
					WHERE UserID = $1 AND DeviceName = $2
				`, userID, action.Device).Scan(&deviceID.Int64)

				if err != nil {
					if err == sql.ErrNoRows {
						// Create the device if it doesn't exist
						log.Printf("[DEBUG] uploadEpisodeActions: Creating new device: %s", action.Device)
						err = tx.QueryRow(`
							INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
							VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
							RETURNING DeviceID
						`, userID, action.Device).Scan(&deviceID.Int64)

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

			// Use provided timestamp or current time
			actionTimestamp := timestamp
			if action.Timestamp > 0 {
				actionTimestamp = action.Timestamp
			}

			// Insert action
			_, err = tx.Exec(`
				INSERT INTO "GpodderSyncEpisodeActions"
				(UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, Started, Position, Total)
				VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
			`,
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
				err := tx.QueryRow(`
					SELECT e.EpisodeID
					FROM "Episodes" e
					JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
					WHERE p.FeedURL = $1 AND e.EpisodeURL = $2 AND p.UserID = $3
				`, cleanPodcastURL, cleanEpisodeURL, userID).Scan(&episodeID)

				if err == nil { // Episode found
					// Try to update existing history record
					result, err := tx.Exec(`
						UPDATE "UserEpisodeHistory"
						SET ListenDuration = $1, ListenDate = $2
						WHERE UserID = $3 AND EpisodeID = $4
					`, action.Position, time.Unix(actionTimestamp, 0), userID, episodeID)

					if err != nil {
						log.Printf("[WARNING] uploadEpisodeActions: Error updating episode history: %v", err)
					} else {
						rowsAffected, _ := result.RowsAffected()
						if rowsAffected == 0 {
							// No history exists, create it
							_, err = tx.Exec(`
								INSERT INTO "UserEpisodeHistory"
								(UserID, EpisodeID, ListenDuration, ListenDate)
								VALUES ($1, $2, $3, $4)
								ON CONFLICT (UserID, EpisodeID) DO UPDATE
								SET ListenDuration = $3, ListenDate = $4
							`, userID, episodeID, action.Position, time.Unix(actionTimestamp, 0))

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

		log.Printf("[DEBUG] uploadEpisodeActions: Successfully processed %d actions", len(actions))

		// Return response
		c.JSON(http.StatusOK, models.EpisodeActionResponse{
			Timestamp:  timestamp,
			UpdateURLs: updateURLs,
		})
	}
}

// getFavoriteEpisodes handles GET /api/2/favorites/{username}.json
func getFavoriteEpisodes(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")

		// Query for favorite episodes
		// Here we identify favorites by checking for episodes with the "is_favorite" setting
		rows, err := database.Query(`
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
		`, userID)

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
func getEpisodeData(database *db.PostgresDB) gin.HandlerFunc {
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

		err := database.QueryRow(`
			SELECT
				e.EpisodeTitle, e.EpisodeURL, e.EpisodeDescription, e.EpisodeArtwork,
				p.PodcastName, p.FeedURL, e.EpisodePubDate
			FROM "Episodes" e
			JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
			WHERE p.FeedURL = $1 AND e.EpisodeURL = $2
			LIMIT 1
		`, podcastURL, episodeURL).Scan(
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
