package db

import (
	"database/sql"
	"fmt"
	"log"
	"time"
)

// Migration represents a database migration
type Migration struct {
	Version     int
	Description string
	SQL         string
}

// MigrationRecord represents a record of an applied migration
type MigrationRecord struct {
	Version     int
	Description string
	AppliedAt   time.Time
}

// EnsureMigrationsTable creates the migrations table if it doesn't exist
func EnsureMigrationsTable(db *sql.DB) error {
	log.Println("Creating GpodderSyncMigrations table if it doesn't exist...")
	_, err := db.Exec(`
		CREATE TABLE IF NOT EXISTS "GpodderSyncMigrations" (
			Version INT PRIMARY KEY,
			Description TEXT NOT NULL,
			AppliedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
		)
	`)
	if err != nil {
		log.Printf("Error creating migrations table: %v", err)
		return err
	}
	log.Println("GpodderSyncMigrations table is ready")
	return nil
}

// GetAppliedMigrations returns a list of already applied migrations
func GetAppliedMigrations(db *sql.DB) ([]MigrationRecord, error) {
	log.Println("Checking previously applied migrations...")
	rows, err := db.Query(`
		SELECT Version, Description, AppliedAt
		FROM "GpodderSyncMigrations"
		ORDER BY Version ASC
	`)
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
func ApplyMigration(db *sql.DB, migration Migration) error {
	log.Printf("Applying migration %d: %s", migration.Version, migration.Description)

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
	_, err = tx.Exec(migration.SQL)
	if err != nil {
		log.Printf("Failed to apply migration %d: %v", migration.Version, err)
		return fmt.Errorf("failed to apply migration %d: %w", migration.Version, err)
	}

	// Record the migration
	_, err = tx.Exec(`
		INSERT INTO "GpodderSyncMigrations" (Version, Description)
		VALUES ($1, $2)
	`, migration.Version, migration.Description)
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
func RunMigrations(db *sql.DB) error {
	log.Println("Starting gpodder API migrations...")

	// Ensure migrations table exists
	if err := EnsureMigrationsTable(db); err != nil {
		return fmt.Errorf("failed to create migrations table: %w", err)
	}

	// Get applied migrations
	appliedMigrations, err := GetAppliedMigrations(db)
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
		if err := ApplyMigration(db, migration); err != nil {
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

// GetMigrations returns all migrations
func GetMigrations() []Migration {
	return []Migration{
		{
			Version:     1,
			Description: "Initial schema creation",
			SQL: `
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
		},
		// Add more migrations here as needed in the future
		{
			Version:     2,
			Description: "Add API version column to GpodderSyncSettings",
			SQL: `
				ALTER TABLE "GpodderSyncSettings"
				ADD COLUMN IF NOT EXISTS APIVersion VARCHAR(10) DEFAULT '2.0';
			`,
		},
		{
			Version:     3,
			Description: "Create GpodderSessions table for API sessions",
			SQL: `
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
		},
		{
			Version:     4,
			Description: "Add sync state table for tracking device sync status",
			SQL: `
				CREATE TABLE IF NOT EXISTS "GpodderSyncState" (
					SyncStateID SERIAL PRIMARY KEY,
					UserID INT NOT NULL,
					DeviceID INT NOT NULL,
					LastTimestamp BIGINT NOT NULL DEFAULT 0,
					LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
					FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
					UNIQUE(UserID, DeviceID)
				);

				CREATE INDEX IF NOT EXISTS idx_gpodder_syncstate_userid_deviceid ON "GpodderSyncState"(UserID, DeviceID);
			`,
		},
		// Example of a future migration (commented out for now)
		/*
			{
				Version:     3,
				Description: "Add support for episode chapters",
				SQL: `
					CREATE TABLE IF NOT EXISTS "GpodderSyncEpisodeChapters" (
						ChapterID SERIAL PRIMARY KEY,
						ActionID INT NOT NULL,
						ChapterTitle TEXT NOT NULL,
						StartTime INT NOT NULL,
						EndTime INT NOT NULL,
						FOREIGN KEY (ActionID) REFERENCES "GpodderSyncEpisodeActions"(ActionID) ON DELETE CASCADE
					);
				`,
			},
		*/
	}
}
