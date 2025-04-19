package db

import (
	"database/sql"
	"fmt"
	"net/url"
	"os"
	"pinepods/gpodder-api/config"
	"strings"

	_ "github.com/lib/pq"
)

// PostgresDB represents a connection to the PostgreSQL database
type PostgresDB struct {
	*sql.DB
}

// NewPostgresDB creates a new PostgreSQL database connection
func NewPostgresDB(cfg config.DatabaseConfig) (*PostgresDB, error) {
	// Print connection details for debugging (hide password for security)
	fmt.Printf("Connecting to database: host=%s port=%d user=%s dbname=%s sslmode=%s\n",
		cfg.Host, cfg.Port, cfg.User, cfg.DBName, cfg.SSLMode)

	// Get password directly from environment to handle special characters
	password := os.Getenv("DB_PASSWORD")
	if password == "" {
		// Fall back to config if env var is empty
		password = cfg.Password
	}

	// Escape special characters in password
	escapedPassword := url.QueryEscape(password)

	// Use a connection string without password for logging
	logConnStr := fmt.Sprintf(
		"host=%s port=%d user=%s dbname=%s sslmode=%s",
		cfg.Host, cfg.Port, cfg.User, cfg.DBName, cfg.SSLMode,
	)
	fmt.Printf("Connection string (without password): %s\n", logConnStr)

	// Build the actual connection string with password
	connStr := fmt.Sprintf(
		"host=%s port=%d user=%s password=%s dbname=%s sslmode=%s",
		cfg.Host, cfg.Port, cfg.User, password, cfg.DBName, cfg.SSLMode,
	)

	// Try alternate connection string format if the first fails
	db, err := sql.Open("postgres", connStr)
	if err != nil {
		// Try URL format connection string
		urlConnStr := fmt.Sprintf(
			"postgres://%s:%s@%s:%d/%s?sslmode=%s",
			cfg.User, escapedPassword, cfg.Host, cfg.Port, cfg.DBName, cfg.SSLMode,
		)
		fmt.Println("First connection attempt failed, trying URL format...")
		db, err = sql.Open("postgres", urlConnStr)
		if err != nil {
			return nil, fmt.Errorf("failed to open database connection: %w", err)
		}
	}

	// Test the connection
	if err := db.Ping(); err != nil {
		db.Close()
		// Check if error contains password authentication failure
		if strings.Contains(err.Error(), "password authentication failed") {
			// Print environment variables (hide password)
			fmt.Println("Password authentication failed. Environment variables:")
			fmt.Printf("DB_HOST=%s\n", os.Getenv("DB_HOST"))
			fmt.Printf("DB_PORT=%s\n", os.Getenv("DB_PORT"))
			fmt.Printf("DB_USER=%s\n", os.Getenv("DB_USER"))
			fmt.Printf("DB_NAME=%s\n", os.Getenv("DB_NAME"))
			fmt.Printf("DB_PASSWORD=*** (length: %d)\n", len(os.Getenv("DB_PASSWORD")))
		}
		return nil, fmt.Errorf("failed to ping database: %w", err)
	}

	fmt.Println("Successfully connected to the database")

	// Run migrations to ensure schema is up to date
	if err := RunMigrations(db); err != nil {
		db.Close()
		return nil, fmt.Errorf("failed to run migrations: %w", err)
	}

	return &PostgresDB{DB: db}, nil
}

// Close closes the database connection
func (db *PostgresDB) Close() error {
	return db.DB.Close()
}
