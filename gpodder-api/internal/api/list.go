package api

import (
	"database/sql"
	"fmt"
	"log"
	"net/http"
	"regexp"
	"strings"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"

	"github.com/gin-gonic/gin"
)

// getUserLists handles GET /api/2/lists/{username}.json
func getUserLists(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware (if authenticated)
		userID, exists := c.Get("userID")
		username := c.Param("username")

		// If not authenticated, get user ID from username
		if !exists {
			var query string

			if database.IsPostgreSQLDB() {
				query = `SELECT UserID FROM "Users" WHERE Username = $1`
			} else {
				query = `SELECT UserID FROM Users WHERE Username = ?`
			}

			err := database.QueryRow(query, username).Scan(&userID)

			if err != nil {
				if err == sql.ErrNoRows {
					c.JSON(http.StatusNotFound, gin.H{"error": "User not found"})
				} else {
					log.Printf("Error getting user ID: %v", err)
					c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get user"})
				}
				return
			}
		}

		// Query for user's podcast lists
		var query string

		if database.IsPostgreSQLDB() {
			query = `
				SELECT ListID, Name, Title
				FROM "GpodderSyncPodcastLists"
				WHERE UserID = $1
			`
		} else {
			query = `
				SELECT ListID, Name, Title
				FROM GpodderSyncPodcastLists
				WHERE UserID = ?
			`
		}

		rows, err := database.Query(query, userID)

		if err != nil {
			log.Printf("Error querying podcast lists: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcast lists"})
			return
		}
		defer rows.Close()

		// Build response
		lists := make([]models.PodcastList, 0)
		for rows.Next() {
			var list models.PodcastList

			if err := rows.Scan(&list.ListID, &list.Name, &list.Title); err != nil {
				log.Printf("Error scanning podcast list: %v", err)
				continue
			}

			// Generate web URL
			list.WebURL = fmt.Sprintf("/user/%s/lists/%s", username, list.Name)

			lists = append(lists, list)
		}

		c.JSON(http.StatusOK, lists)
	}
}

// createPodcastList handles POST /api/2/lists/{username}/create
func createPodcastList(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")
		username := c.Param("username")

		// Get title from query parameter
		title := c.Query("title")
		if title == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Title is required"})
			return
		}

		// Get format from query parameter or default to json
		format := c.Query("format")
		if format == "" {
			format = "json"
		}

		// Parse body for podcast URLs
		var podcastURLs []string

		switch format {
		case "json":
			if err := c.ShouldBindJSON(&podcastURLs); err != nil {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body"})
				return
			}
		case "txt":
			body, err := c.GetRawData()
			if err != nil {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Failed to read request body"})
				return
			}

			// Split by newlines
			lines := strings.Split(string(body), "\n")
			for _, line := range lines {
				line = strings.TrimSpace(line)
				if line != "" {
					podcastURLs = append(podcastURLs, line)
				}
			}
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
			return
		}

		// Generate name from title
		name := generateNameFromTitle(title)

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

		// Check if a list with this name already exists
		var existingID int
		var existsQuery string

		if database.IsPostgreSQLDB() {
			existsQuery = `
				SELECT ListID FROM "GpodderSyncPodcastLists"
				WHERE UserID = $1 AND Name = $2
			`
		} else {
			existsQuery = `
				SELECT ListID FROM GpodderSyncPodcastLists
				WHERE UserID = ? AND Name = ?
			`
		}

		err = tx.QueryRow(existsQuery, userID, name).Scan(&existingID)

		if err == nil {
			// List already exists
			c.JSON(http.StatusConflict, gin.H{"error": "A podcast list with this name already exists"})
			return
		} else if err != sql.ErrNoRows {
			log.Printf("Error checking list existence: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to check list existence"})
			return
		}

		// Create new list
		var listID int

		if database.IsPostgreSQLDB() {
			err = tx.QueryRow(`
				INSERT INTO "GpodderSyncPodcastLists" (UserID, Name, Title)
				VALUES ($1, $2, $3)
				RETURNING ListID
			`, userID, name, title).Scan(&listID)
		} else {
			var result sql.Result
			result, err = tx.Exec(`
				INSERT INTO GpodderSyncPodcastLists (UserID, Name, Title)
				VALUES (?, ?, ?)
			`, userID, name, title)

			if err != nil {
				log.Printf("Error creating podcast list: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create podcast list"})
				return
			}

			lastID, err := result.LastInsertId()
			if err != nil {
				log.Printf("Error getting last insert ID: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create podcast list"})
				return
			}

			listID = int(lastID)
		}

		if err != nil {
			log.Printf("Error creating podcast list: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create podcast list"})
			return
		}

		// Add podcasts to list
		for _, url := range podcastURLs {
			var insertQuery string

			if database.IsPostgreSQLDB() {
				insertQuery = `
					INSERT INTO "GpodderSyncPodcastListEntries" (ListID, PodcastURL)
					VALUES ($1, $2)
				`
			} else {
				insertQuery = `
					INSERT INTO GpodderSyncPodcastListEntries (ListID, PodcastURL)
					VALUES (?, ?)
				`
			}

			_, err = tx.Exec(insertQuery, listID, url)

			if err != nil {
				log.Printf("Error adding podcast to list: %v", err)
				// Continue with other podcasts
			}
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return success with redirect location
		c.Header("Location", fmt.Sprintf("/api/2/lists/%s/list/%s?format=%s", username, name, format))
		c.Status(http.StatusSeeOther)
	}
}

// getPodcastList handles GET /api/2/lists/{username}/list/{listname}
func getPodcastList(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get username and listname from URL
		username := c.Param("username")
		listName := c.Param("listname")

		// Get format from query parameter or default to json
		format := c.Query("format")
		if format == "" {
			format = "json"
		}

		// Get user ID from username
		var userID int
		var query string

		if database.IsPostgreSQLDB() {
			query = `SELECT UserID FROM "Users" WHERE Username = $1`
		} else {
			query = `SELECT UserID FROM Users WHERE Username = ?`
		}

		err := database.QueryRow(query, username).Scan(&userID)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "User not found"})
			} else {
				log.Printf("Error getting user ID: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get user"})
			}
			return
		}

		// Get list info
		var listID int
		var title string

		if database.IsPostgreSQLDB() {
			query = `
				SELECT ListID, Title FROM "GpodderSyncPodcastLists"
				WHERE UserID = $1 AND Name = $2
			`
		} else {
			query = `
				SELECT ListID, Title FROM GpodderSyncPodcastLists
				WHERE UserID = ? AND Name = ?
			`
		}

		err = database.QueryRow(query, userID, listName).Scan(&listID, &title)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "Podcast list not found"})
			} else {
				log.Printf("Error getting podcast list: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcast list"})
			}
			return
		}

		// Get podcasts in list
		var rows *sql.Rows

		if database.IsPostgreSQLDB() {
			query = `
				SELECT e.PodcastURL, p.PodcastName, p.Description, p.Author, p.ArtworkURL, p.WebsiteURL
				FROM "GpodderSyncPodcastListEntries" e
				LEFT JOIN "Podcasts" p ON e.PodcastURL = p.FeedURL
				WHERE e.ListID = $1
			`
			rows, err = database.Query(query, listID)
		} else {
			query = `
				SELECT e.PodcastURL, p.PodcastName, p.Description, p.Author, p.ArtworkURL, p.WebsiteURL
				FROM GpodderSyncPodcastListEntries e
				LEFT JOIN Podcasts p ON e.PodcastURL = p.FeedURL
				WHERE e.ListID = ?
			`
			rows, err = database.Query(query, listID)
		}

		if err != nil {
			log.Printf("Error querying podcasts in list: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcasts in list"})
			return
		}
		defer rows.Close()

		// Build podcast list
		podcasts := make([]models.Podcast, 0)
		for rows.Next() {
			var podcast models.Podcast
			var podcastName, description, author, artworkURL, websiteURL sql.NullString

			if err := rows.Scan(&podcast.URL, &podcastName, &description, &author, &artworkURL, &websiteURL); err != nil {
				log.Printf("Error scanning podcast: %v", err)
				continue
			}

			// Set values if present
			if podcastName.Valid {
				podcast.Title = podcastName.String
			} else {
				podcast.Title = podcast.URL
			}

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

			// Add MygpoLink
			podcast.MygpoLink = fmt.Sprintf("/podcast/%s", podcast.URL)

			podcasts = append(podcasts, podcast)
		}

		// Return in requested format
		switch format {
		case "json":
			c.JSON(http.StatusOK, podcasts)
		case "txt":
			// Plain text format - just URLs
			var sb strings.Builder
			for _, podcast := range podcasts {
				sb.WriteString(podcast.URL)
				sb.WriteString("\n")
			}
			c.String(http.StatusOK, sb.String())
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
		}
	}
}

// updatePodcastList handles PUT /api/2/lists/{username}/list/{listname}
// updatePodcastList handles PUT /api/2/lists/{username}/list/{listname}
func updatePodcastList(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")
		listName := c.Param("listname")

		// Get format from query parameter or default to json
		format := c.Query("format")
		if format == "" {
			format = "json"
		}

		// Parse body for podcast URLs
		var podcastURLs []string

		switch format {
		case "json":
			if err := c.ShouldBindJSON(&podcastURLs); err != nil {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid request body"})
				return
			}
		case "txt":
			body, err := c.GetRawData()
			if err != nil {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Failed to read request body"})
				return
			}

			// Split by newlines
			lines := strings.Split(string(body), "\n")
			for _, line := range lines {
				line = strings.TrimSpace(line)
				if line != "" {
					podcastURLs = append(podcastURLs, line)
				}
			}
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
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

		// Get list ID
		var listID int
		var query string

		if database.IsPostgreSQLDB() {
			query = `
				SELECT ListID FROM "GpodderSyncPodcastLists"
				WHERE UserID = $1 AND Name = $2
			`
		} else {
			query = `
				SELECT ListID FROM GpodderSyncPodcastLists
				WHERE UserID = ? AND Name = ?
			`
		}

		err = tx.QueryRow(query, userID, listName).Scan(&listID)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "Podcast list not found"})
			} else {
				log.Printf("Error getting podcast list: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcast list"})
			}
			return
		}

		// Remove existing entries
		if database.IsPostgreSQLDB() {
			query = `
				DELETE FROM "GpodderSyncPodcastListEntries"
				WHERE ListID = $1
			`
		} else {
			query = `
				DELETE FROM GpodderSyncPodcastListEntries
				WHERE ListID = ?
			`
		}

		_, err = tx.Exec(query, listID)

		if err != nil {
			log.Printf("Error removing existing entries: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to update podcast list"})
			return
		}

		// Add new entries
		for _, url := range podcastURLs {
			if database.IsPostgreSQLDB() {
				query = `
					INSERT INTO "GpodderSyncPodcastListEntries" (ListID, PodcastURL)
					VALUES ($1, $2)
				`
			} else {
				query = `
					INSERT INTO GpodderSyncPodcastListEntries (ListID, PodcastURL)
					VALUES (?, ?)
				`
			}

			_, err = tx.Exec(query, listID, url)

			if err != nil {
				log.Printf("Error adding podcast to list: %v", err)
				// Continue with other podcasts
			}
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return success
		c.Status(http.StatusNoContent)
	}
}

// deletePodcastList handles DELETE /api/2/lists/{username}/list/{listname}
// deletePodcastList handles DELETE /api/2/lists/{username}/list/{listname}
func deletePodcastList(database *db.Database) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, _ := c.Get("userID")
		listName := c.Param("listname")

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

		// Get list ID
		var listID int
		var query string

		if database.IsPostgreSQLDB() {
			query = `
				SELECT ListID FROM "GpodderSyncPodcastLists"
				WHERE UserID = $1 AND Name = $2
			`
		} else {
			query = `
				SELECT ListID FROM GpodderSyncPodcastLists
				WHERE UserID = ? AND Name = ?
			`
		}

		err = tx.QueryRow(query, userID, listName).Scan(&listID)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "Podcast list not found"})
			} else {
				log.Printf("Error getting podcast list: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcast list"})
			}
			return
		}

		// Delete list entries first (cascade should handle this, but being explicit)
		if database.IsPostgreSQLDB() {
			query = `
				DELETE FROM "GpodderSyncPodcastListEntries"
				WHERE ListID = $1
			`
		} else {
			query = `
				DELETE FROM GpodderSyncPodcastListEntries
				WHERE ListID = ?
			`
		}

		_, err = tx.Exec(query, listID)

		if err != nil {
			log.Printf("Error deleting list entries: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to delete podcast list"})
			return
		}

		// Delete list
		if database.IsPostgreSQLDB() {
			query = `
				DELETE FROM "GpodderSyncPodcastLists"
				WHERE ListID = $1
			`
		} else {
			query = `
				DELETE FROM GpodderSyncPodcastLists
				WHERE ListID = ?
			`
		}

		_, err = tx.Exec(query, listID)

		if err != nil {
			log.Printf("Error deleting podcast list: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to delete podcast list"})
			return
		}

		// Commit transaction
		if err = tx.Commit(); err != nil {
			log.Printf("Error committing transaction: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to commit changes"})
			return
		}

		// Return success
		c.Status(http.StatusNoContent)
	}
}

// Helper function to generate a URL-friendly name from a title
func generateNameFromTitle(title string) string {
	// Convert to lowercase
	name := strings.ToLower(title)

	// Replace spaces with hyphens
	name = strings.ReplaceAll(name, " ", "-")

	// Remove special characters
	re := regexp.MustCompile(`[^a-z0-9-]`)
	name = re.ReplaceAllString(name, "")

	// Ensure name is not empty
	if name == "" {
		name = "list"
	}

	return name
}
