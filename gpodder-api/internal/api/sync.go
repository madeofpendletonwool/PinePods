package api

import (
	"log"
	"net/http"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"

	"github.com/gin-gonic/gin"
)

// getSyncStatus handles GET /api/2/sync-devices/{username}.json
func getSyncStatus(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")

		// Query for device sync pairs
		rows, err := database.Query(`
			SELECT d1.DeviceName, d2.DeviceName
			FROM "GpodderSyncDevicePairs" p
			JOIN "GpodderDevices" d1 ON p.DeviceID1 = d1.DeviceID
			JOIN "GpodderDevices" d2 ON p.DeviceID2 = d2.DeviceID
			WHERE p.UserID = $1
		`, userID)

		if err != nil {
			log.Printf("Error querying device sync pairs: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get sync status"})
			return
		}

		// Build sync pairs
		syncPairs := make([][]string, 0)
		for rows.Next() {
			var device1, device2 string
			if err := rows.Scan(&device1, &device2); err != nil {
				log.Printf("Error scanning device pair: %v", err)
				continue
			}
			syncPairs = append(syncPairs, []string{device1, device2})
		}
		rows.Close()

		// Query for devices not in any sync pair
		rows, err = database.Query(`
			SELECT d.DeviceName
			FROM "GpodderDevices" d
			WHERE d.UserID = $1
			AND d.DeviceID NOT IN (
				SELECT DeviceID1 FROM "GpodderSyncDevicePairs" WHERE UserID = $1
				UNION
				SELECT DeviceID2 FROM "GpodderSyncDevicePairs" WHERE UserID = $1
			)
		`, userID)

		if err != nil {
			log.Printf("Error querying non-synced devices: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get sync status"})
			return
		}

		// Build non-synced devices list
		nonSynced := make([]string, 0)
		for rows.Next() {
			var deviceName string
			if err := rows.Scan(&deviceName); err != nil {
				log.Printf("Error scanning non-synced device: %v", err)
				continue
			}
			nonSynced = append(nonSynced, deviceName)
		}
		rows.Close()

		// Return response
		c.JSON(http.StatusOK, models.SyncDevicesResponse{
			Synchronized:    syncPairs,
			NotSynchronized: nonSynced,
		})
	}
}

// updateSyncStatus handles POST /api/2/sync-devices/{username}.json
func updateSyncStatus(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")

		// Parse request
		var req models.SyncDevicesRequest
		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body"})
			return
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

		// Process synchronize pairs
		for _, pair := range req.Synchronize {
			if len(pair) != 2 {
				continue
			}

			// Get device IDs
			var device1ID, device2ID int

			err := tx.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2
			`, userID, pair[0]).Scan(&device1ID)

			if err != nil {
				log.Printf("Error getting device ID for %s: %v", pair[0], err)
				continue
			}

			err = tx.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2
			`, userID, pair[1]).Scan(&device2ID)

			if err != nil {
				log.Printf("Error getting device ID for %s: %v", pair[1], err)
				continue
			}

			// Ensure device1ID < device2ID for consistency
			if device1ID > device2ID {
				device1ID, device2ID = device2ID, device1ID
			}

			// Insert sync pair if it doesn't exist
			_, err = tx.Exec(`
				INSERT INTO "GpodderSyncDevicePairs" (UserID, DeviceID1, DeviceID2)
				VALUES ($1, $2, $3)
				ON CONFLICT (UserID, DeviceID1, DeviceID2) DO NOTHING
			`, userID, device1ID, device2ID)

			if err != nil {
				log.Printf("Error creating sync pair: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create sync pair"})
				return
			}
		}

		// Process stop-synchronize devices
		for _, deviceName := range req.StopSynchronize {
			// Get device ID
			var deviceID int

			err := tx.QueryRow(`
				SELECT DeviceID FROM "GpodderDevices"
				WHERE UserID = $1 AND DeviceName = $2
			`, userID, deviceName).Scan(&deviceID)

			if err != nil {
				log.Printf("Error getting device ID for %s: %v", deviceName, err)
				continue
			}

			// Remove all sync pairs involving this device
			_, err = tx.Exec(`
				DELETE FROM "GpodderSyncDevicePairs"
				WHERE UserID = $1 AND (DeviceID1 = $2 OR DeviceID2 = $2)
			`, userID, deviceID)

			if err != nil {
				log.Printf("Error removing sync pairs: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to remove sync pairs"})
				return
			}
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return updated sync status by reusing the getSyncStatus handler
		getSyncStatus(database)(c)
	}
}
