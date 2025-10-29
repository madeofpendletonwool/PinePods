package api

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"
	"pinepods/gpodder-api/internal/utils"

	"github.com/gin-gonic/gin"
)

// Maximum number of subscriptions per user
const MAX_SUBSCRIPTIONS = 5000

// Limits for subscription sync to prevent overwhelming responses
const MAX_SUBSCRIPTION_CHANGES = 5000 // Reasonable limit for subscription changes per sync

// sanitizeURL cleans and validates a URL
func sanitizeURL(rawURL string) (string, error) {
	// Trim leading/trailing whitespace
	trimmedURL := strings.TrimSpace(rawURL)

	// Check if URL is not empty
	if trimmedURL == "" {
		return "", fmt.Errorf("empty URL")
	}

	// Parse URL to validate format
	parsedURL, err := url.Parse(trimmedURL)
	if err != nil {
		return "", fmt.Errorf("invalid URL format: %w", err)
	}

	// Ensure the URL has a scheme, default to https if missing
	if parsedURL.Scheme == "" {
		parsedURL.Scheme = "https"
	}

	// Only allow http and https schemes
	if parsedURL.Scheme != "http" && parsedURL.Scheme != "https" {
		return "", fmt.Errorf("unsupported URL scheme: %s", parsedURL.Scheme)
	}

	// Ensure the URL has a host
	if parsedURL.Host == "" {
		return "", fmt.Errorf("URL missing host")
	}

	// Return the sanitized URL
	return parsedURL.String(), nil
}

// Fix for getSubscriptions function in subscriptions.go
// Replace the entire getSubscriptions function with this implementation

// getSubscriptions handles GET /api/2/subscriptions/{username}/{deviceid}
func getSubscriptions(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] getSubscriptions: Starting request processing - %s %s", c.Request.Method, c.Request.URL.Path)

		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] getSubscriptions: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}
		log.Printf("[DEBUG] getSubscriptions: userID found: %v", userID)

		// Get device ID from URL - with fix for .json suffix
		deviceName := c.Param("deviceid")
		// Remove .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] getSubscriptions: Using device name: '%s'", deviceName)

		if deviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required"})
			return
		}

		// Check if this is a subscription changes request (has 'since' parameter)
		sinceStr := c.Query("since")
		if sinceStr != "" {
			// This is a subscription changes request
			var since int64 = 0
			var err error
			since, err = strconv.ParseInt(sinceStr, 10, 64)
			if err != nil {
				log.Printf("[ERROR] getSubscriptions: Invalid since parameter: %s", sinceStr)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid since parameter"})
				return
			}

			log.Printf("[DEBUG] getSubscriptions: Processing as subscription changes request with since: %d", since)

			// Get device ID from database
			var deviceID int
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

			err = database.QueryRow(query, userID, deviceName).Scan(&deviceID)

			if err != nil {
				if err == sql.ErrNoRows {
					// Device doesn't exist, create it
					log.Printf("[DEBUG] getSubscriptions: Device not found, creating new device")

					if database.IsPostgreSQLDB() {
						query = `
                            INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
                            VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
                            RETURNING DeviceID
                        `
						err = database.QueryRow(query, userID, deviceName).Scan(&deviceID)
					} else {
						query = `
						    INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
						    VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
						`
						result, err := database.Exec(query, userID, deviceName)
						if err != nil {
							log.Printf("[ERROR] getSubscriptions: Failed to create device: %v", err)
							c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
							return
						}

						lastID, err := result.LastInsertId()
						if err != nil {
							log.Printf("[ERROR] getSubscriptions: Failed to get last insert ID: %v", err)
							c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
							return
						}

						deviceID = int(lastID)
					}

					if err != nil {
						log.Printf("[ERROR] getSubscriptions: Failed to create device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}
				} else {
					log.Printf("[ERROR] getSubscriptions: Error getting device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
					return
				}
			}

			// If since is 0, this is likely the initial request and we should return all subscriptions
			if since == 0 {
				// Get all podcasts for this user
				var rows *sql.Rows

				if database.IsPostgreSQLDB() {
					query = `SELECT FeedURL FROM "Podcasts" WHERE UserID = $1`
				} else {
					query = `SELECT FeedURL FROM Podcasts WHERE UserID = ?`
				}

				rows, err = database.Query(query, userID)

				if err != nil {
					log.Printf("[ERROR] getSubscriptions: Error querying podcasts: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscriptions"})
					return
				}
				defer rows.Close()

				// Build subscription list - ensure never nil
				podcasts := make([]string, 0)
				for rows.Next() {
					var url string
					if err := rows.Scan(&url); err != nil {
						log.Printf("[ERROR] getSubscriptions: Error scanning podcast URL: %v", err)
						continue
					}
					podcasts = append(podcasts, url)
				}

				if err = rows.Err(); err != nil {
					log.Printf("[ERROR] getSubscriptions: Error iterating podcast rows: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process subscriptions"})
					return
				}

				// Update device's last sync time
				if database.IsPostgreSQLDB() {
					query = `
                        UPDATE "GpodderDevices"
                        SET LastSync = CURRENT_TIMESTAMP
                        WHERE DeviceID = $1
                    `
				} else {
					query = `
                        UPDATE GpodderDevices
                        SET LastSync = CURRENT_TIMESTAMP
                        WHERE DeviceID = ?
                    `
				}

				_, err = database.Exec(query, deviceID)

				if err != nil {
					// Non-critical error, just log it
					log.Printf("[WARNING] Error updating device last sync time: %v", err)
				}

				// Return subscriptions in gpodder format, ensuring backward compatibility
				response := gin.H{
					"add":       podcasts,
					"remove":    []string{},
					"timestamp": time.Now().Unix(),
				}

				log.Printf("[DEBUG] getSubscriptions: Returning initial subscription list with %d podcasts", len(podcasts))
				c.Header("Content-Type", "application/json")
				c.JSON(http.StatusOK, response)
				return
			}

			// Process actual changes since the timestamp
			// Query subscriptions added since the given timestamp - simplified for performance
			var addRows *sql.Rows

			if database.IsPostgreSQLDB() {
				query = `
					SELECT s.PodcastURL
					FROM "GpodderSyncSubscriptions" s
					WHERE s.UserID = $1
					AND s.DeviceID != $2
					AND s.Timestamp > $3
					AND s.Action = 'add'
					GROUP BY s.PodcastURL
					ORDER BY MAX(s.Timestamp) DESC
					LIMIT $4
                `
				log.Printf("[DEBUG] getSubscriptions: Executing add query with limit %d", MAX_SUBSCRIPTION_CHANGES)
				addRows, err = database.Query(query, userID, deviceID, since, MAX_SUBSCRIPTION_CHANGES)
			} else {
				query = `
					SELECT s.PodcastURL
					FROM GpodderSyncSubscriptions s
					WHERE s.UserID = ?
					AND s.DeviceID != ?
					AND s.Timestamp > ?
					AND s.Action = 'add'
					GROUP BY s.PodcastURL
					ORDER BY MAX(s.Timestamp) DESC
					LIMIT ?
                `
				log.Printf("[DEBUG] getSubscriptions: Executing add query with limit %d", MAX_SUBSCRIPTION_CHANGES)
				addRows, err = database.Query(query, userID, deviceID, since, MAX_SUBSCRIPTION_CHANGES)
			}

			if err != nil {
				log.Printf("[ERROR] getSubscriptions: Error querying podcasts to add: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscription changes"})
				return
			}
			defer addRows.Close()

			// Ensure addList is never nil
			addList := make([]string, 0)
			for addRows.Next() {
				var url string
				if err := addRows.Scan(&url); err != nil {
					log.Printf("[ERROR] getSubscriptions: Error scanning podcast URL: %v", err)
					continue
				}
				addList = append(addList, url)
			}

			// Query subscriptions removed since the given timestamp - simplified for performance
			var removeRows *sql.Rows

			if database.IsPostgreSQLDB() {
				query = `
					SELECT s.PodcastURL
					FROM "GpodderSyncSubscriptions" s
					WHERE s.UserID = $1
					AND s.DeviceID != $2
					AND s.Timestamp > $3
					AND s.Action = 'remove'
					GROUP BY s.PodcastURL
					ORDER BY MAX(s.Timestamp) DESC
					LIMIT $4
                `
				removeRows, err = database.Query(query, userID, deviceID, since, MAX_SUBSCRIPTION_CHANGES)
			} else {
				query = `
					SELECT s.PodcastURL
					FROM GpodderSyncSubscriptions s
					WHERE s.UserID = ?
					AND s.DeviceID != ?
					AND s.Timestamp > ?
					AND s.Action = 'remove'
					GROUP BY s.PodcastURL
					ORDER BY MAX(s.Timestamp) DESC
					LIMIT ?
                `
				removeRows, err = database.Query(query, userID, deviceID, since, MAX_SUBSCRIPTION_CHANGES)
			}

			if err != nil {
				log.Printf("[ERROR] getSubscriptions: Error querying podcasts to remove: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscription changes"})
				return
			}
			defer removeRows.Close()

			// Ensure removeList is never nil
			removeList := make([]string, 0)
			for removeRows.Next() {
				var url string
				if err := removeRows.Scan(&url); err != nil {
					log.Printf("[ERROR] getSubscriptions: Error scanning podcast URL: %v", err)
					continue
				}
				removeList = append(removeList, url)
			}

			timestamp := time.Now().Unix()

			// Update device's last sync time
			if database.IsPostgreSQLDB() {
				query = `
                    UPDATE "GpodderDevices"
                    SET LastSync = CURRENT_TIMESTAMP
                    WHERE DeviceID = $1
                `
			} else {
				query = `
                    UPDATE GpodderDevices
                    SET LastSync = CURRENT_TIMESTAMP
                    WHERE DeviceID = ?
                `
			}

			_, err = database.Exec(query, deviceID)

			if err != nil {
				// Non-critical error, just log it
				log.Printf("[WARNING] Error updating device last sync time: %v", err)
			}

			response := gin.H{
				"add":       addList,
				"remove":    removeList,
				"timestamp": timestamp,
			}

			log.Printf("[DEBUG] getSubscriptions: Returning subscription changes - add: %d, remove: %d, timestamp: %d",
				len(addList), len(removeList), timestamp)

			c.Header("Content-Type", "application/json")
			c.JSON(http.StatusOK, response)
			return
		}

		// Regular subscription list request
		// Get device ID from database
		var deviceID int
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

		err := database.QueryRow(query, userID, deviceName).Scan(&deviceID)

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist or is inactive
				log.Printf("[INFO] Device not found or inactive: UserID=%v, DeviceName=%s", userID, deviceName)

				// Create device automatically if it doesn't exist
				if database.IsPostgreSQLDB() {
					query = `
                        INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
                        VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
                        RETURNING DeviceID
                    `
					err = database.QueryRow(query, userID, deviceName).Scan(&deviceID)
				} else {
					query = `
					    INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
					    VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
					`
					result, err := database.Exec(query, userID, deviceName)
					if err != nil {
						log.Printf("[ERROR] Error creating device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					lastID, err := result.LastInsertId()
					if err != nil {
						log.Printf("[ERROR] Error getting last insert ID: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					deviceID = int(lastID)
				}

				if err != nil {
					log.Printf("[ERROR] Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}

				log.Printf("[INFO] Created new device: UserID=%v, DeviceName=%s, DeviceID=%d", userID, deviceName, deviceID)

				// Return empty list for new device
				c.JSON(http.StatusOK, []string{})
				return
			}

			log.Printf("[ERROR] Error getting device ID: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
			return
		}

		// Get podcasts for this user
		var rows *sql.Rows

		if database.IsPostgreSQLDB() {
			query = `SELECT FeedURL FROM "Podcasts" WHERE UserID = $1`
		} else {
			query = `SELECT FeedURL FROM Podcasts WHERE UserID = ?`
		}

		rows, err = database.Query(query, userID)

		if err != nil {
			log.Printf("Error getting podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscriptions"})
			return
		}
		defer rows.Close()

		// Build response - ensure never nil
		urls := make([]string, 0)
		for rows.Next() {
			var url string
			if err := rows.Scan(&url); err != nil {
				log.Printf("[ERROR] Error scanning podcast URL: %v", err)
				continue
			}
			urls = append(urls, url)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process subscriptions"})
			return
		}

		log.Printf("[DEBUG] Found %d podcast subscriptions in database for userID %v", len(urls), userID)

		// Update device's last sync time
		if database.IsPostgreSQLDB() {
			query = `
                UPDATE "GpodderDevices"
                SET LastSync = CURRENT_TIMESTAMP
                WHERE DeviceID = $1
            `
		} else {
			query = `
                UPDATE GpodderDevices
                SET LastSync = CURRENT_TIMESTAMP
                WHERE DeviceID = ?
            `
		}

		_, err = database.Exec(query, deviceID)

		if err != nil {
			// Non-critical error, just log it
			log.Printf("Error updating device last sync time: %v", err)
		}

		// Log before returning
		log.Printf("[DEBUG] getSubscriptions: Returning %d subscription URLs to client", len(urls))
		for i, url := range urls {
			if i < 5 { // Only log first 5 to avoid flooding logs
				log.Printf("[DEBUG] Subscription URL %d: %s", i, url)
			}
		}

		c.JSON(http.StatusOK, urls)
	}
}

// updateSubscriptions handles PUT /api/2/subscriptions/{username}/{deviceid}.json
func updateSubscriptions(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get device ID from URL
		deviceName := c.Param("deviceid")
		if deviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required"})
			return
		}

		// Parse request body - should be a list of URLs
		var urls []string
		if err := c.ShouldBindJSON(&urls); err != nil {
			log.Printf("Error parsing request body: %v", err)
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON array of URLs"})
			return
		}

		// Validate number of subscriptions
		if len(urls) > MAX_SUBSCRIPTIONS {
			c.JSON(http.StatusBadRequest, gin.H{
				"error": fmt.Sprintf("Too many subscriptions. Maximum allowed: %d", MAX_SUBSCRIPTIONS),
			})
			return
		}

		// Get or create device
		var deviceID int
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

		err := database.QueryRow(query, userID, deviceName).Scan(&deviceID)

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist, create it
				if database.IsPostgreSQLDB() {
					query = `
						INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
						VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
						RETURNING DeviceID
					`
					err = database.QueryRow(query, userID, deviceName).Scan(&deviceID)
				} else {
					query = `
					    INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
					    VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
					`
					result, err := database.Exec(query, userID, deviceName)
					if err != nil {
						log.Printf("Error creating device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					lastID, err := result.LastInsertId()
					if err != nil {
						log.Printf("Error getting last insert ID: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					deviceID = int(lastID)
				}

				if err != nil {
					log.Printf("Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}
			} else {
				log.Printf("Error checking device existence: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check device"})
				return
			}
		}

		// Begin transaction
		tx, err := database.Begin()
		if err != nil {
			log.Printf("Error beginning transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to begin transaction"})
			return
		}
		defer func() {
			if err != nil {
				tx.Rollback()
			}
		}()

		// Get existing subscriptions
		var rows *sql.Rows

		if database.IsPostgreSQLDB() {
			query = `SELECT FeedURL FROM "Podcasts" WHERE UserID = $1`
		} else {
			query = `SELECT FeedURL FROM Podcasts WHERE UserID = ?`
		}

		rows, err = tx.Query(query, userID)

		if err != nil {
			log.Printf("Error getting existing podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get existing subscriptions"})
			return
		}

		// Build existing subscriptions map
		existing := make(map[string]bool)
		for rows.Next() {
			var url string
			if err := rows.Scan(&url); err != nil {
				log.Printf("Error scanning existing podcast URL: %v", err)
				continue
			}
			existing[url] = true
		}
		rows.Close()

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating existing podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process existing subscriptions"})
			return
		}

		// Find URLs to add and remove
		toAdd := make([]string, 0)
		cleanURLMap := make(map[string]string) // Maps original URL to cleaned URL

		for _, url := range urls {
			// Clean and validate the URL
			cleanURL, err := sanitizeURL(url)
			if err != nil {
				log.Printf("Skipping invalid URL '%s': %v", url, err)
				continue
			}

			cleanURLMap[url] = cleanURL

			if !existing[cleanURL] {
				toAdd = append(toAdd, cleanURL)
			}

			// Remove from existing map to track what's left to delete
			delete(existing, cleanURL)
		}

		// Remaining URLs in 'existing' need to be removed
		toRemove := make([]string, 0, len(existing))
		for url := range existing {
			toRemove = append(toRemove, url)
		}

		// Record subscription changes
		timestamp := time.Now().Unix()

		// Add new podcasts
		for _, url := range toAdd {
			// Insert into Podcasts table with minimal info
			if database.IsPostgreSQLDB() {
				query = `
					INSERT INTO "Podcasts" (PodcastName, FeedURL, UserID)
					VALUES ($1, $2, $3)
					ON CONFLICT (UserID, FeedURL) DO NOTHING
				`
				_, err = tx.Exec(query, url, url, userID)
			} else {
				query = `
					INSERT IGNORE INTO Podcasts (PodcastName, FeedURL, UserID)
					VALUES (?, ?, ?)
				`
				_, err = tx.Exec(query, url, url, userID)
			}

			if err != nil {
				log.Printf("Error adding podcast: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to add podcast"})
				return
			}

			// Record subscription change
			if database.IsPostgreSQLDB() {
				query = `
					INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES ($1, $2, $3, 'add', $4)
				`
			} else {
				query = `
					INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES (?, ?, ?, 'add', ?)
				`
			}

			_, err = tx.Exec(query, userID, deviceID, url, timestamp)

			if err != nil {
				log.Printf("Error recording subscription add: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to record subscription change"})
				return
			}
		}

		// Remove podcasts
		for _, url := range toRemove {
			// First delete related episodes and their dependencies to avoid foreign key constraint violations
			if database.IsPostgreSQLDB() {
				// Delete related data in correct order for PostgreSQL
				deleteQueries := []string{
					`DELETE FROM "PlaylistContents" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "YouTubeVideos" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)`,
					`DELETE FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)`,
					`DELETE FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2`,
				}
				for _, deleteQuery := range deleteQueries {
					_, err = tx.Exec(deleteQuery, userID, url)
					if err != nil {
						log.Printf("Error executing delete query: %v", err)
						break
					}
				}
			} else {
				// Delete related data in correct order for MySQL/MariaDB
				deleteQueries := []string{
					`DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM YouTubeVideos WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?)`,
					`DELETE FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?)`,
					`DELETE FROM Podcasts WHERE UserID = ? AND FeedURL = ?`,
				}
				for _, deleteQuery := range deleteQueries {
					_, err = tx.Exec(deleteQuery, userID, url)
					if err != nil {
						log.Printf("Error executing delete query: %v", err)
						break
					}
				}
			}

			if err != nil {
				log.Printf("Error removing podcast: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to remove podcast"})
				return
			}

			// Record subscription change
			if database.IsPostgreSQLDB() {
				query = `
					INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES ($1, $2, $3, 'remove', $4)
				`
			} else {
				query = `
					INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES (?, ?, ?, 'remove', ?)
				`
			}

			_, err = tx.Exec(query, userID, deviceID, url, timestamp)

			if err != nil {
				log.Printf("Error recording subscription remove: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to record subscription change"})
				return
			}
		}

		// Update device's last sync time
		if database.IsPostgreSQLDB() {
			query = `
				UPDATE "GpodderDevices"
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = $1
			`
		} else {
			query = `
				UPDATE GpodderDevices
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = ?
			`
		}

		_, err = tx.Exec(query, deviceID)

		if err != nil {
			log.Printf("Error updating device last sync time: %v", err)
			// Non-critical error, continue with transaction
		}

		// Commit transaction
		if err := tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return success
		c.Status(http.StatusOK)
	}
}

// Updated version of uploadSubscriptionChanges to ensure update_urls is always in the response
func uploadSubscriptionChanges(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] uploadSubscriptionChanges: Processing request: %s %s",
			c.Request.Method, c.Request.URL.Path)

		// Get parameters
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] uploadSubscriptionChanges: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		username := c.Param("username")
		deviceName := c.Param("deviceid")

		// Remove .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] uploadSubscriptionChanges: For user %s (ID: %v), device: %s",
			username, userID, deviceName)

		// Parse request
		var changes models.SubscriptionChange
		if err := c.ShouldBindJSON(&changes); err != nil {
			log.Printf("[ERROR] uploadSubscriptionChanges: Failed to parse request body: %v", err)
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON object with 'add' and 'remove' arrays"})
			return
		}

		log.Printf("[DEBUG] uploadSubscriptionChanges: Received changes - add: %d, remove: %d",
			len(changes.Add), len(changes.Remove))

		// Validate request (ensure no duplicate URLs between add and remove)
		addMap := make(map[string]bool)
		for _, url := range changes.Add {
			addMap[url] = true
		}

		for _, url := range changes.Remove {
			if addMap[url] {
				c.JSON(http.StatusBadRequest, gin.H{
					"error": fmt.Sprintf("URL appears in both 'add' and 'remove' arrays: %s", url),
				})
				return
			}
		}

		// Validate number of subscriptions
		if len(changes.Add) > MAX_SUBSCRIPTIONS || len(changes.Remove) > MAX_SUBSCRIPTIONS {
			c.JSON(http.StatusBadRequest, gin.H{
				"error": fmt.Sprintf("Too many subscriptions in request. Maximum allowed: %d", MAX_SUBSCRIPTIONS),
			})
			return
		}

		// Get or create device
		var deviceID int
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

		err := database.QueryRow(query, userID, deviceName).Scan(&deviceID)

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist, create it
				log.Printf("[DEBUG] uploadSubscriptionChanges: Creating new device for user %v: %s", userID, deviceName)

				if database.IsPostgreSQLDB() {
					query = `
						INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
						VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
						RETURNING DeviceID
					`
					err = database.QueryRow(query, userID, deviceName).Scan(&deviceID)
				} else {
					query = `
					    INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
					    VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
					`
					result, err := database.Exec(query, userID, deviceName)
					if err != nil {
						log.Printf("[ERROR] uploadSubscriptionChanges: Error creating device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					lastID, err := result.LastInsertId()
					if err != nil {
						log.Printf("[ERROR] uploadSubscriptionChanges: Error getting last insert ID: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					deviceID = int(lastID)
				}

				if err != nil {
					log.Printf("[ERROR] uploadSubscriptionChanges: Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}
				log.Printf("[DEBUG] uploadSubscriptionChanges: Created new device with ID: %d", deviceID)
			} else {
				log.Printf("[ERROR] uploadSubscriptionChanges: Error checking device existence: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check device"})
				return
			}
		} else {
			log.Printf("[DEBUG] uploadSubscriptionChanges: Using existing device with ID: %d", deviceID)
		}

		// Begin transaction
		tx, err := database.Begin()
		if err != nil {
			log.Printf("[ERROR] uploadSubscriptionChanges: Error beginning transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to begin transaction"})
			return
		}
		defer func() {
			if err != nil {
				tx.Rollback()
			}
		}()

		// Process subscriptions to add
		timestamp := time.Now().Unix()
		updateURLs := make([][]string, 0) // Ensure never nil

		for _, url := range changes.Add {
			// Clean URL
			cleanURL, err := sanitizeURL(url)
			if err != nil {
				log.Printf("[WARNING] uploadSubscriptionChanges: Skipping invalid URL in 'add' array: %s - %v", url, err)
				continue
			}

			// Record changes to database
			if database.IsPostgreSQLDB() {
				query = `
					INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES ($1, $2, $3, 'add', $4)
				`
			} else {
				query = `
					INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES (?, ?, ?, 'add', ?)
				`
			}

			_, err = tx.Exec(query, userID, deviceID, cleanURL, timestamp)

			if err != nil {
				log.Printf("[ERROR] uploadSubscriptionChanges: Error recording subscription add: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to record subscription change"})
				return
			}

			// Check if podcast already exists for this user
			var podcastExists bool

			if database.IsPostgreSQLDB() {
				query = `
					SELECT EXISTS(SELECT 1 FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)
				`
			} else {
				query = `
					SELECT EXISTS(SELECT 1 FROM Podcasts WHERE UserID = ? AND FeedURL = ?)
				`
			}

			err = tx.QueryRow(query, userID, cleanURL).Scan(&podcastExists)

			if err != nil {
				log.Printf("[ERROR] uploadSubscriptionChanges: Error checking podcast existence: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check podcast existence"})
				return
			}

			// Add to Podcasts table if it doesn't exist
			if !podcastExists {
				// Fetch podcast metadata from the feed
				podcastValues, err := utils.GetPodcastValues(cleanURL, userID.(int), "", "")
				if err != nil {
					log.Printf("[WARNING] uploadSubscriptionChanges: Error fetching podcast metadata from %s: %v", cleanURL, err)
					// Continue with minimal data if we can't fetch full metadata
				}

				// Use default values if fetch failed
				if podcastValues == nil {
					// Insert minimal data
					if database.IsPostgreSQLDB() {
						query = `
							INSERT INTO "Podcasts" (PodcastName, FeedURL, UserID)
							VALUES ($1, $2, $3)
						`
					} else {
						query = `
							INSERT INTO Podcasts (PodcastName, FeedURL, UserID)
							VALUES (?, ?, ?)
						`
					}

					_, err = tx.Exec(query, cleanURL, cleanURL, userID)
				} else {
					// Insert with full metadata
					if database.IsPostgreSQLDB() {
						query = `
							INSERT INTO "Podcasts" (
								PodcastName, ArtworkURL, Author, Categories,
								Description, EpisodeCount, FeedURL, WebsiteURL,
								Explicit, UserID
							)
							VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
						`
					} else {
						query = `
							INSERT INTO Podcasts (
								PodcastName, ArtworkURL, Author, Categories,
								Description, EpisodeCount, FeedURL, WebsiteURL,
								Explicit, UserID
							)
							VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
						`
					}

					explicit := 0
					if podcastValues.Explicit {
						explicit = 1
					}

					_, err = tx.Exec(
						query,
						podcastValues.Title,
						podcastValues.ArtworkURL,
						podcastValues.Author,
						podcastValues.Categories,
						podcastValues.Description,
						podcastValues.EpisodeCount,
						cleanURL,
						podcastValues.WebsiteURL,
						explicit,
						userID,
					)
				}

				if err != nil {
					log.Printf("[ERROR] uploadSubscriptionChanges: Error adding podcast: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to add podcast"})
					return
				}
			}

			// If URL was cleaned, add to updateURLs
			if cleanURL != url {
				updateURLs = append(updateURLs, []string{url, cleanURL})
			}
		}

		// Process subscriptions to remove
		for _, url := range changes.Remove {
			// Clean URL
			cleanURL, err := sanitizeURL(url)
			if err != nil {
				log.Printf("[WARNING] uploadSubscriptionChanges: Skipping invalid URL in 'remove' array: %s - %v", url, err)
				continue
			}

			// Record changes to database
			if database.IsPostgreSQLDB() {
				query = `
					INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES ($1, $2, $3, 'remove', $4)
				`
			} else {
				query = `
					INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES (?, ?, ?, 'remove', ?)
				`
			}

			_, err = tx.Exec(query, userID, deviceID, cleanURL, timestamp)

			if err != nil {
				log.Printf("[ERROR] uploadSubscriptionChanges: Error recording subscription remove: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to record subscription change"})
				return
			}

			// First delete related episodes and their dependencies to avoid foreign key constraint violations
			if database.IsPostgreSQLDB() {
				// Delete related data in correct order for PostgreSQL
				deleteQueries := []string{
					`DELETE FROM "PlaylistContents" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "YouTubeVideos" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)`,
					`DELETE FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)`,
					`DELETE FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2`,
				}
				for _, deleteQuery := range deleteQueries {
					_, err = tx.Exec(deleteQuery, userID, cleanURL)
					if err != nil {
						log.Printf("[ERROR] uploadSubscriptionChanges: Error executing delete query: %v", err)
						break
					}
				}
			} else {
				// Delete related data in correct order for MySQL/MariaDB
				deleteQueries := []string{
					`DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM YouTubeVideos WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?)`,
					`DELETE FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?)`,
					`DELETE FROM Podcasts WHERE UserID = ? AND FeedURL = ?`,
				}
				for _, deleteQuery := range deleteQueries {
					_, err = tx.Exec(deleteQuery, userID, cleanURL)
					if err != nil {
						log.Printf("[ERROR] uploadSubscriptionChanges: Error executing delete query: %v", err)
						break
					}
				}
			}

			if err != nil {
				log.Printf("[ERROR] uploadSubscriptionChanges: Error removing podcast: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to remove podcast"})
				return
			}

			// If URL was cleaned, add to updateURLs
			if cleanURL != url {
				updateURLs = append(updateURLs, []string{url, cleanURL})
			}
		}

		// Update device's last sync time
		if database.IsPostgreSQLDB() {
			query = `
				UPDATE "GpodderDevices"
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = $1
			`
		} else {
			query = `
				UPDATE GpodderDevices
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = ?
			`
		}

		_, err = tx.Exec(query, deviceID)

		if err != nil {
			log.Printf("[WARNING] uploadSubscriptionChanges: Error updating device last sync time: %v", err)
			// Non-critical error, continue with transaction
		}

		// Commit transaction
		if err := tx.Commit(); err != nil {
			log.Printf("[ERROR] uploadSubscriptionChanges: Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		log.Printf("[DEBUG] uploadSubscriptionChanges: Successfully processed changes - add: %d, remove: %d",
			len(changes.Add), len(changes.Remove))

		// CRITICAL: Always include update_urls in response, even if empty
		// AntennaPod specifically checks for existence of this field
		var response gin.H
		if updateURLs == nil || len(updateURLs) == 0 {
			// Ensure an empty array is returned, not null or missing
			response = gin.H{
				"timestamp":   timestamp,
				"update_urls": [][]string{}, // Empty array
			}
		} else {
			response = gin.H{
				"timestamp":   timestamp,
				"update_urls": updateURLs,
			}
		}

		log.Printf("[DEBUG] uploadSubscriptionChanges: Returning response with timestamp %d and %d update URLs",
			timestamp, len(updateURLs))

		// Return response
		c.JSON(http.StatusOK, response)
	}
}

// getAllSubscriptions handles GET /api/2/subscriptions/{username}.json
func getAllSubscriptions(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get all podcasts for this user
		var query string

		if database.IsPostgreSQLDB() {
			query = `SELECT FeedURL FROM "Podcasts" WHERE UserID = $1`
		} else {
			query = `SELECT FeedURL FROM Podcasts WHERE UserID = ?`
		}

		rows, err := database.Query(query, userID)
		if err != nil {
			log.Printf("Error getting podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscriptions"})
			return
		}
		defer rows.Close()

		// Build response - ensure never nil
		urls := make([]string, 0)
		for rows.Next() {
			var url string
			if err := rows.Scan(&url); err != nil {
				log.Printf("Error scanning podcast URL: %v", err)
				continue
			}
			urls = append(urls, url)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process subscriptions"})
			return
		}

		c.JSON(http.StatusOK, urls)
	}
}

// getSubscriptionsSimple handles GET /subscriptions/{username}/{deviceid}.{format}
func getSubscriptionsSimple(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get format from URL
		format := c.Param("format")
		if format == "" {
			format = "json" // Default format
		}

		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get device ID from URL
		deviceName := c.Param("deviceid")
		if deviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required"})
			return
		}

		// Get device ID from database
		var deviceID int
		var err error

		if database.IsPostgreSQLDB() {
			err = database.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
			`, userID, deviceName).Scan(&deviceID)
		} else {
			err = database.QueryRow(`
				SELECT DeviceID FROM GpodderDevices
				WHERE UserID = ? AND DeviceName = ? AND IsActive = true
			`, userID, deviceName).Scan(&deviceID)
		}

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist or is inactive, create it
				if database.IsPostgreSQLDB() {
					err = database.QueryRow(`
				        INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
				        VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
				        RETURNING DeviceID
				    `, userID, deviceName).Scan(&deviceID)
				} else {
					// For MySQL, define the query string first
					var query string = `
				        INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
				        VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
				    `
					result, err := database.Exec(query, userID, deviceName)
					if err != nil {
						log.Printf("Error creating device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}
					lastID, err := result.LastInsertId()
					if err != nil {
						log.Printf("Error getting last insert ID: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}
					deviceID = int(lastID)
				}

				if err != nil {
					log.Printf("Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}

				// Return empty list for new device
				c.JSON(http.StatusOK, []string{})
				return
			}

			log.Printf("Error getting device ID: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
			return
		}

		// Get podcasts for this user
		var rows *sql.Rows

		if database.IsPostgreSQLDB() {
			rows, err = database.Query(`
				SELECT FeedURL FROM "Podcasts" WHERE UserID = $1
			`, userID)
		} else {
			rows, err = database.Query(`
				SELECT FeedURL FROM Podcasts WHERE UserID = ?
			`, userID)
		}

		if err != nil {
			log.Printf("Error getting podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscriptions"})
			return
		}
		defer rows.Close()

		// Build response - ensure never nil
		urls := make([]string, 0)
		for rows.Next() {
			var url string
			if err := rows.Scan(&url); err != nil {
				log.Printf("Error scanning podcast URL: %v", err)
				continue
			}
			urls = append(urls, url)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process subscriptions"})
			return
		}

		// Update device's last sync time
		if database.IsPostgreSQLDB() {
			_, err = database.Exec(`
				UPDATE "GpodderDevices"
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = $1
			`, deviceID)
		} else {
			_, err = database.Exec(`
				UPDATE GpodderDevices
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = ?
			`, deviceID)
		}

		if err != nil {
			// Non-critical error, just log it
			log.Printf("Error updating device last sync time: %v", err)
		}

		// Return in requested format
		switch format {
		case "json", "jsonp":
			if format == "jsonp" {
				// JSONP callback
				callback := c.Query("jsonp")
				if callback == "" {
					callback = "callback" // Default callback name
				}
				c.Header("Content-Type", "application/javascript")
				jsonData, err := json.Marshal(urls)
				if err != nil {
					log.Printf("Error marshaling JSON: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to marshal response"})
					return
				}
				c.String(http.StatusOK, "%s(%s);", callback, string(jsonData))
			} else {
				c.JSON(http.StatusOK, urls)
			}
		case "txt":
			// Plain text format
			c.Header("Content-Type", "text/plain")
			var sb strings.Builder
			for _, url := range urls {
				sb.WriteString(url)
				sb.WriteString("\n")
			}
			c.String(http.StatusOK, sb.String())
		case "opml":
			// OPML format
			c.Header("Content-Type", "text/xml")
			var sb strings.Builder
			sb.WriteString(`<?xml version="1.0" encoding="utf-8"?>
<opml version="1.0">
  <head>
    <title>gPodder Subscriptions</title>
  </head>
  <body>
`)
			for _, url := range urls {
				sb.WriteString(fmt.Sprintf(`    <outline text="%s" type="rss" xmlUrl="%s" />
`, url, url))
			}
			sb.WriteString(`  </body>
</opml>`)
			c.String(http.StatusOK, sb.String())
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
		}
	}
}

// updateSubscriptionsSimple handles PUT /subscriptions/{username}/{deviceid}.{format}
func updateSubscriptionsSimple(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get format from URL
		format := c.Param("format")
		if format == "" {
			format = "json" // Default format
		}

		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get device ID from URL
		deviceName := c.Param("deviceid")
		if deviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required"})
			return
		}

		// Parse request body based on format
		var urls []string

		switch format {
		case "json", "jsonp":
			if err := c.ShouldBindJSON(&urls); err != nil {
				log.Printf("Error parsing JSON request body: %v", err)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON array of URLs"})
				return
			}
		case "txt":
			// Read as plain text, split by lines
			body, err := c.GetRawData()
			if err != nil {
				log.Printf("Error reading request body: %v", err)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Failed to read request body"})
				return
			}

			lines := strings.Split(string(body), "\n")
			for _, line := range lines {
				line = strings.TrimSpace(line)
				if line != "" {
					urls = append(urls, line)
				}
			}
		case "opml":
			// Parse OPML format
			body, err := c.GetRawData()
			if err != nil {
				log.Printf("Error reading request body: %v", err)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Failed to read request body"})
				return
			}

			// Simple regex-based OPML parser (for a robust implementation, use proper XML parsing)
			opmlContent := string(body)
			matches := opmlOutlineRegex.FindAllStringSubmatch(opmlContent, -1)

			for _, match := range matches {
				if len(match) > 1 {
					url := match[1]
					if url != "" {
						urls = append(urls, url)
					}
				}
			}
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
			return
		}

		// Validate number of subscriptions
		if len(urls) > MAX_SUBSCRIPTIONS {
			c.JSON(http.StatusBadRequest, gin.H{
				"error": fmt.Sprintf("Too many subscriptions. Maximum allowed: %d", MAX_SUBSCRIPTIONS),
			})
			return
		}

		// From here, use the same logic as updateSubscriptions
		// Get or create device, process changes, etc.
		var deviceID int
		var err error

		if database.IsPostgreSQLDB() {
			err = database.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2
			`, userID, deviceName).Scan(&deviceID)
		} else {
			err = database.QueryRow(`
				SELECT DeviceID FROM GpodderDevices
				WHERE UserID = ? AND DeviceName = ?
			`, userID, deviceName).Scan(&deviceID)
		}

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist, create it
				if database.IsPostgreSQLDB() {
					err = database.QueryRow(`
						INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
						VALUES ($1, $2, 'other', true, CURRENT_TIMESTAMP)
						RETURNING DeviceID
					`, userID, deviceName).Scan(&deviceID)
				} else {
					// For MySQL, we need to use a different approach without RETURNING
					res, err := database.Exec(`
						INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsActive, LastSync)
						VALUES (?, ?, 'other', true, CURRENT_TIMESTAMP)
					`, userID, deviceName)

					if err != nil {
						log.Printf("Error creating device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}

					// Get the last inserted ID
					lastID, err := res.LastInsertId()
					if err != nil {
						log.Printf("Error getting last insert ID: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device ID"})
						return
					}

					deviceID = int(lastID)
				}

				if err != nil {
					log.Printf("Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}
			} else {
				log.Printf("Error checking device existence: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check device"})
				return
			}
		}

		// Begin transaction
		tx, err := database.Begin()
		if err != nil {
			log.Printf("Error beginning transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to begin transaction"})
			return
		}
		defer func() {
			if err != nil {
				tx.Rollback()
			}
		}()

		// Get existing subscriptions
		var rows *sql.Rows

		if database.IsPostgreSQLDB() {
			rows, err = tx.Query(`
				SELECT FeedURL FROM "Podcasts" WHERE UserID = $1
			`, userID)
		} else {
			rows, err = tx.Query(`
				SELECT FeedURL FROM Podcasts WHERE UserID = ?
			`, userID)
		}

		if err != nil {
			log.Printf("Error getting existing podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get existing subscriptions"})
			return
		}

		// Build existing subscriptions map
		existing := make(map[string]bool)
		for rows.Next() {
			var url string
			if err := rows.Scan(&url); err != nil {
				log.Printf("Error scanning existing podcast URL: %v", err)
				continue
			}
			existing[url] = true
		}
		rows.Close()

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating existing podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process existing subscriptions"})
			return
		}

		// Find URLs to add and remove
		toAdd := make([]string, 0)
		cleanURLMap := make(map[string]string) // Maps original URL to cleaned URL

		for _, url := range urls {
			// Clean and validate the URL
			cleanURL, err := sanitizeURL(url)
			if err != nil {
				log.Printf("Skipping invalid URL '%s': %v", url, err)
				continue
			}

			cleanURLMap[url] = cleanURL

			if !existing[cleanURL] {
				toAdd = append(toAdd, cleanURL)
			}

			// Remove from existing map to track what's left to delete
			delete(existing, cleanURL)
		}

		// Remaining URLs in 'existing' need to be removed
		toRemove := make([]string, 0, len(existing))
		for url := range existing {
			toRemove = append(toRemove, url)
		}

		// Record subscription changes
		timestamp := time.Now().Unix()

		// Add new podcasts
		for _, url := range toAdd {
			// Insert into Podcasts table with minimal info
			if database.IsPostgreSQLDB() {
				_, err = tx.Exec(`
					INSERT INTO "Podcasts" (PodcastName, FeedURL, UserID)
					VALUES ($1, $2, $3)
					ON CONFLICT (UserID, FeedURL) DO NOTHING
				`, url, url, userID)
			} else {
				// For MySQL, use INSERT IGNORE
				_, err = tx.Exec(`
					INSERT IGNORE INTO Podcasts (PodcastName, FeedURL, UserID)
					VALUES (?, ?, ?)
				`, url, url, userID)
			}

			if err != nil {
				log.Printf("Error adding podcast: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to add podcast"})
				return
			}

			// Record subscription change
			if database.IsPostgreSQLDB() {
				_, err = tx.Exec(`
					INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES ($1, $2, $3, 'add', $4)
				`, userID, deviceID, url, timestamp)
			} else {
				_, err = tx.Exec(`
					INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES (?, ?, ?, 'add', ?)
				`, userID, deviceID, url, timestamp)
			}

			if err != nil {
				log.Printf("Error recording subscription add: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to record subscription change"})
				return
			}
		}

		// Remove podcasts
		for _, url := range toRemove {
			// First delete related episodes and their dependencies to avoid foreign key constraint violations
			if database.IsPostgreSQLDB() {
				// Delete related data in correct order for PostgreSQL
				deleteQueries := []string{
					`DELETE FROM "PlaylistContents" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2))`,
					`DELETE FROM "YouTubeVideos" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)`,
					`DELETE FROM "Episodes" WHERE PodcastID IN (SELECT PodcastID FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2)`,
					`DELETE FROM "Podcasts" WHERE UserID = $1 AND FeedURL = $2`,
				}
				for _, deleteQuery := range deleteQueries {
					_, err = tx.Exec(deleteQuery, userID, url)
					if err != nil {
						log.Printf("Error executing delete query: %v", err)
						break
					}
				}
			} else {
				// Delete related data in correct order for MySQL/MariaDB
				deleteQueries := []string{
					`DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?))`,
					`DELETE FROM YouTubeVideos WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?)`,
					`DELETE FROM Episodes WHERE PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?)`,
					`DELETE FROM Podcasts WHERE UserID = ? AND FeedURL = ?`,
				}
				for _, deleteQuery := range deleteQueries {
					_, err = tx.Exec(deleteQuery, userID, url)
					if err != nil {
						log.Printf("Error executing delete query: %v", err)
						break
					}
				}
			}

			if err != nil {
				log.Printf("Error removing podcast: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to remove podcast"})
				return
			}

			// Record subscription change
			if database.IsPostgreSQLDB() {
				_, err = tx.Exec(`
					INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES ($1, $2, $3, 'remove', $4)
				`, userID, deviceID, url, timestamp)
			} else {
				_, err = tx.Exec(`
					INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp)
					VALUES (?, ?, ?, 'remove', ?)
				`, userID, deviceID, url, timestamp)
			}

			if err != nil {
				log.Printf("Error recording subscription remove: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to record subscription change"})
				return
			}
		}

		// Update device's last sync time
		if database.IsPostgreSQLDB() {
			_, err = tx.Exec(`
				UPDATE "GpodderDevices"
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = $1
			`, deviceID)
		} else {
			_, err = tx.Exec(`
				UPDATE GpodderDevices
				SET LastSync = CURRENT_TIMESTAMP
				WHERE DeviceID = ?
			`, deviceID)
		}

		if err != nil {
			log.Printf("Error updating device last sync time: %v", err)
			// Non-critical error, continue with transaction
		}

		// Commit transaction
		if err := tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return success
		c.Status(http.StatusOK)
	}
}

// Regex for parsing OPML outline tags
var opmlOutlineRegex = regexp.MustCompile(`<outline[^>]*xmlUrl="([^"]+)"[^>]*/>`)
