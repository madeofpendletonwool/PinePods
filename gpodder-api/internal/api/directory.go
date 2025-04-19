package api

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"regexp"
	"strconv"
	"strings"

	"pinepods/gpodder-api/internal/db"
	"pinepods/gpodder-api/internal/models"

	"github.com/gin-gonic/gin"
)

// Maximum number of items to return in listings
const MAX_DIRECTORY_ITEMS = 100

// Common tag categories for podcasts
var commonCategories = []models.Tag{
	{Title: "Technology", Tag: "technology", Usage: 530},
	{Title: "Society & Culture", Tag: "society-culture", Usage: 420},
	{Title: "Arts", Tag: "arts", Usage: 400},
	{Title: "News & Politics", Tag: "news-politics", Usage: 320},
	{Title: "Business", Tag: "business", Usage: 300},
	{Title: "Education", Tag: "education", Usage: 280},
	{Title: "Science", Tag: "science", Usage: 260},
	{Title: "Comedy", Tag: "comedy", Usage: 240},
	{Title: "Health", Tag: "health", Usage: 220},
	{Title: "Sports", Tag: "sports", Usage: 200},
	{Title: "History", Tag: "history", Usage: 180},
	{Title: "Religion & Spirituality", Tag: "religion-spirituality", Usage: 160},
	{Title: "TV & Film", Tag: "tv-film", Usage: 140},
	{Title: "Music", Tag: "music", Usage: 120},
	{Title: "Games & Hobbies", Tag: "games-hobbies", Usage: 100},
}

// getTopTags handles GET /api/2/tags/{count}.json
func getTopTags(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Parse count parameter
		countStr := c.Param("count")
		count, err := strconv.Atoi(countStr)
		if err != nil || count < 1 || count > MAX_DIRECTORY_ITEMS {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("Invalid count parameter: must be between 1 and %d", MAX_DIRECTORY_ITEMS)})
			return
		}

		// Try to query categories from database first
		rows, err := database.Query(`
			WITH category_counts AS (
				SELECT
					unnest(string_to_array(Categories, ',')) as category,
					COUNT(*) as usage
				FROM "Podcasts"
				WHERE Categories IS NOT NULL AND Categories != ''
				GROUP BY category
			)
			SELECT
				category as tag,
				category as title,
				usage
			FROM category_counts
			ORDER BY usage DESC
			LIMIT $1
		`, count)

		// If query fails or returns no rows, use the default list
		if err != nil || rows == nil {
			log.Printf("Error querying categories, using default list: %v", err)
			result := commonCategories
			if len(result) > count {
				result = result[:count]
			}
			c.JSON(http.StatusOK, result)
			return
		}
		defer rows.Close()

		// Process database results
		tags := make([]models.Tag, 0, count)
		for rows.Next() {
			var tag models.Tag
			if err := rows.Scan(&tag.Tag, &tag.Title, &tag.Usage); err != nil {
				log.Printf("Error scanning tag row: %v", err)
				continue
			}

			// Clean the tag
			tag.Tag = strings.ToLower(strings.TrimSpace(tag.Tag))
			tag.Tag = strings.ReplaceAll(tag.Tag, " ", "-")

			// Format the title properly
			tag.Title = formatTagTitle(tag.Tag)

			tags = append(tags, tag)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating tag rows: %v", err)
		}

		// If we got no results from the database, use the default list
		if len(tags) == 0 {
			result := commonCategories
			if len(result) > count {
				result = result[:count]
			}
			c.JSON(http.StatusOK, result)
			return
		}

		c.JSON(http.StatusOK, tags)
	}
}

// formatTagTitle formats a tag string into a proper title
func formatTagTitle(tag string) string {
	// Replace hyphens with spaces
	title := strings.ReplaceAll(tag, "-", " ")

	// Convert to title case (capitalize first letter of each word)
	words := strings.Fields(title)
	for i, word := range words {
		if len(word) > 0 {
			words[i] = strings.ToUpper(word[:1]) + word[1:]
		}
	}

	return strings.Join(words, " ")
}

// getPodcastsForTag handles GET /api/2/tag/{tag}/{count}.json
func getPodcastsForTag(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Parse parameters
		tag := c.Param("tag")
		countStr := c.Param("count")
		count, err := strconv.Atoi(countStr)
		if err != nil || count < 1 || count > MAX_DIRECTORY_ITEMS {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("Invalid count parameter: must be between 1 and %d", MAX_DIRECTORY_ITEMS)})
			return
		}

		// Format tag for searching
		searchTag := "%" + strings.ReplaceAll(tag, "-", " ") + "%"

		// Query podcasts with the given tag
		rows, err := database.Query(`
			SELECT DISTINCT ON (p.PodcastID)
				p.PodcastID, p.PodcastName, p.Author, p.Description,
				p.FeedURL, p.WebsiteURL, p.ArtworkURL,
				COUNT(DISTINCT u.UserID) OVER (PARTITION BY p.PodcastID) as subscribers
			FROM "Podcasts" p
			JOIN "Users" u ON p.UserID = u.UserID
			WHERE
				p.Categories ILIKE $1 OR
				p.PodcastName ILIKE $1 OR
				p.Description ILIKE $1
			ORDER BY p.PodcastID, subscribers DESC
			LIMIT $2
		`, searchTag, count)

		if err != nil {
			log.Printf("Error querying podcasts by tag: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcasts for tag"})
			return
		}
		defer rows.Close()

		// Build podcast list
		podcasts := make([]models.Podcast, 0)
		for rows.Next() {
			var podcast models.Podcast
			var podcastID int
			var author, description, websiteURL, artworkURL sql.NullString
			var subscribers int

			if err := rows.Scan(
				&podcastID,
				&podcast.Title,
				&author,
				&description,
				&podcast.URL,
				&websiteURL,
				&artworkURL,
				&subscribers,
			); err != nil {
				log.Printf("Error scanning podcast: %v", err)
				continue
			}

			// Set optional fields if present
			if author.Valid {
				podcast.Author = author.String
			}

			if description.Valid {
				podcast.Description = description.String
			}

			if websiteURL.Valid {
				podcast.Website = websiteURL.String
			}

			if artworkURL.Valid {
				podcast.LogoURL = artworkURL.String
			}

			podcast.Subscribers = subscribers
			podcast.MygpoLink = fmt.Sprintf("/podcast/%d", podcastID)

			podcasts = append(podcasts, podcast)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process podcasts"})
			return
		}

		c.JSON(http.StatusOK, podcasts)
	}
}

// getPodcastData handles GET /api/2/data/podcast.json
func getPodcastData(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get podcast URL from query parameter
		podcastURL := c.Query("url")
		if podcastURL == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "URL parameter is required"})
			return
		}

		// Query podcast data
		var podcast models.Podcast
		var podcastID int
		var author, description, websiteURL, artworkURL sql.NullString
		var subscribers int

		err := database.QueryRow(`
			SELECT
				p.PodcastID, p.PodcastName, p.Author, p.Description,
				p.FeedURL, p.WebsiteURL, p.ArtworkURL,
				COUNT(DISTINCT u.UserID) as subscribers
			FROM "Podcasts" p
			JOIN "Users" u ON p.UserID = u.UserID
			WHERE p.FeedURL = $1
			GROUP BY p.PodcastID
			LIMIT 1
		`, podcastURL).Scan(
			&podcastID,
			&podcast.Title,
			&author,
			&description,
			&podcast.URL,
			&websiteURL,
			&artworkURL,
			&subscribers,
		)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusNotFound, gin.H{"error": "Podcast not found"})
			} else {
				log.Printf("Error querying podcast data: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get podcast data"})
			}
			return
		}

		// Set optional fields if present
		if author.Valid {
			podcast.Author = author.String
		}

		if description.Valid {
			podcast.Description = description.String
		}

		if websiteURL.Valid {
			podcast.Website = websiteURL.String
		}

		if artworkURL.Valid {
			podcast.LogoURL = artworkURL.String
		}

		podcast.Subscribers = subscribers
		podcast.MygpoLink = fmt.Sprintf("/podcast/%d", podcastID)

		c.JSON(http.StatusOK, podcast)
	}
}

// isValidCallbackName checks if a JSONP callback name is valid and safe
func isValidCallbackName(callback string) bool {
	// Only allow alphanumeric characters, underscore, and period in callback names
	validCallbackRegex := regexp.MustCompile(`^[a-zA-Z0-9_.]+$`)
	return validCallbackRegex.MatchString(callback)
}

// podcastSearch handles GET /search.{format}
func podcastSearch(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get query parameter
		query := c.Query("q")
		if query == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Query parameter 'q' is required"})
			return
		}

		// Get format parameter
		format := c.Param("format")
		if format == "" {
			format = "json" // Default format
		}

		// Parse optional parameters
		scaleLogo := c.Query("scale_logo")
		var scaleSize int
		if scaleLogo != "" {
			size, err := strconv.Atoi(scaleLogo)
			if err != nil || size < 1 || size > 256 {
				scaleSize = 64 // Default size
			} else {
				scaleSize = size
			}
		}

		// Limit search terms to prevent performance issues
		if len(query) > 100 {
			query = query[:100]
		}

		// Prepare search query terms for SQL
		searchTerms := "%" + strings.ReplaceAll(query, " ", "%") + "%"

		// Search podcasts
		rows, err := database.Query(`
			SELECT DISTINCT ON (p.PodcastID)
				p.PodcastID, p.PodcastName, p.Author, p.Description,
				p.FeedURL, p.WebsiteURL, p.ArtworkURL,
				COUNT(DISTINCT u.UserID) OVER (PARTITION BY p.PodcastID) as subscribers,
				CASE
					WHEN p.PodcastName ILIKE $1 THEN 1
					WHEN p.Author ILIKE $1 THEN 2
					WHEN p.Description ILIKE $1 THEN 3
					ELSE 4
				END as match_priority
			FROM "Podcasts" p
			JOIN "Users" u ON p.UserID = u.UserID
			WHERE
				p.PodcastName ILIKE $1 OR
				p.Author ILIKE $1 OR
				p.Description ILIKE $1
			ORDER BY p.PodcastID, match_priority, subscribers DESC
			LIMIT $2
		`, searchTerms, MAX_DIRECTORY_ITEMS)

		if err != nil {
			log.Printf("Error searching podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to search podcasts"})
			return
		}
		defer rows.Close()

		// Build podcast list
		podcasts := make([]models.Podcast, 0)
		for rows.Next() {
			var podcast models.Podcast
			var podcastID int
			var author, description, websiteURL, artworkURL sql.NullString
			var subscribers, matchPriority int

			if err := rows.Scan(
				&podcastID,
				&podcast.Title,
				&author,
				&description,
				&podcast.URL,
				&websiteURL,
				&artworkURL,
				&subscribers,
				&matchPriority,
			); err != nil {
				log.Printf("Error scanning podcast: %v", err)
				continue
			}

			// Set optional fields if present
			if author.Valid {
				podcast.Author = author.String
			}

			if description.Valid {
				podcast.Description = description.String
			}

			if websiteURL.Valid {
				podcast.Website = websiteURL.String
			}

			if artworkURL.Valid {
				podcast.LogoURL = artworkURL.String

				// Add scaled logo URL if requested
				if scaleLogo != "" {
					podcast.ScaledLogoURL = fmt.Sprintf("/logo/%d/%s", scaleSize, artworkURL.String)
				}
			}

			podcast.Subscribers = subscribers
			podcast.MygpoLink = fmt.Sprintf("/podcast/%d", podcastID)

			podcasts = append(podcasts, podcast)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process search results"})
			return
		}

		// Return in requested format
		switch format {
		case "json":
			c.JSON(http.StatusOK, podcasts)
		case "jsonp":
			// JSONP callback
			callback := c.Query("jsonp")
			if callback == "" {
				callback = "callback" // Default callback name
			}

			// Validate callback name for security
			if !isValidCallbackName(callback) {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid JSONP callback name"})
				return
			}

			// Convert to JSON using the standard json package
			jsonData, err := json.Marshal(podcasts)
			if err != nil {
				log.Printf("Error marshaling to JSON: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to marshal to JSON"})
				return
			}

			// Wrap in callback
			c.Header("Content-Type", "application/javascript")
			c.String(http.StatusOK, "%s(%s);", callback, string(jsonData))
		case "txt":
			// Plain text format - just URLs
			var sb strings.Builder
			for _, podcast := range podcasts {
				sb.WriteString(podcast.URL)
				sb.WriteString("\n")
			}
			c.String(http.StatusOK, sb.String())
		case "opml":
			// OPML format
			opml := generateOpml(podcasts)
			c.Header("Content-Type", "text/xml")
			c.String(http.StatusOK, opml)
		case "xml":
			// XML format
			xml := generateXml(podcasts)
			c.Header("Content-Type", "text/xml")
			c.String(http.StatusOK, xml)
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
		}
	}
}

// getToplist handles GET /toplist/{number}.{format}
func getToplist(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Parse count parameter
		countStr := c.Param("number")
		count, err := strconv.Atoi(countStr)
		if err != nil || count < 1 || count > MAX_DIRECTORY_ITEMS {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("Invalid number parameter: must be between 1 and %d", MAX_DIRECTORY_ITEMS)})
			return
		}

		// Get format parameter
		format := c.Param("format")
		if format == "" {
			format = "json" // Default format
		}

		// Parse optional parameters
		scaleLogo := c.Query("scale_logo")
		var scaleSize int
		if scaleLogo != "" {
			size, err := strconv.Atoi(scaleLogo)
			if err != nil || size < 1 || size > 256 {
				scaleSize = 64 // Default size
			} else {
				scaleSize = size
			}
		}

		// Query top podcasts
		rows, err := database.Query(`
			WITH podcast_stats AS (
				SELECT
					p.PodcastID,
					p.PodcastName,
					p.Author,
					p.Description,
					p.FeedURL,
					p.WebsiteURL,
					p.ArtworkURL,
					COUNT(DISTINCT u.UserID) as subscribers,
					0 as position_last_week -- Placeholder for now
				FROM "Podcasts" p
				JOIN "Users" u ON p.UserID = u.UserID
				GROUP BY p.PodcastID
			)
			SELECT * FROM podcast_stats
			ORDER BY subscribers DESC, PodcastID
			LIMIT $1
		`, count)

		if err != nil {
			log.Printf("Error querying top podcasts: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get top podcasts"})
			return
		}
		defer rows.Close()

		// Build podcast list
		podcasts := make([]models.Podcast, 0)
		for rows.Next() {
			var podcast models.Podcast
			var podcastID int
			var author, description, websiteURL, artworkURL sql.NullString
			var subscribers, positionLastWeek int

			if err := rows.Scan(
				&podcastID,
				&podcast.Title,
				&author,
				&description,
				&podcast.URL,
				&websiteURL,
				&artworkURL,
				&subscribers,
				&positionLastWeek,
			); err != nil {
				log.Printf("Error scanning podcast: %v", err)
				continue
			}

			// Set optional fields if present
			if author.Valid {
				podcast.Author = author.String
			}

			if description.Valid {
				podcast.Description = description.String
			}

			if websiteURL.Valid {
				podcast.Website = websiteURL.String
			}

			if artworkURL.Valid {
				podcast.LogoURL = artworkURL.String

				// Add scaled logo URL if requested
				if scaleLogo != "" {
					podcast.ScaledLogoURL = fmt.Sprintf("/logo/%d/%s", scaleSize, artworkURL.String)
				}
			}

			podcast.Subscribers = subscribers
			podcast.MygpoLink = fmt.Sprintf("/podcast/%d", podcastID)

			podcasts = append(podcasts, podcast)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating podcast rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process podcasts"})
			return
		}

		// Return in requested format (same as search)
		switch format {
		case "json":
			c.JSON(http.StatusOK, podcasts)
		case "jsonp":
			// JSONP callback
			callback := c.Query("jsonp")
			if callback == "" {
				callback = "callback" // Default callback name
			}

			// Validate callback name for security
			if !isValidCallbackName(callback) {
				c.JSON(http.StatusBadRequest, gin.H{"error": "Invalid JSONP callback name"})
				return
			}

			// Convert to JSON using the standard json package
			jsonData, err := json.Marshal(podcasts)
			if err != nil {
				log.Printf("Error marshaling to JSON: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to marshal to JSON"})
				return
			}

			// Wrap in callback
			c.Header("Content-Type", "application/javascript")
			c.String(http.StatusOK, "%s(%s);", callback, string(jsonData))
		case "txt":
			// Plain text format - just URLs
			var sb strings.Builder
			for _, podcast := range podcasts {
				sb.WriteString(podcast.URL)
				sb.WriteString("\n")
			}
			c.String(http.StatusOK, sb.String())
		case "opml":
			// OPML format
			opml := generateOpml(podcasts)
			c.Header("Content-Type", "text/xml")
			c.String(http.StatusOK, opml)
		case "xml":
			// XML format
			xml := generateXml(podcasts)
			c.Header("Content-Type", "text/xml")
			c.String(http.StatusOK, xml)
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
		}
	}
}

// getSuggestions handles GET /suggestions/{count}.{format}
func getSuggestions(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get user ID from middleware
		userID, exists := c.Get("userID")
		if !exists {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Unauthorized"})
			return
		}

		// Parse count parameter
		countStr := c.Param("count")
		count, err := strconv.Atoi(countStr)
		if err != nil || count < 1 || count > MAX_DIRECTORY_ITEMS {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("Invalid count parameter: must be between 1 and %d", MAX_DIRECTORY_ITEMS)})
			return
		}

		// Get format parameter
		format := c.Param("format")
		if format == "" {
			format = "json" // Default format
		}

		// Get user's current subscriptions
		rows, err := database.Query(`
			SELECT FeedURL FROM "Podcasts" WHERE UserID = $1
		`, userID)

		if err != nil {
			log.Printf("Error getting user subscriptions: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get subscriptions"})
			return
		}
		defer rows.Close()

		// Build map of current subscriptions
		currentSubs := make(map[string]bool)
		for rows.Next() {
			var url string
			if err := rows.Scan(&url); err != nil {
				log.Printf("Error scanning subscription URL: %v", err)
				continue
			}
			currentSubs[url] = true
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating subscription rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process subscriptions"})
			return
		}

		// Query for similar podcasts based on categories of current subscriptions
		rows, err = database.Query(`
			WITH user_categories AS (
				SELECT DISTINCT unnest(string_to_array(p.Categories, ',')) as category
				FROM "Podcasts" p
				WHERE p.UserID = $1 AND p.Categories IS NOT NULL AND p.Categories != ''
			),
			recommended_podcasts AS (
				SELECT DISTINCT ON (p.PodcastID)
					p.PodcastID,
					p.PodcastName,
					p.Author,
					p.Description,
					p.FeedURL,
					p.WebsiteURL,
					p.ArtworkURL,
					COUNT(DISTINCT u.UserID) as subscribers
				FROM "Podcasts" p
				JOIN "Users" u ON p.UserID = u.UserID
				WHERE EXISTS (
					SELECT 1 FROM user_categories uc
					WHERE p.Categories ILIKE '%' || uc.category || '%'
				)
				AND p.FeedURL NOT IN (
					SELECT FeedURL FROM "Podcasts" WHERE UserID = $1
				)
				GROUP BY p.PodcastID
				ORDER BY p.PodcastID, subscribers DESC
			)
			SELECT * FROM recommended_podcasts
			LIMIT $2
		`, userID, count)

		if err != nil {
			log.Printf("Error querying suggested podcasts: %v", err)

			// If category-based query fails, fall back to popularity-based suggestions
			rows, err = database.Query(`
				SELECT
					p.PodcastID,
					p.PodcastName,
					p.Author,
					p.Description,
					p.FeedURL,
					p.WebsiteURL,
					p.ArtworkURL,
					COUNT(DISTINCT u.UserID) as subscribers
				FROM "Podcasts" p
				JOIN "Users" u ON p.UserID = u.UserID
				WHERE p.FeedURL NOT IN (
					SELECT FeedURL FROM "Podcasts" WHERE UserID = $1
				)
				GROUP BY p.PodcastID
				ORDER BY subscribers DESC, p.PodcastID
				LIMIT $2
			`, userID, count)

			if err != nil {
				log.Printf("Error querying popular podcasts: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to get suggestions"})
				return
			}
		}
		defer rows.Close()

		// Build podcast list
		podcasts := make([]models.Podcast, 0)
		for rows.Next() {
			var podcast models.Podcast
			var podcastID int
			var author, description, websiteURL, artworkURL sql.NullString
			var subscribers int

			if err := rows.Scan(
				&podcastID,
				&podcast.Title,
				&author,
				&description,
				&podcast.URL,
				&websiteURL,
				&artworkURL,
				&subscribers,
			); err != nil {
				log.Printf("Error scanning podcast: %v", err)
				continue
			}

			// Skip if already subscribed (double-check)
			if currentSubs[podcast.URL] {
				continue
			}

			// Set optional fields if present
			if author.Valid {
				podcast.Author = author.String
			}

			if description.Valid {
				podcast.Description = description.String
			}

			if websiteURL.Valid {
				podcast.Website = websiteURL.String
			}

			if artworkURL.Valid {
				podcast.LogoURL = artworkURL.String
			}

			podcast.Subscribers = subscribers
			podcast.MygpoLink = fmt.Sprintf("/podcast/%d", podcastID)

			podcasts = append(podcasts, podcast)
		}

		if err = rows.Err(); err != nil {
			log.Printf("Error iterating suggestion rows: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to process suggestions"})
			return
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
		case "opml":
			// OPML format
			opml := generateOpml(podcasts)
			c.Header("Content-Type", "text/xml")
			c.String(http.StatusOK, opml)
		default:
			c.JSON(http.StatusBadRequest, gin.H{"error": "Unsupported format"})
		}
	}
}

// generateOpml generates an OPML format document from a list of podcasts
func generateOpml(podcasts []models.Podcast) string {
	var sb strings.Builder

	sb.WriteString(`<?xml version="1.0" encoding="utf-8"?>
<opml version="1.0">
  <head>
    <title>gPodder Subscriptions</title>
  </head>
  <body>
`)

	for _, podcast := range podcasts {
		sb.WriteString(fmt.Sprintf(`    <outline text="%s" type="rss" xmlUrl="%s"`,
			escapeXml(podcast.Title), escapeXml(podcast.URL)))

		if podcast.Website != "" {
			sb.WriteString(fmt.Sprintf(` htmlUrl="%s"`, escapeXml(podcast.Website)))
		}

		if podcast.Description != "" {
			sb.WriteString(fmt.Sprintf(` description="%s"`, escapeXml(podcast.Description)))
		}

		sb.WriteString(" />\n")
	}

	sb.WriteString(`  </body>
</opml>`)

	return sb.String()
}

// generateXml generates an XML format document from a list of podcasts
func generateXml(podcasts []models.Podcast) string {
	var sb strings.Builder

	sb.WriteString(`<?xml version="1.0" encoding="utf-8"?>
<podcasts>
`)

	for _, podcast := range podcasts {
		sb.WriteString("  <podcast>\n")
		sb.WriteString(fmt.Sprintf("    <title>%s</title>\n", escapeXml(podcast.Title)))
		sb.WriteString(fmt.Sprintf("    <url>%s</url>\n", escapeXml(podcast.URL)))

		if podcast.Website != "" {
			sb.WriteString(fmt.Sprintf("    <website>%s</website>\n", escapeXml(podcast.Website)))
		}

		if podcast.MygpoLink != "" {
			sb.WriteString(fmt.Sprintf("    <mygpo_link>%s</mygpo_link>\n", escapeXml(podcast.MygpoLink)))
		}

		if podcast.Author != "" {
			sb.WriteString(fmt.Sprintf("    <author>%s</author>\n", escapeXml(podcast.Author)))
		}

		if podcast.Description != "" {
			sb.WriteString(fmt.Sprintf("    <description>%s</description>\n", escapeXml(podcast.Description)))
		}

		sb.WriteString(fmt.Sprintf("    <subscribers>%d</subscribers>\n", podcast.Subscribers))

		if podcast.LogoURL != "" {
			sb.WriteString(fmt.Sprintf("    <logo_url>%s</logo_url>\n", escapeXml(podcast.LogoURL)))
		}

		if podcast.ScaledLogoURL != "" {
			sb.WriteString(fmt.Sprintf("    <scaled_logo_url>%s</scaled_logo_url>\n", escapeXml(podcast.ScaledLogoURL)))
		}

		sb.WriteString("  </podcast>\n")
	}

	sb.WriteString("</podcasts>")

	return sb.String()
}

// escapeXml escapes special characters for XML output
func escapeXml(s string) string {
	s = strings.ReplaceAll(s, "&", "&amp;")
	s = strings.ReplaceAll(s, "<", "&lt;")
	s = strings.ReplaceAll(s, ">", "&gt;")
	s = strings.ReplaceAll(s, "\"", "&quot;")
	s = strings.ReplaceAll(s, "'", "&apos;")
	return s
}
