package db

import (
	"fmt"
	"strings"
)

// GetTableName returns the properly formatted table name based on DB type
func GetTableName(tableName string, dbType string) string {
	if dbType == "postgresql" {
		return fmt.Sprintf("\"%s\"", tableName)
	}
	return tableName
}

// GetPlaceholder returns the correct parameter placeholder based on DB type and index
func GetPlaceholder(index int, dbType string) string {
	if dbType == "postgresql" {
		return fmt.Sprintf("$%d", index)
	}
	return "?"
}

// GetPlaceholders returns a comma-separated list of placeholders
func GetPlaceholders(count int, dbType string) string {
	placeholders := make([]string, count)

	for i := 0; i < count; i++ {
		if dbType == "postgresql" {
			placeholders[i] = fmt.Sprintf("$%d", i+1)
		} else {
			placeholders[i] = "?"
		}
	}

	return strings.Join(placeholders, ", ")
}

// GetColumnDefinition returns the appropriate column definition
func GetColumnDefinition(columnName, dataType string, dbType string) string {
	// Handle special cases for different database types
	switch dataType {
	case "serial":
		if dbType == "postgresql" {
			return fmt.Sprintf("%s SERIAL", columnName)
		}
		return fmt.Sprintf("%s INT AUTO_INCREMENT", columnName)
	case "boolean":
		if dbType == "postgresql" {
			return fmt.Sprintf("%s BOOLEAN", columnName)
		}
		return fmt.Sprintf("%s TINYINT(1)", columnName)
	case "timestamp":
		if dbType == "postgresql" {
			return fmt.Sprintf("%s TIMESTAMP", columnName)
		}
		return fmt.Sprintf("%s TIMESTAMP", columnName)
	default:
		return fmt.Sprintf("%s %s", columnName, dataType)
	}
}

// GetSerialPrimaryKey returns a serial primary key definition
func GetSerialPrimaryKey(columnName string, dbType string) string {
	if dbType == "postgresql" {
		return fmt.Sprintf("%s SERIAL PRIMARY KEY", columnName)
	}
	return fmt.Sprintf("%s INT AUTO_INCREMENT PRIMARY KEY", columnName)
}

// GetTimestampDefault returns a timestamp with default value
func GetTimestampDefault(columnName string, dbType string) string {
	if dbType == "postgresql" {
		return fmt.Sprintf("%s TIMESTAMP DEFAULT CURRENT_TIMESTAMP", columnName)
	}
	return fmt.Sprintf("%s TIMESTAMP DEFAULT CURRENT_TIMESTAMP", columnName)
}

// GetAutoUpdateTimestamp returns a timestamp that updates automatically
func GetAutoUpdateTimestamp(columnName string, dbType string) string {
	if dbType == "postgresql" {
		// PostgreSQL doesn't have a direct equivalent to MySQL's ON UPDATE
		// In PostgreSQL this would typically be handled with a trigger
		return fmt.Sprintf("%s TIMESTAMP DEFAULT CURRENT_TIMESTAMP", columnName)
	}
	return fmt.Sprintf("%s TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP", columnName)
}

// BuildInsertQuery builds an INSERT query with the correct placeholder syntax
func BuildInsertQuery(tableName string, columns []string, dbType string) string {
	columnsStr := strings.Join(columns, ", ")
	placeholders := GetPlaceholders(len(columns), dbType)

	if dbType == "postgresql" {
		return fmt.Sprintf("INSERT INTO \"%s\" (%s) VALUES (%s)", tableName, columnsStr, placeholders)
	}

	return fmt.Sprintf("INSERT INTO %s (%s) VALUES (%s)", tableName, columnsStr, placeholders)
}

// BuildSelectQuery builds a SELECT query with the correct table name syntax
func BuildSelectQuery(tableName string, columns []string, whereClause string, dbType string) string {
	columnsStr := strings.Join(columns, ", ")

	if dbType == "postgresql" {
		if whereClause != "" {
			return fmt.Sprintf("SELECT %s FROM \"%s\" WHERE %s", columnsStr, tableName, whereClause)
		}
		return fmt.Sprintf("SELECT %s FROM \"%s\"", columnsStr, tableName)
	}

	if whereClause != "" {
		return fmt.Sprintf("SELECT %s FROM %s WHERE %s", columnsStr, tableName, whereClause)
	}
	return fmt.Sprintf("SELECT %s FROM %s", columnsStr, tableName)
}

// BuildUpdateQuery builds an UPDATE query with the correct syntax
func BuildUpdateQuery(tableName string, setColumns []string, whereClause string, dbType string) string {
	setClauses := make([]string, len(setColumns))

	for i, col := range setColumns {
		if dbType == "postgresql" {
			setClauses[i] = fmt.Sprintf("%s = $%d", col, i+1)
		} else {
			setClauses[i] = fmt.Sprintf("%s = ?", col)
		}
	}

	setClauseStr := strings.Join(setClauses, ", ")

	if dbType == "postgresql" {
		return fmt.Sprintf("UPDATE \"%s\" SET %s WHERE %s", tableName, setClauseStr, whereClause)
	}

	return fmt.Sprintf("UPDATE %s SET %s WHERE %s", tableName, setClauseStr, whereClause)
}

// RewriteQuery rewrites a PostgreSQL query to MySQL syntax
func RewriteQuery(query, dbType string) string {
	if dbType == "postgresql" {
		return query
	}

	// Replace placeholders
	rewritten := query

	// Replace placeholders first, starting from highest number to avoid conflicts
	for i := 20; i > 0; i-- {
		placeholder := fmt.Sprintf("$%d", i)
		rewritten = strings.ReplaceAll(rewritten, placeholder, "?")
	}

	// Replace quoted table names
	knownTables := []string{
		"Users", "GpodderDevices", "GpodderSyncSettings",
		"GpodderSyncSubscriptions", "GpodderSyncEpisodeActions",
		"GpodderSyncPodcastLists", "GpodderSyncState", "GpodderSessions",
		"GpodderSyncMigrations", "Podcasts", "Episodes", "SavedEpisodes",
		"UserEpisodeHistory", "UserSettings", "APIKeys", "UserVideoHistory",
		"SavedVideos", "DownloadedEpisodes", "DownloadedVideos", "EpisodeQueue",
	}

	for _, table := range knownTables {
		quotedTable := fmt.Sprintf("\"%s\"", table)
		rewritten = strings.ReplaceAll(rewritten, quotedTable, table)
	}

	// Handle RETURNING clause (MySQL doesn't support it)
	returningIdx := strings.Index(strings.ToUpper(rewritten), "RETURNING")
	if returningIdx > 0 {
		rewritten = rewritten[:returningIdx]
	}

	return rewritten
}
