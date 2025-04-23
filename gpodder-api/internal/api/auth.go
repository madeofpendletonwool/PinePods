// Package api provides the API endpoints for the gpodder API
package api

import (
	"crypto/rand"
	"database/sql"
	"encoding/base64"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"

	"pinepods/gpodder-api/internal/db"

	"github.com/alexedwards/argon2id"
	"github.com/fernet/fernet-go"
	"github.com/gin-gonic/gin"
)

// Define the parameters we use for Argon2id
type argon2Params struct {
	memory      uint32
	iterations  uint32
	parallelism uint8
	saltLength  uint32
	keyLength   uint32
}

// AuthMiddleware creates a middleware for authentication
func AuthMiddleware(db *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] AuthMiddleware processing request: %s %s", c.Request.Method, c.Request.URL.Path)

		// Get the username from the URL parameters
		username := c.Param("username")
		if username == "" {
			log.Printf("[ERROR] AuthMiddleware: Username parameter is missing in path")
			c.JSON(http.StatusBadRequest, gin.H{"error": "Username is required"})
			c.Abort()
			return
		}

		log.Printf("[DEBUG] AuthMiddleware: Processing request for username: %s", username)

		// Check if this is an internal API call via X-GPodder-Token
		gpodderTokenHeader := c.GetHeader("X-GPodder-Token")
		if gpodderTokenHeader != "" {
			log.Printf("[DEBUG] AuthMiddleware: Found X-GPodder-Token header")

			// Get user data
			var userID int
			var gpodderToken sql.NullString
			var podSyncType string

			err := db.QueryRow(`
				SELECT UserID, GpodderToken, Pod_Sync_Type FROM "Users"
				WHERE LOWER(Username) = LOWER($1)
			`, username).Scan(&userID, &gpodderToken, &podSyncType)

			if err != nil {
				log.Printf("[ERROR] AuthMiddleware: Database error: %v", err)
				c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or token"})
				c.Abort()
				return
			}

			// Check if gpodder sync is enabled
			if podSyncType != "gpodder" && podSyncType != "both" && podSyncType != "external" {
				log.Printf("[ERROR] AuthMiddleware: Gpodder API not enabled for user: %s (sync type: %s)",
					username, podSyncType)
				c.JSON(http.StatusForbidden, gin.H{"error": "Gpodder API not enabled for this user"})
				c.Abort()
				return
			}

			// For internal calls with X-GPodder-Token header, validate token directly
			if gpodderToken.Valid && gpodderToken.String == gpodderTokenHeader {
				log.Printf("[DEBUG] AuthMiddleware: X-GPodder-Token validated for user: %s", username)
				c.Set("userID", userID)
				c.Set("username", username)
				c.Next()
				return
			}

			// If token doesn't match, authentication failed
			log.Printf("[ERROR] AuthMiddleware: Invalid X-GPodder-Token for user: %s", username)
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid token"})
			c.Abort()
			return
		}

		// If no token header found, proceed with standard authentication
		authHeader := c.GetHeader("Authorization")
		if authHeader == "" {
			log.Printf("[ERROR] AuthMiddleware: Authorization header is missing")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Authorization header is required"})
			c.Abort()
			return
		}

		// Extract credentials
		parts := strings.Split(authHeader, " ")
		if len(parts) != 2 || parts[0] != "Basic" {
			log.Printf("[ERROR] AuthMiddleware: Invalid Authorization header format")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid Authorization header format"})
			c.Abort()
			return
		}

		// Decode credentials
		decoded, err := base64.StdEncoding.DecodeString(parts[1])
		if err != nil {
			log.Printf("[ERROR] AuthMiddleware: Failed to decode base64 credentials: %v", err)
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid Authorization header"})
			c.Abort()
			return
		}

		// Extract username and password
		credentials := strings.SplitN(string(decoded), ":", 2)
		if len(credentials) != 2 {
			log.Printf("[ERROR] AuthMiddleware: Invalid credentials format")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid credentials format"})
			c.Abort()
			return
		}

		authUsername := credentials[0]
		password := credentials[1]

		// Check username match
		if strings.ToLower(username) != strings.ToLower(authUsername) {
			log.Printf("[ERROR] AuthMiddleware: Username mismatch - URL: %s, Auth: %s",
				strings.ToLower(username), strings.ToLower(authUsername))
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Username mismatch"})
			c.Abort()
			return
		}

		// Query user data
		var userID int
		var hashedPassword string
		var podSyncType string
		var gpodderToken sql.NullString

		err = db.QueryRow(`
			SELECT UserID, Hashed_PW, Pod_Sync_Type, GpodderToken FROM "Users"
			WHERE LOWER(Username) = LOWER($1)
		`, username).Scan(&userID, &hashedPassword, &podSyncType, &gpodderToken)

		if err != nil {
			if err == sql.ErrNoRows {
				log.Printf("[ERROR] AuthMiddleware: User not found: %s", username)
				c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or password"})
			} else {
				log.Printf("[ERROR] AuthMiddleware: Database error: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Database error"})
			}
			c.Abort()
			return
		}

		// Check if gpodder sync is enabled
		if podSyncType != "gpodder" && podSyncType != "both" && podSyncType != "external" {
			log.Printf("[ERROR] AuthMiddleware: Gpodder API not enabled for user: %s (sync type: %s)",
				username, podSyncType)
			c.JSON(http.StatusForbidden, gin.H{"error": "Gpodder API not enabled for this user"})
			c.Abort()
			return
		}

		// Flag to track authentication success
		authenticated := false

		// Check if this is a gpodder token authentication
		// Check if this is a gpodder token authentication
		if gpodderToken.Valid && (gpodderToken.String == password || gpodderToken.String == gpodderTokenHeader) {
			log.Printf("[DEBUG] AuthMiddleware: User authenticated with gpodder token: %s", username)
			authenticated = true
		}

		// If token auth didn't succeed, try password authentication
		if !authenticated && verifyPassword(password, hashedPassword) {
			log.Printf("[DEBUG] AuthMiddleware: User authenticated with password: %s", username)
			authenticated = true
		}

		// If authentication was successful, set context and continue
		if authenticated {
			c.Set("userID", userID)
			c.Set("username", username)
			c.Next()
			return
		}

		// Authentication failed
		log.Printf("[ERROR] AuthMiddleware: Invalid credentials for user: %s", username)
		c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or password"})
		c.Abort()
	}
}

// Helper function to decrypt token
func decryptToken(encryptionKey []byte, encryptedToken string) (string, error) {
	// Ensure the encryptionKey is correctly formatted for fernet
	// Fernet requires a 32-byte key encoded in base64
	keyStr := base64.StdEncoding.EncodeToString(encryptionKey)

	// Parse the key
	key, err := fernet.DecodeKey(keyStr)
	if err != nil {
		return "", fmt.Errorf("failed to decode key: %w", err)
	}

	// Decrypt the token
	token := []byte(encryptedToken)
	msg := fernet.VerifyAndDecrypt(token, 0, []*fernet.Key{key})
	if msg == nil {
		return "", fmt.Errorf("failed to decrypt token or token invalid")
	}

	return string(msg), nil
}

// generateSessionToken generates a random token for sessions
func generateSessionToken() (string, error) {
	b := make([]byte, 32)
	_, err := rand.Read(b)
	if err != nil {
		return "", err
	}
	return base64.URLEncoding.EncodeToString(b), nil
}

// createSession creates a new session in the database
func createSession(db *db.PostgresDB, userID int, userAgent, clientIP string) (string, time.Time, error) {
	// Generate a random session token
	token, err := generateSessionToken()
	if err != nil {
		return "", time.Time{}, fmt.Errorf("failed to generate session token: %w", err)
	}

	// Set expiration time (30 days from now)
	expires := time.Now().Add(30 * 24 * time.Hour)

	// Insert session into database
	_, err = db.Exec(`
		INSERT INTO "GpodderSessions" (UserID, SessionToken, ExpiresAt, UserAgent, ClientIP)
		VALUES ($1, $2, $3, $4, $5)
	`, userID, token, expires, userAgent, clientIP)

	if err != nil {
		return "", time.Time{}, fmt.Errorf("failed to create session: %w", err)
	}

	return token, expires, nil
}

// validateSession validates a session token
func validateSession(db *db.PostgresDB, token string) (int, bool, error) {
	var userID int
	var expires time.Time

	err := db.QueryRow(`
		SELECT UserID, ExpiresAt
		FROM "GpodderSessions"
		WHERE SessionToken = $1
	`, token).Scan(&userID, &expires)

	if err != nil {
		if err == sql.ErrNoRows {
			return 0, false, nil // Session not found
		}
		return 0, false, fmt.Errorf("error validating session: %w", err)
	}

	// Check if session has expired
	if time.Now().After(expires) {
		// Delete expired session
		_, err = db.Exec(`DELETE FROM "GpodderSessions" WHERE SessionToken = $1`, token)
		if err != nil {
			log.Printf("Failed to delete expired session: %v", err)
		}
		return 0, false, nil
	}

	// Update last active time
	_, err = db.Exec(`
		UPDATE "GpodderSessions"
		SET LastActive = CURRENT_TIMESTAMP
		WHERE SessionToken = $1
	`, token)

	if err != nil {
		log.Printf("Failed to update session last active time: %v", err)
	}

	return userID, true, nil
}

// deleteSession removes a session from the database
func deleteSession(db *db.PostgresDB, token string) error {
	_, err := db.Exec(`DELETE FROM "GpodderSessions" WHERE SessionToken = $1`, token)
	if err != nil {
		return fmt.Errorf("failed to delete session: %w", err)
	}
	return nil
}

// deleteUserSessions removes all sessions for a user
func deleteUserSessions(db *db.PostgresDB, userID int) error {
	_, err := db.Exec(`DELETE FROM "GpodderSessions" WHERE UserID = $1`, userID)
	if err != nil {
		return fmt.Errorf("failed to delete user sessions: %w", err)
	}
	return nil
}

// handleLogin enhanced with session management
func handleLogin(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Use the AuthMiddleware to authenticate the user
		username := c.Param("username")
		if username == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Username is required"})
			return
		}

		// Get the Authorization header
		authHeader := c.GetHeader("Authorization")
		if authHeader == "" {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Authorization header is required"})
			return
		}

		// Check if the Authorization header is in the correct format
		parts := strings.Split(authHeader, " ")
		if len(parts) != 2 || parts[0] != "Basic" {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid Authorization header format"})
			return
		}

		// Decode the base64-encoded credentials
		decoded, err := base64.StdEncoding.DecodeString(parts[1])
		if err != nil {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid Authorization header"})
			return
		}

		// Extract username and password
		credentials := strings.SplitN(string(decoded), ":", 2)
		if len(credentials) != 2 {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid credentials format"})
			return
		}

		authUsername := credentials[0]
		password := credentials[1]

		// Verify that the username in the URL matches the one in the Authorization header
		if strings.ToLower(username) != strings.ToLower(authUsername) {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Username mismatch"})
			return
		}

		// Check if the user exists and the password is correct
		var userID int
		var hashedPassword string
		var podSyncType string

		// Make sure to use case-insensitive username lookup
		err = database.QueryRow(`
			SELECT UserID, Hashed_PW, Pod_Sync_Type FROM "Users" WHERE LOWER(Username) = LOWER($1)
		`, username).Scan(&userID, &hashedPassword, &podSyncType)

		if err != nil {
			if err == sql.ErrNoRows {
				c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or password"})
			} else {
				log.Printf("Database error during login: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Database error"})
			}
			return
		}

		// Check if gpodder sync is enabled for this user
		if podSyncType != "gpodder" && podSyncType != "both" {
			c.JSON(http.StatusForbidden, gin.H{"error": "Gpodder API not enabled for this user"})
			return
		}

		// Verify password using Pinepods' Argon2 password method
		if !verifyPassword(password, hashedPassword) {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or password"})
			return
		}

		// Create a new session
		userAgent := c.Request.UserAgent()
		clientIP := c.ClientIP()
		sessionToken, expiresAt, err := createSession(database, userID, userAgent, clientIP)

		if err != nil {
			log.Printf("Failed to create session: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Failed to create session"})
			return
		}

		log.Printf("[DEBUG] handleLogin: Login successful for user: %s, created session token (first 8 chars): %s...",
			username, sessionToken[:8])

		// Set session cookie
		c.SetCookie(
			"sessionid",                    // name
			sessionToken,                   // value
			int(30*24*time.Hour.Seconds()), // max age in seconds (30 days)
			"/",                            // path
			"",                             // domain (empty = current domain)
			c.Request.TLS != nil,           // secure (HTTPS only)
			true,                           // httpOnly (not accessible via JavaScript)
		)

		log.Printf("[DEBUG] handleLogin: Sending response with session expiry: %s",
			expiresAt.Format(time.RFC3339))
		// Return success with info
		c.JSON(http.StatusOK, gin.H{
			"status":          "success",
			"userid":          userID,
			"username":        username,
			"session_expires": expiresAt.Format(time.RFC3339),
		})
	}
}

// handleLogout enhanced with session management
func handleLogout(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Get username from URL
		username := c.Param("username")
		if username == "" {
			c.JSON(http.StatusBadRequest, gin.H{"error": "Username is required"})
			return
		}

		// Get the session cookie
		sessionToken, err := c.Cookie("sessionid")
		if err != nil || sessionToken == "" {
			// No session cookie, just return success (idempotent operation)
			c.JSON(http.StatusOK, gin.H{
				"status": "logged out",
			})
			return
		}

		// Delete the session
		err = deleteSession(database, sessionToken)
		if err != nil {
			log.Printf("Error deleting session: %v", err)
			// Continue anyway - we still want to invalidate the cookie
		}

		// Clear the session cookie
		c.SetCookie(
			"sessionid",          // name
			"",                   // value (empty = delete)
			-1,                   // max age (negative = delete)
			"/",                  // path
			"",                   // domain
			c.Request.TLS != nil, // secure
			true,                 // httpOnly
		)

		c.JSON(http.StatusOK, gin.H{
			"status": "logged out",
		})
	}
}

// SessionMiddleware checks if a user is logged in via session
func SessionMiddleware(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] SessionMiddleware processing request: %s %s",
			c.Request.Method, c.Request.URL.Path)

		// First, try to get user from Authorization header for direct API access
		authHeader := c.GetHeader("Authorization")
		if authHeader != "" {
			log.Printf("[DEBUG] SessionMiddleware: Authorization header found, passing to next middleware")
			c.Next()
			return
		}

		// No Authorization header, check for session cookie
		sessionToken, err := c.Cookie("sessionid")
		if err != nil || sessionToken == "" {
			log.Printf("[ERROR] SessionMiddleware: No session cookie found: %v", err)
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Not logged in"})
			c.Abort()
			return
		}

		log.Printf("[DEBUG] SessionMiddleware: Found session cookie, validating")

		// Validate the session
		userID, valid, err := validateSession(database, sessionToken)
		if err != nil {
			log.Printf("[ERROR] SessionMiddleware: Error validating session: %v", err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "Session error"})
			c.Abort()
			return
		}

		if !valid {
			log.Printf("[ERROR] SessionMiddleware: Invalid or expired session")
			// Clear the invalid cookie
			c.SetCookie("sessionid", "", -1, "/", "", c.Request.TLS != nil, true)
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Session expired"})
			c.Abort()
			return
		}

		log.Printf("[DEBUG] SessionMiddleware: Session valid for userID: %d", userID)

		// Get the username for the user ID
		var username string
		err = database.QueryRow(`SELECT Username FROM "Users" WHERE UserID = $1`, userID).Scan(&username)
		if err != nil {
			log.Printf("[ERROR] SessionMiddleware: Error getting username for userID %d: %v",
				userID, err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "User data error"})
			c.Abort()
			return
		}

		// Check if gpodder sync is enabled for this user
		var podSyncType string
		err = database.QueryRow(`SELECT Pod_Sync_Type FROM "Users" WHERE UserID = $1`, userID).Scan(&podSyncType)
		if err != nil {
			log.Printf("[ERROR] SessionMiddleware: Error checking sync type for userID %d: %v",
				userID, err)
			c.JSON(http.StatusInternalServerError, gin.H{"error": "User data error"})
			c.Abort()
			return
		}

		if podSyncType != "gpodder" && podSyncType != "both" {
			log.Printf("[ERROR] SessionMiddleware: Gpodder API not enabled for user: %s (sync type: %s)",
				username, podSyncType)
			c.JSON(http.StatusForbidden, gin.H{"error": "Gpodder API not enabled for this user"})
			c.Abort()
			return
		}

		// Set the user information in the context
		c.Set("userID", userID)
		c.Set("username", username)

		// Check if the path username matches the session username
		pathUsername := c.Param("username")
		if pathUsername != "" && strings.ToLower(pathUsername) != strings.ToLower(username) {
			log.Printf("[ERROR] SessionMiddleware: Username mismatch - Path: %s, Session: %s",
				pathUsername, username)
			c.JSON(http.StatusForbidden, gin.H{"error": "Username mismatch"})
			c.Abort()
			return
		}

		log.Printf("[DEBUG] SessionMiddleware: Session authentication successful for user: %s", username)
		c.Next()
	}
}

// AuthenticationMiddleware with GPodder token handling
func AuthenticationMiddleware(database *db.PostgresDB) gin.HandlerFunc {
	return func(c *gin.Context) {
		log.Printf("[DEBUG] AuthenticationMiddleware processing request: %s %s",
			c.Request.Method, c.Request.URL.Path)

		if strings.Contains(c.Request.URL.Path, "/episodes/") && strings.HasSuffix(c.Request.URL.Path, ".json") {
			// Extract username from URL path for episode actions
			parts := strings.Split(c.Request.URL.Path, "/")
			if len(parts) >= 3 {
				// The path format is /episodes/username.json
				usernameWithExt := parts[len(parts)-1]
				// Remove .json extension
				username := strings.TrimSuffix(usernameWithExt, ".json")
				// Set it as the username parameter
				c.Params = append(c.Params, gin.Param{Key: "username", Value: username})
				log.Printf("[DEBUG] AuthenticationMiddleware: Extracted username '%s' from episode actions URL", username)
			}
		}

		// First try session auth
		sessionToken, err := c.Cookie("sessionid")
		if err == nil && sessionToken != "" {
			log.Printf("[DEBUG] AuthenticationMiddleware: Found session cookie, validating")

			userID, valid, err := validateSession(database, sessionToken)
			if err == nil && valid {
				log.Printf("[DEBUG] AuthenticationMiddleware: Session valid for userID: %d", userID)

				var username string
				err = database.QueryRow(`SELECT Username FROM "Users" WHERE UserID = $1`, userID).Scan(&username)
				if err == nil {
					var podSyncType string
					err = database.QueryRow(`SELECT Pod_Sync_Type FROM "Users" WHERE UserID = $1`, userID).Scan(&podSyncType)

					if err == nil && (podSyncType == "gpodder" || podSyncType == "both") {
						// Check if the path username matches the session username
						pathUsername := c.Param("username")
						if pathUsername == "" || strings.ToLower(pathUsername) == strings.ToLower(username) {
							log.Printf("[DEBUG] AuthenticationMiddleware: Session auth successful for user: %s",
								username)
							c.Set("userID", userID)
							c.Set("username", username)
							c.Next()
							return
						} else {
							log.Printf("[ERROR] AuthenticationMiddleware: Session username mismatch - Path: %s, Session: %s",
								pathUsername, username)
						}
					} else {
						log.Printf("[ERROR] AuthenticationMiddleware: Gpodder not enabled for user: %s", username)
					}
				} else {
					log.Printf("[ERROR] AuthenticationMiddleware: Could not get username for userID %d: %v",
						userID, err)
				}
			} else {
				log.Printf("[ERROR] AuthenticationMiddleware: Invalid session: %v", err)
			}
		} else {
			log.Printf("[DEBUG] AuthenticationMiddleware: No session cookie, falling back to basic auth")
		}

		// Try basic auth if session auth failed
		log.Printf("[DEBUG] AuthenticationMiddleware: Attempting basic auth")

		username := c.Param("username")
		if username == "" {
			log.Printf("[ERROR] AuthenticationMiddleware: Username parameter is missing in path")
			c.JSON(http.StatusBadRequest, gin.H{"error": "Username is required"})
			return
		}

		// Check if this is an internal API call via X-GPodder-Token
		gpodderTokenHeader := c.GetHeader("X-GPodder-Token")
		if gpodderTokenHeader != "" {
			log.Printf("[DEBUG] AuthenticationMiddleware: Found X-GPodder-Token header")

			// Get user data
			var userID int
			var gpodderToken sql.NullString
			var podSyncType string

			err := database.QueryRow(`
                SELECT UserID, GpodderToken, Pod_Sync_Type FROM "Users"
                WHERE LOWER(Username) = LOWER($1)
            `, username).Scan(&userID, &gpodderToken, &podSyncType)

			if err != nil {
				log.Printf("[ERROR] AuthenticationMiddleware: Database error: %v", err)
				c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or token"})
				return
			}

			// Check if gpodder sync is enabled
			if podSyncType != "gpodder" && podSyncType != "both" && podSyncType != "external" {
				log.Printf("[ERROR] AuthenticationMiddleware: Gpodder API not enabled for user: %s (sync type: %s)",
					username, podSyncType)
				c.JSON(http.StatusForbidden, gin.H{"error": "Gpodder API not enabled for this user"})
				return
			}

			// For internal calls with X-GPodder-Token header, validate token directly
			if gpodderToken.Valid && gpodderToken.String == gpodderTokenHeader {
				log.Printf("[DEBUG] AuthenticationMiddleware: X-GPodder-Token validated for user: %s", username)
				c.Set("userID", userID)
				c.Set("username", username)
				c.Next()
				return
			}

			// If token doesn't match, authentication failed
			log.Printf("[ERROR] AuthenticationMiddleware: Invalid X-GPodder-Token for user: %s", username)
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid token"})
			return
		}

		// Standard basic auth handling
		authHeader := c.GetHeader("Authorization")
		if authHeader == "" {
			log.Printf("[ERROR] AuthenticationMiddleware: No Authorization header found")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Not authenticated"})
			return
		}

		parts := strings.Split(authHeader, " ")
		if len(parts) != 2 || parts[0] != "Basic" {
			log.Printf("[ERROR] AuthenticationMiddleware: Invalid Authorization header format")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid Authorization header format"})
			return
		}

		decoded, err := base64.StdEncoding.DecodeString(parts[1])
		if err != nil {
			log.Printf("[ERROR] AuthenticationMiddleware: Failed to decode base64 credentials: %v", err)
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid Authorization header"})
			return
		}

		credentials := strings.SplitN(string(decoded), ":", 2)
		if len(credentials) != 2 {
			log.Printf("[ERROR] AuthenticationMiddleware: Invalid credentials format")
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid credentials format"})
			return
		}

		authUsername := credentials[0]
		password := credentials[1]

		if strings.ToLower(username) != strings.ToLower(authUsername) {
			log.Printf("[ERROR] AuthenticationMiddleware: Username mismatch - URL: %s, Auth: %s",
				strings.ToLower(username), strings.ToLower(authUsername))
			c.JSON(http.StatusUnauthorized, gin.H{"error": "Username mismatch"})
			return
		}

		var userID int
		var hashedPassword string
		var podSyncType string
		var gpodderToken sql.NullString

		err = database.QueryRow(`
            SELECT UserID, Hashed_PW, Pod_Sync_Type, GpodderToken FROM "Users" WHERE LOWER(Username) = LOWER($1)
        `, username).Scan(&userID, &hashedPassword, &podSyncType, &gpodderToken)

		if err != nil {
			if err == sql.ErrNoRows {
				log.Printf("[ERROR] AuthenticationMiddleware: User not found: %s", username)
				c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or password"})
			} else {
				log.Printf("[ERROR] AuthenticationMiddleware: Database error: %v", err)
				c.JSON(http.StatusInternalServerError, gin.H{"error": "Database error"})
			}
			return
		}

		if podSyncType != "gpodder" && podSyncType != "both" && podSyncType != "external" {
			log.Printf("[ERROR] AuthenticationMiddleware: Gpodder API not enabled for user: %s (sync type: %s)",
				username, podSyncType)
			c.JSON(http.StatusForbidden, gin.H{"error": "Gpodder API not enabled for this user"})
			return
		}

		// Flag to track authentication success
		authenticated := false

		// Check if this is a gpodder token authentication
		if gpodderToken.Valid && gpodderToken.String == password {
			log.Printf("[DEBUG] AuthenticationMiddleware: User authenticated with gpodder token: %s", username)
			authenticated = true
		}

		// If token auth didn't succeed, try password authentication
		if !authenticated && verifyPassword(password, hashedPassword) {
			log.Printf("[DEBUG] AuthenticationMiddleware: User authenticated with password: %s", username)
			authenticated = true
		}

		// If authentication was successful, set context and continue
		if authenticated {
			c.Set("userID", userID)
			c.Set("username", username)
			c.Next()
			return
		}

		// Authentication failed
		log.Printf("[ERROR] AuthenticationMiddleware: Invalid credentials for user: %s", username)
		c.JSON(http.StatusUnauthorized, gin.H{"error": "Invalid username or password"})
	}
}

// verifyPassword verifies a password against a hash using Argon2
// This implementation matches the Pinepods authentication mechanism using alexedwards/argon2id
func verifyPassword(password, hashedPassword string) bool {
	// Use the alexedwards/argon2id package to compare password and hash
	match, err := argon2id.ComparePasswordAndHash(password, hashedPassword)
	if err != nil {
		// Log the error in a production environment
		return false
	}

	return match
}
