package db

import (
	"database/sql"
	"fmt"
	"log"
	"time"
)

// Migration represents a database migration
type Migration struct {
	Version       int
	Description   string
	PostgreSQLSQL string
	MySQLSQL      string
}

// MigrationRecord represents a record of an applied migration
type MigrationRecord struct {
	Version     int
	Description string
	AppliedAt   time.Time
}

// EnsureMigrationsTable creates the migrations table if it doesn't exist
func EnsureMigrationsTable(db *sql.DB, dbType string) error {
	log.Println("Creating GpodderSyncMigrations table if it doesn't exist...")

	var query string
	if dbType == "postgresql" {
		query = `
			CREATE TABLE IF NOT EXISTS "GpodderSyncMigrations" (
				Version INT PRIMARY KEY,
				Description TEXT NOT NULL,
				AppliedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
			)
		`
	} else {
		query = `
			CREATE TABLE IF NOT EXISTS GpodderSyncMigrations (
				Version INT PRIMARY KEY,
				Description TEXT NOT NULL,
				AppliedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
			)
		`
	}

	_, err := db.Exec(query)
	if err != nil {
		log.Printf("Error creating migrations table: %v", err)
		return err
	}
	log.Println("GpodderSyncMigrations table is ready")
	return nil
}

// GetAppliedMigrations returns a list of already applied migrations
func GetAppliedMigrations(db *sql.DB, dbType string) ([]MigrationRecord, error) {
	log.Println("Checking previously applied migrations...")

	var query string
	if dbType == "postgresql" {
		query = `
			SELECT Version, Description, AppliedAt
			FROM "GpodderSyncMigrations"
			ORDER BY Version ASC
		`
	} else {
		query = `
			SELECT Version, Description, AppliedAt
			FROM GpodderSyncMigrations
			ORDER BY Version ASC
		`
	}

	rows, err := db.Query(query)
	if err != nil {
		log.Printf("Error checking applied migrations: %v", err)
		return nil, err
	}
	defer rows.Close()

	var migrations []MigrationRecord
	for rows.Next() {
		var m MigrationRecord
		if err := rows.Scan(&m.Version, &m.Description, &m.AppliedAt); err != nil {
			log.Printf("Error scanning migration record: %v", err)
			return nil, err
		}
		migrations = append(migrations, m)
	}

	if len(migrations) > 0 {
		log.Printf("Found %d previously applied migrations", len(migrations))
	} else {
		log.Println("No previously applied migrations found")
	}
	return migrations, nil
}

// ApplyMigration applies a single migration
func ApplyMigration(db *sql.DB, migration Migration, dbType string) error {
	log.Printf("Applying migration %d: %s", migration.Version, migration.Description)

	// Select the appropriate SQL based on database type
	var sql string
	if dbType == "postgresql" {
		sql = migration.PostgreSQLSQL
	} else {
		sql = migration.MySQLSQL
	}

	// Begin transaction
	tx, err := db.Begin()
	if err != nil {
		log.Printf("Error beginning transaction for migration %d: %v", migration.Version, err)
		return err
	}
	defer func() {
		if err != nil {
			log.Printf("Rolling back migration %d due to error", migration.Version)
			tx.Rollback()
			return
		}
	}()

	// Execute the migration SQL
	_, err = tx.Exec(sql)
	if err != nil {
		log.Printf("Failed to apply migration %d: %v", migration.Version, err)
		return fmt.Errorf("failed to apply migration %d: %w", migration.Version, err)
	}

	// Record the migration
	var insertQuery string
	if dbType == "postgresql" {
		insertQuery = `
			INSERT INTO "GpodderSyncMigrations" (Version, Description)
			VALUES ($1, $2)
		`
	} else {
		insertQuery = `
			INSERT INTO GpodderSyncMigrations (Version, Description)
			VALUES (?, ?)
		`
	}

	_, err = tx.Exec(insertQuery, migration.Version, migration.Description)
	if err != nil {
		log.Printf("Failed to record migration %d: %v", migration.Version, err)
		return fmt.Errorf("failed to record migration %d: %w", migration.Version, err)
	}

	// Commit the transaction
	err = tx.Commit()
	if err != nil {
		log.Printf("Failed to commit migration %d: %v", migration.Version, err)
		return err
	}

	log.Printf("Successfully applied migration %d", migration.Version)
	return nil
}

// RunMigrations runs all pending migrations
func RunMigrations(db *sql.DB, dbType string) error {
	log.Println("Starting gpodder API migrations...")

	// Ensure migrations table exists
	if err := EnsureMigrationsTable(db, dbType); err != nil {
		return fmt.Errorf("failed to create migrations table: %w", err)
	}

	// Get applied migrations
	appliedMigrations, err := GetAppliedMigrations(db, dbType)
	if err != nil {
		return fmt.Errorf("failed to get applied migrations: %w", err)
	}

	// Build a map of applied migration versions for quick lookup
	appliedVersions := make(map[int]bool)
	for _, m := range appliedMigrations {
		appliedVersions[m.Version] = true
	}

	// Get all migrations
	migrations := GetMigrations()
	log.Printf("Found %d total migrations to check", len(migrations))

	// Apply pending migrations
	appliedCount := 0
	for _, migration := range migrations {
		if appliedVersions[migration.Version] {
			// Migration already applied, skip
			log.Printf("Migration %d already applied, skipping", migration.Version)
			continue
		}

		log.Printf("Applying migration %d: %s", migration.Version, migration.Description)
		if err := ApplyMigration(db, migration, dbType); err != nil {
			return err
		}
		appliedCount++
	}

	if appliedCount > 0 {
		log.Printf("Successfully applied %d new migrations", appliedCount)
	} else {
		log.Println("No new migrations to apply")
	}

	return nil
}

// GetMigrations returns all migrations with SQL variants for both database types
func GetMigrations() []Migration {
	return []Migration{
		{
			Version:     1,
			Description: "Initial schema creation",
			PostgreSQLSQL: `
				-- Device sync state for the API
				CREATE TABLE IF NOT EXISTS "GpodderSyncDeviceState" (
					DeviceStateID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					SubscriptionCount INT DEFAULT 0,
					LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID)
				);

				-- Subscription changes
				CREATE TABLE IF NOT EXISTS "GpodderSyncSubscriptions" (
					SubscriptionID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					PodcastURL TEXT NOT NULL,
					Action VARCHAR(10) NOT NULL,
					Timestamp BIGINT NOT NULL,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE
				);

				-- Episode actions
				CREATE TABLE IF NOT EXISTS "GpodderSyncEpisodeActions" (
					ActionID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT,
					PodcastURL TEXT NOT NULL,
					EpisodeURL TEXT NOT NULL,
					Action VARCHAR(20) NOT NULL,
					Timestamp BIGINT NOT NULL,
					Started INT,
					Position INT,
					Total INT,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE
				);

				-- Podcast lists
				CREATE TABLE IF NOT EXISTS "GpodderSyncPodcastLists" (
					ListID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					Name VARCHAR(255) NOT NULL,
					Title VARCHAR(255) NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					UNIQUE(UserID, Name)
				);

				-- Podcast list entries
				CREATE TABLE IF NOT EXISTS "GpodderSyncPodcastListEntries" (
					EntryID SERIAL PRIMARY KEY,
					ListID INT NOT NULL,
					PodcastURL TEXT NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (ListID) REFERENCES "GpodderSyncPodcastLists"(ListID) ON DELETE CASCADE
				);

				-- Synchronization relationships between devices
				CREATE TABLE IF NOT EXISTS "GpodderSyncDevicePairs" (
					PairID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID1 INT NOT NULL,
					DeviceID2 INT NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID1) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID2) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID1, DeviceID2)
				);

				-- Settings storage
				CREATE TABLE IF NOT EXISTS "GpodderSyncSettings" (
					SettingID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					Scope VARCHAR(20) NOT NULL,
					DeviceID INT,
					PodcastURL TEXT,
					EpisodeURL TEXT,
					SettingKey VARCHAR(255) NOT NULL,
					SettingValue TEXT,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE
				);

				-- Create indexes for faster queries
				CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subscriptions_userid ON "GpodderSyncSubscriptions"(UserID);
				CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subscriptions_deviceid ON "GpodderSyncSubscriptions"(DeviceID);
				CREATE INDEX IF NOT EXISTS idx_gpodder_sync_episode_actions_userid ON "GpodderSyncEpisodeActions"(UserID);
				CREATE INDEX IF NOT EXISTS idx_gpodder_sync_podcast_lists_userid ON "GpodderSyncPodcastLists"(UserID);
			`,
			MySQLSQL: `
				-- Device sync state for the API
				CREATE TABLE IF NOT EXISTS GpodderSyncDeviceState (
					DeviceStateID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					SubscriptionCount INT DEFAULT 0,
					LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID)
				);

				-- Subscription changes
				CREATE TABLE IF NOT EXISTS GpodderSyncSubscriptions (
					SubscriptionID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					PodcastURL TEXT NOT NULL,
					Action VARCHAR(10) NOT NULL,
					Timestamp BIGINT NOT NULL,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
				);

				-- Episode actions
				CREATE TABLE IF NOT EXISTS GpodderSyncEpisodeActions (
					ActionID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT,
					PodcastURL TEXT NOT NULL,
					EpisodeURL TEXT NOT NULL,
					Action VARCHAR(20) NOT NULL,
					Timestamp BIGINT NOT NULL,
					Started INT,
					Position INT,
					Total INT,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
				);

				-- Podcast lists
				CREATE TABLE IF NOT EXISTS GpodderSyncPodcastLists (
					ListID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					Name VARCHAR(255) NOT NULL,
					Title VARCHAR(255) NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					UNIQUE(UserID, Name)
				);

				-- Podcast list entries
				CREATE TABLE IF NOT EXISTS GpodderSyncPodcastListEntries (
					EntryID INT AUTO_INCREMENT PRIMARY KEY,
					ListID INT NOT NULL,
					PodcastURL TEXT NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (ListID) REFERENCES GpodderSyncPodcastLists(ListID) ON DELETE CASCADE
				);

				-- Synchronization relationships between devices
				CREATE TABLE IF NOT EXISTS GpodderSyncDevicePairs (
					PairID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID1 INT NOT NULL,
					DeviceID2 INT NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID1) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID2) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID1, DeviceID2)
				);

				-- Settings storage
				CREATE TABLE IF NOT EXISTS GpodderSyncSettings (
					SettingID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					Scope VARCHAR(20) NOT NULL,
					DeviceID INT,
					PodcastURL TEXT,
					EpisodeURL TEXT,
					SettingKey VARCHAR(255) NOT NULL,
					SettingValue TEXT,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
				);

				-- Create indexes for faster queries
				CREATE INDEX idx_gpodder_sync_subscriptions_userid ON GpodderSyncSubscriptions(UserID);
				CREATE INDEX idx_gpodder_sync_subscriptions_deviceid ON GpodderSyncSubscriptions(DeviceID);
				CREATE INDEX idx_gpodder_sync_episode_actions_userid ON GpodderSyncEpisodeActions(UserID);
				CREATE INDEX idx_gpodder_sync_podcast_lists_userid ON GpodderSyncPodcastLists(UserID);
			`,
		},
		{
			Version:     2,
			Description: "Add API version column to GpodderSyncSettings",
			PostgreSQLSQL: `
		        ALTER TABLE "GpodderSyncSettings"
		        ADD COLUMN IF NOT EXISTS APIVersion VARCHAR(10) DEFAULT '2.0';
		    `,
			MySQLSQL: `
		        -- Check if column exists first
		        SET @s = (SELECT IF(
		            COUNT(*) = 0,
		            'ALTER TABLE GpodderSyncSettings ADD COLUMN APIVersion VARCHAR(10) DEFAULT "2.0"',
		            'SELECT 1'
		        ) FROM INFORMATION_SCHEMA.COLUMNS
		        WHERE TABLE_NAME = 'GpodderSyncSettings'
		        AND COLUMN_NAME = 'APIVersion');

		        PREPARE stmt FROM @s;
		        EXECUTE stmt;
		        DEALLOCATE PREPARE stmt;
		    `,
		},
		{
			Version:     3,
			Description: "Create GpodderSessions table for API sessions",
			PostgreSQLSQL: `
				CREATE TABLE IF NOT EXISTS "GpodderSessions" (
					SessionID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					SessionToken TEXT NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					ExpiresAt TIMESTAMP NOT NULL,
					LastActive TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					UserAgent TEXT,
					ClientIP TEXT,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					UNIQUE(SessionToken)
				);

				CREATE INDEX IF NOT EXISTS idx_gpodder_sessions_token ON "GpodderSessions"(SessionToken);
				CREATE INDEX IF NOT EXISTS idx_gpodder_sessions_userid ON "GpodderSessions"(UserID);
				CREATE INDEX IF NOT EXISTS idx_gpodder_sessions_expires ON "GpodderSessions"(ExpiresAt);
			`,
			MySQLSQL: `
				CREATE TABLE IF NOT EXISTS GpodderSessions (
					SessionID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					SessionToken TEXT NOT NULL,
					CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					ExpiresAt TIMESTAMP NOT NULL,
					LastActive TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					UserAgent TEXT,
					ClientIP TEXT,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
				);

				CREATE INDEX idx_gpodder_sessions_userid ON GpodderSessions(UserID);
				CREATE INDEX idx_gpodder_sessions_expires ON GpodderSessions(ExpiresAt);
			`,
		},
		{
			Version:     4,
			Description: "Add sync state table for tracking device sync status",
			PostgreSQLSQL: `
				CREATE TABLE IF NOT EXISTS "GpodderSyncState" (
					SyncStateID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					LastTimestamp BIGINT DEFAULT 0,
					LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID)
				);

				CREATE INDEX IF NOT EXISTS idx_gpodder_syncstate_userid_deviceid ON "GpodderSyncState"(UserID, DeviceID);
			`,
			MySQLSQL: `
				CREATE TABLE IF NOT EXISTS GpodderSyncState (
					SyncStateID INT AUTO_INCREMENT PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					LastTimestamp BIGINT DEFAULT 0,
					LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID)
				);

				CREATE INDEX idx_gpodder_syncstate_userid_deviceid ON GpodderSyncState(UserID, DeviceID);
			`,
		},
	}
}
