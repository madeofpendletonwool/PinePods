package api

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"
	"unicode/utf8"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"

	"github.com/gin-gonic/gin"
)

// Constants for settings
const (
	MAX_SETTING_KEY_LENGTH   = 255
	MAX_SETTING_VALUE_LENGTH = 8192
	MAX_SETTINGS_PER_REQUEST = 50
)

// Known settings that trigger behavior
var knownSettings = map[string]map[string]bool{
	"account": {
		"public_profile":        true,
		"store_user_agent":      true,
		"public_subscriptions":  true,
		"color_theme":           true,
		"default_subscribe_all": true,
	},
	"episode": {
		"is_favorite":      true,
		"played":           true,
		"current_position": true,
	},
	"podcast": {
		"public_subscription": true,
		"auto_download":       true,
		"episode_sort":        true,
	},
	"device": {
		"auto_update":           true,
		"update_interval":       true,
		"wifi_only_downloads":   true,
		"max_episodes_per_feed": true,
	},
}

// Validation interfaces and functions

// ValueValidator defines interface for validating settings values
type ValueValidator interface {
	Validate(value interface{}) bool
}

// BooleanValidator validates boolean values
type BooleanValidator struct{}

func (v BooleanValidator) Validate(value interface{}) bool {
	_, ok := value.(bool)
	return ok
}

// IntValidator validates integer values
type IntValidator struct {
	Min int
	Max int
}

func (v IntValidator) Validate(value interface{}) bool {
	num, ok := value.(float64) // JSON numbers are parsed as float64
	if !ok {
		return false
	}

	// Check if it's a whole number
	if num != float64(int(num)) {
		return false
	}

	// Check range if specified
	intVal := int(num)
	if v.Min != 0 || v.Max != 0 {
		if intVal < v.Min || (v.Max != 0 && intVal > v.Max) {
			return false
		}
	}

	return true
}

// StringValidator validates string values
type StringValidator struct {
	AllowedValues []string
	MaxLength     int
}

func (v StringValidator) Validate(value interface{}) bool {
	str, ok := value.(string)
	if !ok {
		return false
	}

	// Check maximum length if specified
	if v.MaxLength > 0 && utf8.RuneCountInString(str) > v.MaxLength {
		return false
	}

	// Check allowed values if specified
	if len(v.AllowedValues) > 0 {
		for _, allowed := range v.AllowedValues {
			if str == allowed {
				return true
			}
		}
		return false
	}

	return true
}

// validation rules for specific settings
var settingValidators = map[string]map[string]ValueValidator{
	"account": {
		"public_profile":        BooleanValidator{},
		"store_user_agent":      BooleanValidator{},
		"public_subscriptions":  BooleanValidator{},
		"default_subscribe_all": BooleanValidator{},
		"color_theme":           StringValidator{AllowedValues: []string{"light", "dark", "system"}, MaxLength: 10},
	},
	"episode": {
		"is_favorite":      BooleanValidator{},
		"played":           BooleanValidator{},
		"current_position": IntValidator{Min: 0},
	},
	"podcast": {
		"public_subscription": BooleanValidator{},
		"auto_download":       BooleanValidator{},
		"episode_sort":        StringValidator{AllowedValues: []string{"newest_first", "oldest_first", "title"}, MaxLength: 20},
	},
	"device": {
		"auto_update":           BooleanValidator{},
		"update_interval":       IntValidator{Min: 10, Max: 1440}, // 10 minutes to 24 hours
		"wifi_only_downloads":   BooleanValidator{},
		"max_episodes_per_feed": IntValidator{Min: 1, Max: 1000},
	},
}

// validateSettingValue validates a setting value based on its scope and key
func validateSettingValue(scope, key string, value interface{}) (bool, string) {
	// Maximum setting value length check
	jsonValue, err := json.Marshal(value)
	if err != nil {
		return false, "Failed to serialize setting value"
	}

	if len(jsonValue) > MAX_SETTING_VALUE_LENGTH {
		return false, "Setting value exceeds maximum length"
	}

	// Check if we have a specific validator for this setting
	if validators, ok := settingValidators[scope]; ok {
		if validator, ok := validators[key]; ok {
			if !validator.Validate(value) {
				return false, "Setting value failed validation for the specified scope and key"
			}
		}
	}

	return true, ""
}

// getSettings handles GET /api/2/settings/{username}/{scope}.json
func getSettings(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get scope from URL
		scope := c.Param("scope")
		if scope == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Scope is required"})
			return
		}

		// Validate scope
		if scope != "account" && scope != "device" && scope != "podcast" && scope != "episode" {
			c.JSON(http.StatusBadRequest, gin.H{
				"error": "Invalid scope. Valid values are: account, device, podcast, episode",
			})
			return
		}

		// Get optional query parameters
		deviceID := c.Query("device")
		podcastURL := c.Query("podcast")
		episodeURL := c.Query("episode")

		// Validate parameters based on scope
		if scope == "device" && deviceID == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required for device scope"})
			return
		}
		if scope == "podcast" && podcastURL == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Podcast URL is required for podcast scope"})
			return
		}
		if scope == "episode" && (podcastURL == "" || episodeURL == "") {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Podcast URL and Episode URL are required for episode scope"})
			return
		}

		// Build query based on scope
		var query string
		var args []interface{}

		switch scope {
		case "account":
			query = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2
				AND DeviceID IS NULL AND PodcastURL IS NULL AND EpisodeURL IS NULL
			`
			args = append(args, userID, scope)
		case "device":
			// Get device ID from name
			var deviceIDInt int
			err := database.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
			`, userID, deviceID).Scan(&deviceIDInt)

			if err != nil {
				if err == sql.ErrNoRows {
					log.Printf("Device not found: %s", deviceID)
					c.JSON(http.StatusNotFound, gin.H{"error": "Device not found or not active"})
				} else {
					log.Printf("Error getting device ID: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
				}
				return
			}

			query = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2 AND DeviceID = $3
				AND PodcastURL IS NULL AND EpisodeURL IS NULL
			`
			args = append(args, userID, scope, deviceIDInt)
		case "podcast":
			// Validate podcast URL
			if !isValidURL(podcastURL) {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid podcast URL"})
				return
			}

			query = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2 AND PodcastURL = $3
				AND DeviceID IS NULL AND EpisodeURL IS NULL
			`
			args = append(args, userID, scope, podcastURL)
		case "episode":
			// Validate URLs
			if !isValidURL(podcastURL) || !isValidURL(episodeURL) {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid podcast or episode URL"})
				return
			}

			query = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2 AND PodcastURL = $3 AND EpisodeURL = $4
				AND DeviceID IS NULL
			`
			args = append(args, userID, scope, podcastURL, episodeURL)
		}

		// Query settings
		rows, err := database.Query(query, args...)
		if err != nil {
			log.Printf("Error querying settings: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get settings"})
			return
		}
		defer rows.Close()

		// Build settings map
		settings := make(map[string]interface{})
		for rows.Next() {
			var key, value string
			if err := rows.Scan(&key, &value); err != nil {
				log.Printf("Error scanning setting row: %v", err)
				continue
			}

			// Try to unmarshal as JSON, fallback to string if not valid JSON
			var jsonValue interface{}
			if err := json.Unmarshal([]byte(value), &jsonValue); err != nil {
				// Not valid JSON, use as string
				settings[key] = value
			} else {
				// Valid JSON, use parsed value
				settings[key] = jsonValue
			}
		}

		if err := rows.Err(); err != nil {
			log.Printf("Error iterating setting rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get settings"})
			return
		}

		// Return settings
		c.JSON(http.StatusOK, settings)
	}
}

// isValidURL performs basic URL validation
func isValidURL(urlStr string) bool {
	// Check if empty
	if urlStr == "" {
		return false
	}

	// Must start with http:// or https://
	if !strings.HasPrefix(strings.ToLower(urlStr), "http://") &&
		!strings.HasPrefix(strings.ToLower(urlStr), "https://") {
		return false
	}

	// Basic length check
	if len(urlStr) < 10 || len(urlStr) > 2048 {
		return false
	}

	return true
}

// saveSettings handles POST /api/2/settings/{username}/{scope}.json
func saveSettings(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Get scope from URL
		scope := c.Param("scope")
		if scope == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Scope is required"})
			return
		}

		// Validate scope
		if scope != "account" && scope != "device" && scope != "podcast" && scope != "episode" {
			c.JSON(http.StatusBadRequest, gin.H{
				"error": "Invalid scope. Valid values are: account, device, podcast, episode",
			})
			return
		}

		// Get optional query parameters
		deviceName := c.Query("device")
		podcastURL := c.Query("podcast")
		episodeURL := c.Query("episode")

		// Validate parameters based on scope
		if scope == "device" && deviceName == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Device ID is required for device scope"})
			return
		}
		if scope == "podcast" && podcastURL == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Podcast URL is required for podcast scope"})
			return
		}
		if scope == "episode" && (podcastURL == "" || episodeURL == "") {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Podcast URL and Episode URL are required for episode scope"})
			return
		}

		// Validate URLs
		if scope == "podcast" && !isValidURL(podcastURL) {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid podcast URL"})
			return
		}
		if scope == "episode" && (!isValidURL(podcastURL) || !isValidURL(episodeURL)) {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid podcast or episode URL"})
			return
		}

		// Parse request body
		var req models.SettingsRequest
		if err := c.ShouldBindJSON(&req); err != nil {
			log.Printf("Error parsing request: %v", err)
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body: expected a JSON object with 'set' and 'remove' properties"})
			return
		}

		// Validate request size
		if len(req.Set) > MAX_SETTINGS_PER_REQUEST || len(req.Remove) > MAX_SETTINGS_PER_REQUEST {
			c.JSON(http.StatusBadRequest, gin.H{
				"error": fmt.Sprintf("Too many settings in request. Maximum allowed: %d", MAX_SETTINGS_PER_REQUEST),
			})
			return
		}

		// Process device ID if needed
		var deviceID *int
		if scope == "device" {
			var deviceIDInt int
			err := database.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2 AND IsActive = true
			`, userID, deviceName).Scan(&deviceIDInt)

			if err != nil {
				if err == sql.ErrNoRows {
					// Create the device if it doesn't exist
					err = database.QueryRow(`
						INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, IsActive, LastSync)
						VALUES ($1, $2, 'other', true, $3)
						RETURNING DeviceID
					`, userID, deviceName, time.Now()).Scan(&deviceIDInt)

					if err != nil {
						log.Printf("Error creating device: %v", err)
						c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create device"})
						return
					}
				} else {
					log.Printf("Error getting device ID: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get device"})
					return
				}
			}

			deviceID = &deviceIDInt
		}

		// Begin transaction
		tx, err := database.Begin()
		if err != nil {
			log.Printf("Error beginning transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to save settings"})
			return
		}
		defer func() {
			if err != nil {
				tx.Rollback()
				return
			}
		}()

		// Process settings to set
		for key, value := range req.Set {
			// Validate key
			if len(key) == 0 || len(key) > MAX_SETTING_KEY_LENGTH {
				log.Printf("Invalid setting key length: %s", key)
				c.JSON(http.StatusBadRequest, gin.H{
					"error": fmt.Sprintf("Invalid setting key: must be between 1 and %d characters", MAX_SETTING_KEY_LENGTH),
				})
				return
			}

			// Allow only letters, numbers, underscores and hyphens
			if !isValidSettingKey(key) {
				log.Printf("Invalid setting key: %s", key)
				c.JSON(http.StatusBadRequest, gin.H{
					"error": "Invalid setting key: must contain only letters, numbers, underscores and hyphens",
				})
				return
			}

			// Validate value
			valid, errMsg := validateSettingValue(scope, key, value)
			if !valid {
				log.Printf("Invalid setting value for key %s: %s", key, errMsg)
				c.JSON(http.StatusBadRequest, gin.H{
					"error": fmt.Sprintf("Invalid value for key '%s': %s", key, errMsg),
				})
				return
			}

			// Convert value to JSON string
			jsonValue, err := json.Marshal(value)
			if err != nil {
				log.Printf("Error marshaling value to JSON: %v", err)
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid value for key: " + key})
				return
			}

			// Build query based on scope
			var query string
			var args []interface{}

			switch scope {
			case "account":
				query = `
					INSERT INTO "GpodderSyncSettings" (UserID, Scope, SettingKey, SettingValue, LastUpdated)
					VALUES ($1, $2, $3, $4, $5)
					ON CONFLICT (UserID, Scope, SettingKey)
					WHERE DeviceID IS NULL AND PodcastURL IS NULL AND EpisodeURL IS NULL
					DO UPDATE SET SettingValue = $4, LastUpdated = $5
				`
				args = append(args, userID, scope, key, string(jsonValue), time.Now())
			case "device":
				query = `
					INSERT INTO "GpodderSyncSettings" (UserID, Scope, DeviceID, SettingKey, SettingValue, LastUpdated)
					VALUES ($1, $2, $3, $4, $5, $6)
					ON CONFLICT (UserID, Scope, SettingKey, DeviceID)
					WHERE PodcastURL IS NULL AND EpisodeURL IS NULL
					DO UPDATE SET SettingValue = $5, LastUpdated = $6
				`
				args = append(args, userID, scope, deviceID, key, string(jsonValue), time.Now())
			case "podcast":
				query = `
					INSERT INTO "GpodderSyncSettings" (UserID, Scope, PodcastURL, SettingKey, SettingValue, LastUpdated)
					VALUES ($1, $2, $3, $4, $5, $6)
					ON CONFLICT (UserID, Scope, SettingKey, PodcastURL)
					WHERE DeviceID IS NULL AND EpisodeURL IS NULL
					DO UPDATE SET SettingValue = $5, LastUpdated = $6
				`
				args = append(args, userID, scope, podcastURL, key, string(jsonValue), time.Now())
			case "episode":
				query = `
					INSERT INTO "GpodderSyncSettings" (UserID, Scope, PodcastURL, EpisodeURL, SettingKey, SettingValue, LastUpdated)
					VALUES ($1, $2, $3, $4, $5, $6, $7)
					ON CONFLICT (UserID, Scope, SettingKey, PodcastURL, EpisodeURL)
					WHERE DeviceID IS NULL
					DO UPDATE SET SettingValue = $6, LastUpdated = $7
				`
				args = append(args, userID, scope, podcastURL, episodeURL, key, string(jsonValue), time.Now())
			}

			// Execute query
			_, err = tx.Exec(query, args...)
			if err != nil {
				log.Printf("Error setting value for key %s: %v", key, err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to save settings"})
				return
			}
		}

		// Process settings to remove
		for _, key := range req.Remove {
			// Validate key
			if len(key) == 0 || len(key) > MAX_SETTING_KEY_LENGTH {
				log.Printf("Invalid setting key length: %s", key)
				c.JSON(http.StatusBadRequest, gin.H{
					"error": fmt.Sprintf("Invalid setting key: must be between 1 and %d characters", MAX_SETTING_KEY_LENGTH),
				})
				return
			}

			// Allow only letters, numbers, underscores and hyphens
			if !isValidSettingKey(key) {
				log.Printf("Invalid setting key: %s", key)
				c.JSON(http.StatusBadRequest, gin.H{
					"error": "Invalid setting key: must contain only letters, numbers, underscores and hyphens",
				})
				return
			}

			// Build query based on scope
			var query string
			var args []interface{}

			switch scope {
			case "account":
				query = `
					DELETE FROM "GpodderSyncSettings"
					WHERE UserID = $1 AND Scope = $2 AND SettingKey = $3
					AND DeviceID IS NULL AND PodcastURL IS NULL AND EpisodeURL IS NULL
				`
				args = append(args, userID, scope, key)
			case "device":
				query = `
					DELETE FROM "GpodderSyncSettings"
					WHERE UserID = $1 AND Scope = $2 AND DeviceID = $3 AND SettingKey = $4
					AND PodcastURL IS NULL AND EpisodeURL IS NULL
				`
				args = append(args, userID, scope, deviceID, key)
			case "podcast":
				query = `
					DELETE FROM "GpodderSyncSettings"
					WHERE UserID = $1 AND Scope = $2 AND PodcastURL = $3 AND SettingKey = $4
					AND DeviceID IS NULL AND EpisodeURL IS NULL
				`
				args = append(args, userID, scope, podcastURL, key)
			case "episode":
				query = `
					DELETE FROM "GpodderSyncSettings"
					WHERE UserID = $1 AND Scope = $2 AND PodcastURL = $3 AND EpisodeURL = $4 AND SettingKey = $5
					AND DeviceID IS NULL
				`
				args = append(args, userID, scope, podcastURL, episodeURL, key)
			}

			// Execute query
			_, err = tx.Exec(query, args...)
			if err != nil {
				log.Printf("Error removing key %s: %v", key, err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to save settings"})
				return
			}
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to save settings"})
			return
		}

		// Query all settings for the updated response
		var queryAll string
		var argsAll []interface{}

		switch scope {
		case "account":
			queryAll = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2
				AND DeviceID IS NULL AND PodcastURL IS NULL AND EpisodeURL IS NULL
			`
			argsAll = append(argsAll, userID, scope)
		case "device":
			queryAll = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2 AND DeviceID = $3
				AND PodcastURL IS NULL AND EpisodeURL IS NULL
			`
			argsAll = append(argsAll, userID, scope, deviceID)
		case "podcast":
			queryAll = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2 AND PodcastURL = $3
				AND DeviceID IS NULL AND EpisodeURL IS NULL
			`
			argsAll = append(argsAll, userID, scope, podcastURL)
		case "episode":
			queryAll = `
				SELECT SettingKey, SettingValue
				FROM "GpodderSyncSettings"
				WHERE UserID = $1 AND Scope = $2 AND PodcastURL = $3 AND EpisodeURL = $4
				AND DeviceID IS NULL
			`
			argsAll = append(argsAll, userID, scope, podcastURL, episodeURL)
		}

		// Query all settings
		rows, err := database.Query(queryAll, argsAll...)
		if err != nil {
			log.Printf("Error querying all settings: %v", err)
			c.JSON(http.StatusOK, gin.H{}) // Return empty object in case of error
			return
		}
		defer rows.Close()

		// Build settings map
		settings := make(map[string]interface{})
		for rows.Next() {
			var key, value string
			if err := rows.Scan(&key, &value); err != nil {
				log.Printf("Error scanning setting row: %v", err)
				continue
			}

			// Try to unmarshal as JSON, fallback to string if not valid JSON
			var jsonValue interface{}
			if err := json.Unmarshal([]byte(value), &jsonValue); err != nil {
				// Not valid JSON, use as string
				settings[key] = value
			} else {
				// Valid JSON, use parsed value
				settings[key] = jsonValue
			}
		}

		if err := rows.Err(); err != nil {
			log.Printf("Error iterating setting rows: %v", err)
			c.JSON(http.StatusOK, gin.H{}) // Return empty object in case of error
			return
		}

		// Return updated settings
		c.JSON(http.StatusOK, settings)
	}
}

// isValidSettingKey checks if the key contains only valid characters
func isValidSettingKey(key string) bool {
	for _, r := range key {
		if (r < 'a' || r > 'z') && (r < 'A' || r > 'Z') && (r < '0' || r > '9') && r != '_' && r != '-' {
			return false
		}
	}
	return true
}

// toggleGpodderAPI is a Pinepods-specific extension to enable/disable the gpodder API for a user
func toggleGpodderAPI(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from context (set by AuthMiddleware)
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Parse request body
		var req struct {
			Enable bool `json:"enable"`
		}
		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body"})
			return
		}

		// Set Pod_Sync_Type based on enable flag
		var podSyncType string
		if req.Enable {
			// Check if external gpodder sync is already enabled
			var currentSyncType string
			err := database.QueryRow(`
				SELECT Pod_Sync_Type FROM "Users" WHERE UserID = $1
			`, userID).Scan(&currentSyncType)

			if err != nil {
				log.Printf("Error getting current sync type: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to toggle gpodder API"})
				return
			}

			if currentSyncType == "external" {
				podSyncType = "both"
			} else {
				podSyncType = "gpodder"
			}
		} else {
			// Check if external gpodder sync is enabled
			var currentSyncType string
			err := database.QueryRow(`
				SELECT Pod_Sync_Type FROM "Users" WHERE UserID = $1
			`, userID).Scan(&currentSyncType)

			if err != nil {
				log.Printf("Error getting current sync type: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to toggle gpodder API"})
				return
			}

			if currentSyncType == "both" {
				podSyncType = "external"
			} else {
				podSyncType = "None"
			}
		}

		// Update user's Pod_Sync_Type
		_, err := database.Exec(`
			UPDATE "Users" SET Pod_Sync_Type = $1 WHERE UserID = $2
		`, podSyncType, userID)

		if err != nil {
			log.Printf("Error updating Pod_Sync_Type: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to toggle gpodder API"})
			return
		}

		// Return success response
		c.JSON(http.StatusOK, gin.H{
			"enabled":   req.Enable,
			"sync_type": podSyncType,
		})
	}
}
