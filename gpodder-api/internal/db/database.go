package db

import (
	"context"
	"database/sql"
	"fmt"
	"net/url"
	"os"
	"pinepods/gpodder-api/config"
	"regexp"
	"strings"
	"time"

	_ "github.com/go-sql-driver/mysql" // MySQL driver
	_ "github.com/lib/pq"              // PostgreSQL driver
)

// Database represents a database connection that can be either PostgreSQL or MySQL
type Database struct {
	*sql.DB
	Type string // "postgresql" or "mysql"
}

// NewDatabase creates a new database connection based on the DB_TYPE environment variable
func NewDatabase(cfg config.DatabaseConfig) (*Database, error) {
	// Print connection details for debugging (hide password for security)
	fmt.Printf("Connecting to %s database: host=%s port=%d user=%s dbname=%s\n",
		cfg.Type, cfg.Host, cfg.Port, cfg.User, cfg.DBName)

	var db *sql.DB
	var err error

	switch cfg.Type {
	case "postgresql":
		db, err = connectPostgreSQL(cfg)
	case "mysql", "mariadb":
		db, err = connectMySQL(cfg)
	default:
		return nil, fmt.Errorf("unsupported database type: %s", cfg.Type)
	}

	if err != nil {
		return nil, err
	}

	// Test the connection
	if err := db.Ping(); err != nil {
		db.Close()
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
	if err := RunMigrations(db, cfg.Type); err != nil {
		db.Close()
		return nil, fmt.Errorf("failed to run migrations: %w", err)
	}

	return &Database{DB: db, Type: cfg.Type}, nil
}

// connectPostgreSQL connects to a PostgreSQL database
func connectPostgreSQL(cfg config.DatabaseConfig) (*sql.DB, error) {
	// Escape special characters in password
	escapedPassword := url.QueryEscape(cfg.Password)

	// Use a connection string without password for logging
	logConnStr := fmt.Sprintf(
		"host=%s port=%d user=%s dbname=%s sslmode=%s",
		cfg.Host, cfg.Port, cfg.User, cfg.DBName, cfg.SSLMode,
	)
	fmt.Printf("PostgreSQL connection string (without password): %s\n", logConnStr)

	// Build the actual connection string with password
	connStr := fmt.Sprintf(
		"host=%s port=%d user=%s password=%s dbname=%s sslmode=%s",
		cfg.Host, cfg.Port, cfg.User, cfg.Password, cfg.DBName, cfg.SSLMode,
	)

	// Try standard connection string first
	db, err := sql.Open("postgres", connStr)
	if err != nil {
		// Try URL format connection string
		urlConnStr := fmt.Sprintf(
			"postgres://%s:%s@%s:%d/%s?sslmode=%s",
			cfg.User, escapedPassword, cfg.Host, cfg.Port, cfg.DBName, cfg.SSLMode,
		)
		fmt.Println("First connection attempt failed, trying URL format...")
		db, err = sql.Open("postgres", urlConnStr)
	}

	return db, err
}

// Replace the existing connectMySQL function with this version
func connectMySQL(cfg config.DatabaseConfig) (*sql.DB, error) {
	// Add needed parameters for MySQL authentication
	connStr := fmt.Sprintf(
		"%s:%s@tcp(%s:%d)/%s?parseTime=true&allowNativePasswords=true&multiStatements=true",
		cfg.User, cfg.Password, cfg.Host, cfg.Port, cfg.DBName,
	)

	fmt.Printf("Attempting MySQL connection to %s:%d as user '%s'\n",
		cfg.Host, cfg.Port, cfg.User)

	// Open the connection
	db, err := sql.Open("mysql", connStr)
	if err != nil {
		return nil, fmt.Errorf("failed to open MySQL connection: %w", err)
	}

	// Configure connection pool
	db.SetConnMaxLifetime(time.Minute * 3)
	db.SetMaxOpenConns(10)
	db.SetMaxIdleConns(5)

	// Explicitly test the connection
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	fmt.Println("Testing MySQL connection with ping...")
	if err := db.PingContext(ctx); err != nil {
		db.Close()
		fmt.Printf("MySQL connection failed: %v\n", err)
		return nil, fmt.Errorf("failed to ping MySQL database: %w", err)
	}

	fmt.Println("MySQL connection successful!")
	return db, nil
}

// Close closes the database connection
func (db *Database) Close() error {
	return db.DB.Close()
}

// IsMySQLDB returns true if the database is MySQL/MariaDB
func (db *Database) IsMySQLDB() bool {
	return db.Type == "mysql"
}

// IsPostgreSQLDB returns true if the database is PostgreSQL
func (db *Database) IsPostgreSQLDB() bool {
	return db.Type == "postgresql"
}

// FormatQuery formats a query for the specific database type
func (db *Database) FormatQuery(query string) string {
	if db.Type == "postgresql" {
		return query // PostgreSQL queries already have correct format
	}

	// For MySQL:
	result := query

	// First, replace quoted table names
	knownTables := []string{
		"Users", "GpodderDevices", "GpodderSyncSettings",
		"GpodderSyncSubscriptions", "GpodderSyncEpisodeActions",
		"GpodderSyncPodcastLists", "GpodderSyncState", "GpodderSessions",
		"GpodderSyncMigrations", "Podcasts", "Episodes", "SavedEpisodes",
		"UserEpisodeHistory", "UserSettings", "APIKeys",
	}

	for _, table := range knownTables {
		quoted := fmt.Sprintf("\"%s\"", table)
		result = strings.ReplaceAll(result, quoted, table)
	}

	// Replace column quotes (double quotes to backticks)
	re := regexp.MustCompile(`"([^"]+)"`)
	result = re.ReplaceAllString(result, "`$1`")

	// Then replace placeholders
	for i := 10; i > 0; i-- {
		old := fmt.Sprintf("$%d", i)
		result = strings.ReplaceAll(result, old, "?")
	}

	return result
}

// Exec executes a query with the correct formatting for the database type
func (db *Database) Exec(query string, args ...interface{}) (sql.Result, error) {
	formattedQuery := db.FormatQuery(query)
	return db.DB.Exec(formattedQuery, args...)
}

// Query executes a query with the correct formatting for the database type
func (db *Database) Query(query string, args ...interface{}) (*sql.Rows, error) {
	formattedQuery := db.FormatQuery(query)
	return db.DB.Query(formattedQuery, args...)
}

// QueryRow executes a query with the correct formatting for the database type
func (db *Database) QueryRow(query string, args ...interface{}) *sql.Row {
	formattedQuery := db.FormatQuery(query)
	return db.DB.QueryRow(formattedQuery, args...)
}

// Begin starts a transaction with the correct formatting for the database type
func (db *Database) Begin() (*Transaction, error) {
	tx, err := db.DB.Begin()
	if err != nil {
		return nil, err
	}

	return &Transaction{tx: tx, dbType: db.Type}, nil
}

// Transaction is a wrapper around sql.Tx that formats queries correctly
type Transaction struct {
	tx     *sql.Tx
	dbType string
}

// Commit commits the transaction
func (tx *Transaction) Commit() error {
	return tx.tx.Commit()
}

// Rollback rolls back the transaction
func (tx *Transaction) Rollback() error {
	return tx.tx.Rollback()
}

// Exec executes a query in the transaction with correct formatting
func (tx *Transaction) Exec(query string, args ...interface{}) (sql.Result, error) {
	formattedQuery := formatQuery(query, tx.dbType)
	return tx.tx.Exec(formattedQuery, args...)
}

// Query executes a query in the transaction with correct formatting
func (tx *Transaction) Query(query string, args ...interface{}) (*sql.Rows, error) {
	formattedQuery := formatQuery(query, tx.dbType)
	return tx.tx.Query(formattedQuery, args...)
}

// QueryRow executes a query in the transaction with correct formatting
func (tx *Transaction) QueryRow(query string, args ...interface{}) *sql.Row {
	formattedQuery := formatQuery(query, tx.dbType)
	return tx.tx.QueryRow(formattedQuery, args...)
}

// Helper function to format queries
func formatQuery(query string, dbType string) string {
	if dbType == "postgresql" {
		return query
	}

	// For MySQL:
	// Same logic as FormatQuery method
	result := query

	knownTables := []string{
		"Users", "GpodderDevices", "GpodderSyncSettings",
		"GpodderSyncSubscriptions", "GpodderSyncEpisodeActions",
		"GpodderSyncPodcastLists", "GpodderSyncState", "GpodderSessions",
		"GpodderSyncMigrations", "Podcasts", "Episodes", "SavedEpisodes",
		"UserEpisodeHistory", "UserSettings", "APIKeys",
	}

	for _, table := range knownTables {
		quoted := fmt.Sprintf("\"%s\"", table)
		result = strings.ReplaceAll(result, quoted, table)
	}

	for i := 10; i > 0; i-- {
		old := fmt.Sprintf("$%d", i)
		result = strings.ReplaceAll(result, old, "?")
	}

	return result
}
