package api

import (
	"database/sql"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"

	"github.com/gin-gonic/gin"
)

// ValidDeviceTypes contains the allowed device types according to the gpodder API
var ValidDeviceTypes = map[string]bool{
	"desktop": true,
	"laptop":  true,
	"mobile":  true,
	"server":  true,
	"other":   true,
}

func listDevices(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] listDevices handling request: %s %s", c.Request.Method, c.Request.URL.Path)

		// Log headers for debugging
		headers := c.Request.Header
		for name, values := range headers {
			for _, value := range values {
				log.Printf("[DEBUG] Header: %s: %s", name, value)
			}
		}

		// Log cookies
		cookies := c.Request.Cookies()
		for _, cookie := range cookies {
			log.Printf("[DEBUG] Cookie: %s: %s", cookie.Name, cookie.Value)
		}

		// Get user ID from context
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] listDevices: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		log.Printf("[DEBUG] listDevices called for user ID: %v", userID)

		// Query devices from the database
		log.Printf("[DEBUG] listDevices: Querying devices for userID: %v", userID)

		// The key change is here - use COALESCE to handle NULL values for DeviceCaption
		rows, err := database.Query(`
            SELECT d.DeviceID, d.DeviceName, d.DeviceType,
                   COALESCE(d.DeviceCaption, '') as DeviceCaption, d.IsActive,
                   COALESCE(
                       (SELECT COUNT(p.PodcastID)
                        FROM "Podcasts" p
                        WHERE p.UserID = $1),
                       0
                   ) as subscription_count
            FROM "GpodderDevices" d
            WHERE d.UserID = $1 AND d.IsActive = true
        `, userID)

		if err != nil {
			log.Printf("[ERROR] listDevices: Error querying devices: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get devices"})
			return
		}
		defer rows.Close()

		var devices []models.GpodderDevice
		for rows.Next() {
			var device models.GpodderDevice
			var isActive bool

			if err := rows.Scan(
				&device.DeviceID,
				&device.DeviceName,
				&device.DeviceType,
				&device.DeviceCaption,
				&isActive,
				&device.Subscriptions,
			); err != nil {
				log.Printf("[ERROR] listDevices: Error scanning device row: %v", err)
				continue // Continue instead of returning to try to get at least some devices
			}

			// Only add active devices
			if isActive {
				log.Printf("[DEBUG] listDevices: Found active device: %s (ID: %d)",
					device.DeviceName, device.DeviceID)
				devices = append(devices, device)
			}
		}

		if err := rows.Err(); err != nil {
			log.Printf("[ERROR] listDevices: Error iterating device rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get devices"})
			return
		}

		// If no devices found, return empty array rather than error
		if len(devices) == 0 {
			log.Printf("[DEBUG] listDevices: No devices found for userID: %v", userID)
			c.JSON(http.StatusOK, []models.GpodderDevice{})
			return
		}

		log.Printf("[DEBUG] listDevices: Returning %d devices for userID: %v", len(devices), userID)

		// Return the list of devices
		c.JSON(http.StatusOK, devices)
	}
}

// updateDeviceData handles POST /api/2/devices/{username}/{deviceid}.json
func updateDeviceData(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] updateDeviceData handling request: %s %s", c.Request.Method, c.Request.URL.Path)

		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			log.Printf("[ERROR] updateDeviceData: userID not found in context")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}
		log.Printf("[DEBUG] All URL parameters: %v", c.Params)
		// Get device ID from URL
		// Get device ID from URL - use the correct parameter name
		// Get device name from URL with fix for .json suffix
		deviceName := c.Param("deviceid")
		// Also try alternative parameter name if needed
		if deviceName == "" {
			deviceName = c.Param("deviceid.json")
		}

		// Remove .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] updateDeviceData: Using device name: '%s'", deviceName)

		// Additionally, strip .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] updateDeviceData: Got device name from URL param: '%s'", deviceName)

		if deviceName == "" {
			log.Printf("[ERROR] updateDeviceData: Device ID is required")
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required"})
			return
		}

		// Parse request body
		var req struct {
			Caption string `json:"caption"`
			Type    string `json:"type"`
		}
		if err := c.ShouldBindJSON(&req); err != nil {
			log.Printf("[ERROR] updateDeviceData: Error parsing request body: %v", err)
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON object with 'caption' and 'type'"})
			return
		}

		log.Printf("[DEBUG] updateDeviceData: Device info - Name: %s, Caption: %s, Type: %s",
			deviceName, req.Caption, req.Type)

		// Validate device type if provided
		if req.Type != "" && !ValidDeviceTypes[req.Type] {
			log.Printf("[ERROR] updateDeviceData: Invalid device type: %s", req.Type)
			c.JSON(http.StatusBadRequest, gin.H{
				"error": fmt.Sprintf("Invalid device type: %s. Valid types are: desktop, laptop, mobile, server, other", req.Type),
			})
			return
		}

		// If type is empty, set to default 'other'
		if req.Type == "" {
			req.Type = "other"
		}

		// Begin transaction
		tx, err := database.Begin()
		if err != nil {
			log.Printf("[ERROR] updateDeviceData: Error beginning transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to begin transaction"})
			return
		}
		defer func() {
			if err != nil {
				tx.Rollback()
			}
		}()

		// Check if device exists
		var deviceID int
		log.Printf("[DEBUG] updateDeviceData: Checking if device exists - UserID: %v, DeviceName: %s", userID, deviceName)
		err = tx.QueryRow(`
            SELECT DeviceID FROM "GpodderDevices"
            WHERE UserID = $1 AND DeviceName = $2
        `, userID, deviceName).Scan(&deviceID)

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist, create it
				log.Printf("[DEBUG] updateDeviceData: Creating new device - UserID: %v, DeviceName: %s, Type: %s",
					userID, deviceName, req.Type)
				err = tx.QueryRow(`
                    INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, DeviceCaption, IsActive, LastSync)
                    VALUES ($1, $2, $3, $4, true, $5)
                    RETURNING DeviceID
                `, userID, deviceName, req.Type, req.Caption, time.Now()).Scan(&deviceID)

				if err != nil {
					log.Printf("[ERROR] updateDeviceData: Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}

				log.Printf("[DEBUG] updateDeviceData: Created new device with ID: %d", deviceID)

				// Also create entry in device state table
				_, err = tx.Exec(`
                    INSERT INTO "GpodderSyncDeviceState" (UserID, DeviceID)
                    VALUES ($1, $2)
                    ON CONFLICT (UserID, DeviceID) DO NOTHING
                `, userID, deviceID)

				if err != nil {
					log.Printf("[ERROR] updateDeviceData: Error creating device state: %v", err)
					// Not fatal, continue
				}
			} else {
				log.Printf("[ERROR] updateDeviceData: Error checking device existence: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check device"})
				return
			}
		} else {
			// Device exists, update it
			log.Printf("[DEBUG] updateDeviceData: Updating existing device with ID: %d", deviceID)
			_, err = tx.Exec(`
                UPDATE "GpodderDevices"
                SET DeviceType = $1, DeviceCaption = $2, LastSync = $3, IsActive = true
                WHERE DeviceID = $4
            `, req.Type, req.Caption, time.Now(), deviceID)

			if err != nil {
				log.Printf("[ERROR] updateDeviceData: Error updating device: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to update device"})
				return
			}
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("[ERROR] updateDeviceData: Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return empty response with 200 status code as per gpodder API
		log.Printf("[DEBUG] updateDeviceData: Successfully processed device request")
		c.JSON(http.StatusOK, gin.H{})
	}
}

// getDeviceUpdates handles GET /api/2/updates/{username}/{deviceid}.json
func getDeviceUpdates(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] getDeviceUpdates: Processing request: %s %s",
			c.Request.Method, c.Request.URL.Path)
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get device name from URL
		// Get device name from URL with fix for .json suffix
		// Get device name from URL with fix for .json suffix
		deviceName := c.Param("deviceid")
		// Also try alternative parameter name if needed
		if deviceName == "" {
			deviceName = c.Param("deviceid.json")
		}

		// Remove .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] getDeviceUpdates: Using device name: '%s'", deviceName)

		// Remove .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] getDeviceUpdates: Using device name: '%s'", deviceName)

		// Parse query parameters
		sinceStr := c.Query("since")
		includeActions := c.Query("include_actions") == "true"

		var since int64 = 0
		if sinceStr != "" {
			_, err := fmt.Sscanf(sinceStr, "%d", &since)
			if err != nil {
				log.Printf("Invalid 'since' parameter: %s - %v", sinceStr, err)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid 'since' parameter: must be a Unix timestamp"})
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

		// Get or create the device
		var deviceID int
		err = tx.QueryRow(`
			SELECT DeviceID FROM "GpodderDevices"
			WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
		`, userID, deviceName).Scan(&deviceID)

		if err != nil {
			if err == sql.ErrNoRows {
				// Device doesn't exist or is inactive, create it
				log.Printf("Creating new device for updates: %s", deviceName)
				err = tx.QueryRow(`
					INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
					VALUES ($1, $2, 'other', true, $3)
					RETURNING DeviceID
				`, userID, deviceName, time.Now()).Scan(&deviceID)

				if err != nil {
					log.Printf("Error creating device: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
					return
				}

				// Also create entry in device state table
				_, err = tx.Exec(`
					INSERT INTO "GpodderSyncDeviceState" (UserID, DeviceID)
					VALUES ($1, $2)
					ON CONFLICT (UserID, DeviceID) DO NOTHING
				`, userID, deviceID)

				if err != nil {
					log.Printf("Error creating device state: %v", err)
					// Not fatal, continue
				}
			} else {
				log.Printf("Error getting device ID: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
				return
			}
		}

		// Get the current timestamp for the response
		timestamp := time.Now().Unix()

		// Build the response structure
		response := models.DeviceUpdateResponse{
			Add:       []models.Podcast{},
			Remove:    []string{},
			Updates:   []models.Episode{},
			Timestamp: timestamp,
		}

		// Only process updates if a since timestamp was provided
		if since > 0 {
			// Get the last sync timestamp for this device
			var lastSync int64
			err = tx.QueryRow(`
				SELECT COALESCE(LastTimestamp, 0)
				FROM "GpodderSyncState"
				WHERE UserID = $1 AND DeviceID = $2
			`, userID, deviceID).Scan(&lastSync)

			if err != nil && err != sql.ErrNoRows {
				log.Printf("Error getting last sync timestamp: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get sync state"})
				return
			}

			// Handle podcasts to add (subscribed on other devices since the timestamp)
			addRows, err := tx.Query(`
				SELECT DISTINCT p.FeedURL, p.PodcastName, p.Description, p.Author, p.ArtworkURL, p.WebsiteURL,
					  (SELECT COUNT(*) FROM "Podcasts" WHERE FeedURL = p.FeedURL) as subscribers
				FROM "Podcasts" p
				JOIN "GpodderSyncSubscriptions" s ON p.FeedURL = s.PodcastURL
				WHERE s.UserID = $1
				  AND s.DeviceID != $2
				  AND s.Timestamp > $3
				  AND s.Action = 'add'
				  AND NOT EXISTS (
					SELECT 1 FROM "GpodderSyncSubscriptions" s2
					WHERE s2.UserID = s.UserID
					  AND s2.PodcastURL = s.PodcastURL
					  AND s2.DeviceID = $2
					  AND s2.Timestamp > s.Timestamp
					  AND s2.Action = 'add'
				  )
			`, userID, deviceID, since)

			if err != nil {
				log.Printf("Error getting podcasts to add: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get updates"})
				return
			}
			defer addRows.Close()

			for addRows.Next() {
				var podcast models.Podcast
				var podcastName, description, author, artworkURL, websiteURL sql.NullString
				var subscribers int

				err := addRows.Scan(
					&podcast.URL,
					&podcastName,
					&description,
					&author,
					&artworkURL,
					&websiteURL,
					&subscribers,
				)
				if err != nil {
					log.Printf("Error scanning podcast row: %v", err)
					continue
				}

				// Set title - default to URL if name is null
				if podcastName.Valid && podcastName.String != "" {
					podcast.Title = podcastName.String
				} else {
					podcast.Title = podcast.URL
				}

				// Set optional fields if present
				if description.Valid {
					podcast.Description = description.String
				}
				if author.Valid {
					podcast.Author = author.String
				}
				if artworkURL.Valid {
					podcast.LogoURL = artworkURL.String
				}
				if websiteURL.Valid {
					podcast.Website = websiteURL.String
				}

				podcast.Subscribers = subscribers
				podcast.MygpoLink = fmt.Sprintf("/podcast/%s", podcast.URL)

				// Add the podcast to the response
				response.Add = append(response.Add, podcast)
			}

			if err = addRows.Err(); err != nil {
				log.Printf("Error iterating add rows: %v", err)
				// Continue processing other updates
			}

			// Query podcasts to remove (unsubscribed on other devices)
			removeRows, err := tx.Query(`
				SELECT DISTINCT s.PodcastURL
				FROM "GpodderSyncSubscriptions" s
				WHERE s.UserID = $1
				  AND s.DeviceID != $2
				  AND s.Timestamp > $3
				  AND s.Action = 'remove'
				  AND NOT EXISTS (
					SELECT 1 FROM "GpodderSyncSubscriptions" s2
					WHERE s2.UserID = s.UserID
					  AND s2.PodcastURL = s.PodcastURL
					  AND s2.DeviceID = $2
					  AND s2.Timestamp > s.Timestamp
					  AND s2.Action = 'add'
				  )
			`, userID, deviceID, since)

			if err != nil {
				log.Printf("Error getting podcasts to remove: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get updates"})
				return
			}
			defer removeRows.Close()

			for removeRows.Next() {
				var podcastURL string
				err := removeRows.Scan(&podcastURL)
				if err != nil {
					log.Printf("Error scanning podcast URL: %v", err)
					continue
				}

				// Add the podcast URL to the response
				response.Remove = append(response.Remove, podcastURL)
			}

			if err = removeRows.Err(); err != nil {
				log.Printf("Error iterating remove rows: %v", err)
				// Continue processing other updates
			}

			// Query episode updates (if includeActions is true)
			if includeActions {
				updateRows, err := tx.Query(`
					SELECT e.EpisodeTitle, e.EpisodeURL, p.PodcastName, p.FeedURL,
						   e.EpisodeDescription, e.EpisodeURL, e.EpisodePubDate,
						   a.Action, a.Position, a.Total, a.Started
					FROM "GpodderSyncEpisodeActions" a
					JOIN "Episodes" e ON a.EpisodeURL = e.EpisodeURL
					JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
					WHERE a.UserID = $1
					  AND a.Timestamp > $2
					  AND a.Action != 'new'
					ORDER BY a.Timestamp DESC
				`, userID, since)

				if err != nil {
					log.Printf("Error getting episode updates: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get episode updates"})
					return
				}
				defer updateRows.Close()

				for updateRows.Next() {
					var episode models.Episode
					var pubDate time.Time
					var action string
					var position, total, started sql.NullInt64

					err := updateRows.Scan(
						&episode.Title,
						&episode.URL,
						&episode.PodcastTitle,
						&episode.PodcastURL,
						&episode.Description,
						&episode.Website,
						&pubDate,
						&action,
						&position,
						&total,
						&started,
					)
					if err != nil {
						log.Printf("Error scanning episode row: %v", err)
						continue
					}

					// Format the publication date in ISO 8601 format
					episode.Released = pubDate.Format(time.RFC3339)

					// Add the episode to the response
					response.Updates = append(response.Updates, episode)
				}

				if err = updateRows.Err(); err != nil {
					log.Printf("Error iterating episode update rows: %v", err)
					// Continue with other processing
				}
			}
		}

		// Update the last sync timestamp for this device
		_, err = tx.Exec(`
			INSERT INTO "GpodderSyncState" (UserID, DeviceID, LastTimestamp)
			VALUES ($1, $2, $3)
			ON CONFLICT (UserID, DeviceID)
			DO UPDATE SET LastTimestamp = $3
		`, userID, deviceID, timestamp)

		if err != nil {
			log.Printf("Error updating device sync state: %v", err)
			// Not fatal, continue with response
		}

		// Update the device LastSync
		_, err = tx.Exec(`
			UPDATE "GpodderDevices"
			SET LastSync = $1
			WHERE DeviceID = $2
		`, time.Now(), deviceID)

		if err != nil {
			log.Printf("Error updating device last sync time: %v", err)
			// Non-critical error, continue
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return the response
		c.JSON(http.StatusOK, response)
	}
}

// deactivateDevice handles DELETE /api/2/devices/{username}/{deviceid}.json
// This is an extension to the gpodder API for device management
func deactivateDevice(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get device name from URL
		// Get device name from URL with fix for .json suffix
		deviceName := c.Param("deviceid")
		// Also try alternative parameter name if needed
		if deviceName == "" {
			deviceName = c.Param("deviceid.json")
		}

		// Remove .json suffix if present
		if strings.HasSuffix(deviceName, ".json") {
			deviceName = strings.TrimSuffix(deviceName, ".json")
		}

		log.Printf("[DEBUG] deactivateDevice: Using device name: '%s'", deviceName)

		// Get the device ID
		var deviceID int
		err := database.QueryRow(`
			SELECT DeviceID FROM "GpodderDevices"
			WHERE UserID = $1 AND DeviceName = $2
		`, userID, deviceName).Scan(&deviceID)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "Device not found"})
			} else {
				log.Printf("Error getting device ID: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
			}
			return
		}

		// Deactivate the device (rather than delete, to preserve history)
		_, err = database.Exec(`
			UPDATE "GpodderDevices"
			SET IsActive = false
			WHERE DeviceID = $1
		`, deviceID)

		if err != nil {
			log.Printf("Error deactivating device: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to deactivate device"})
			return
		}

		// Return success
		c.JSON(http.StatusOK, gin.H{
			"result":  "success",
			"message": "Device deactivated",
		})
	}
}

// renameDevice handles PUT /api/2/devices/{username}/{deviceid}/rename.json
// This is an extension to the gpodder API for device management
func renameDevice(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get device name from URL
		oldDeviceName := c.Param("deviceid")
		if oldDeviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required"})
			return
		}

		// Parse request body
		var req struct {
			NewDeviceName string `json:"new_deviceid"`
		}
		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON object with 'new_deviceid'"})
			return
		}

		if req.NewDeviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "New device ID is required"})
			return
		}

		// Check if the new device name already exists
		var existingCount int
		err := database.QueryRow(`
			SELECT COUNT(*) FROM "GpodderDevices"
			WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
		`, userID, req.NewDeviceName).Scan(&existingCount)

		if err != nil {
			log.Printf("Error checking for existing device: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check for existing device"})
			return
		}

		if existingCount > 0 {
			c.JSON(http.StatusConflict, gin.H{"error": "Device with this name already exists"})
			return
		}

		// Update the device name
		result, err := database.Exec(`
			UPDATE "GpodderDevices"
			SET DeviceName = $1, LastSync = $2
			WHERE UserID = $3 AND DeviceName = $4 AND IsActive = true
		`, req.NewDeviceName, time.Now(), userID, oldDeviceName)

		if err != nil {
			log.Printf("Error renaming device: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to rename device"})
			return
		}

		rowsAffected, err := result.RowsAffected()
		if err != nil {
			log.Printf("Error getting rows affected: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get operation result"})
			return
		}

		if rowsAffected == 0 {
			c.JSON(http.StatusNotFound, gin.H{"error": "Device not found or not active"})
			return
		}

		// Return success
		c.JSON(http.StatusOK, gin.H{
			"result":  "success",
			"message": "Device renamed successfully",
		})
	}
}

// deviceSync represents the synchronization state of a device
type deviceSync struct {
	LastSync    time.Time `json:"last_sync"`
	DeviceID    int       `json:"-"`
	DeviceName  string    `json:"device_id"`
	DeviceType  string    `json:"device_type"`
	IsActive    bool      `json:"-"`
	SyncEnabled bool      `json:"sync_enabled"`
}

// getDeviceSyncStatus handles GET /api/2/devices/{username}/sync.json
// This is an extension to the gpodder API for device sync status
func getDeviceSyncStatus(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Query all devices and their sync status
		rows, err := database.Query(`
			SELECT d.DeviceID, d.DeviceName, d.DeviceType, d.LastSync, d.IsActive,
				   EXISTS (
					   SELECT 1 FROM "GpodderSyncDevicePairs" p
					   WHERE (p.DeviceID1 = d.DeviceID OR p.DeviceID2 = d.DeviceID)
						 AND p.UserID = d.UserID
				   ) as sync_enabled
			FROM "GpodderDevices" d
			WHERE d.UserID = $1
			ORDER BY d.LastSync DESC
		`, userID)

		if err != nil {
			log.Printf("Error querying device sync status: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device sync status"})
			return
		}
		defer rows.Close()

		devices := make([]deviceSync, 0)
		for rows.Next() {
			var device deviceSync
			var lastSync sql.NullTime

			if err := rows.Scan(
				&device.DeviceID,
				&device.DeviceName,
				&device.DeviceType,
				&lastSync,
				&device.IsActive,
				&device.SyncEnabled,
			); err != nil {
				log.Printf("Error scanning device sync row: %v", err)
				continue
			}

			// Set the last sync time if valid
			if lastSync.Valid {
				device.LastSync = lastSync.Time
			}

			// Only include active devices
			if device.IsActive {
				devices = append(devices, device)
			}
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating device sync rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process device sync status"})
			return
		}

		// Return the response
		c.JSON(http.StatusOK, gin.H{
			"devices":   devices,
			"timestamp": time.Now().Unix(),
		})
	}
}
