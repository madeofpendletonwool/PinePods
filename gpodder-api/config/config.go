package config

import (
	"os"
	"strconv"
)

// Config represents the application configuration
type Config struct {
	Server      ServerConfig
	Database    DatabaseConfig
	Environment string
}

// ServerConfig holds server-related configuration
type ServerConfig struct {
	Port int
}

// DatabaseConfig holds database-related configuration
type DatabaseConfig struct {
	Host     string
	Port     int
	User     string
	Password string
	DBName   string
	SSLMode  string
	Type     string // "postgresql" or "mysql"
}

// Load loads configuration from environment variables
func Load() (*Config, error) {
	// Server configuration
	serverPort, err := strconv.Atoi(getEnv("SERVER_PORT", "8080"))
	if err != nil {
		serverPort = 8080
	}

	// Database configuration - use DB_* environment variables
	dbPort, err := strconv.Atoi(getEnv("DB_PORT", "5432"))
	if err != nil {
		dbPort = 5432
	}

	// Get database type - defaults to postgresql if not specified
	dbType := getEnv("DB_TYPE", "postgresql")

	// Set default port based on database type if not explicitly provided
	if os.Getenv("DB_PORT") == "" {
		if dbType == "mysql" {
			dbPort = 3306
		} else {
			dbPort = 5432
		}
	}

	// Set default user based on database type if not explicitly provided
	dbUser := getEnv("DB_USER", "")
	if dbUser == "" {
		if dbType == "mysql" {
			// Use root user for MySQL by default
			dbUser = "root"
		} else {
			// Use postgres user for PostgreSQL by default
			dbUser = "postgres"
		}
	}

	return &Config{
		Server: ServerConfig{
			Port: serverPort,
		},
		Database: DatabaseConfig{
			Host:     getEnv("DB_HOST", "localhost"),
			Port:     dbPort,
			User:     dbUser,
			Password: getEnv("DB_PASSWORD", "password"),
			DBName:   getEnv("DB_NAME", "pinepods_database"),
			SSLMode:  getEnv("DB_SSL_MODE", "disable"),
			Type:     dbType,
		},
		Environment: getEnv("ENVIRONMENT", "development"),
	}, nil
}

// getEnv gets an environment variable or returns a default value
func getEnv(key, defaultValue string) string {
	value, exists := os.LookupEnv(key)
	if !exists {
		return defaultValue
	}
	return value
}
