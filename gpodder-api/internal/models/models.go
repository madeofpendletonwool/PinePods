package models

import (
	"time"
)

// Device represents a user device
type Device struct {
	ID            int       `json:"-"`
	UserID        int       `json:"-"`
	DeviceID      string    `json:"id"`
	Caption       string    `json:"caption"`
	Type          string    `json:"type"`
	Subscriptions int       `json:"subscriptions"`
	CreatedAt     time.Time `json:"-"`
	LastUpdated   time.Time `json:"-"`
}

// GpodderDevice represents a device from the GpodderDevices table
type GpodderDevice struct {
	DeviceID      int       `json:"-"`
	UserID        int       `json:"-"`
	DeviceName    string    `json:"id"`
	DeviceType    string    `json:"type"`
	DeviceCaption string    `json:"caption"`
	IsDefault     bool      `json:"-"`
	LastSync      time.Time `json:"-"`
	IsActive      bool      `json:"-"`
	// Additional field for API responses
	Subscriptions int `json:"subscriptions"`
}

// Subscription represents a podcast subscription
type Subscription struct {
	SubscriptionID int    `json:"-"`
	UserID         int    `json:"-"`
	DeviceID       int    `json:"-"`
	PodcastURL     string `json:"url"`
	Action         string `json:"-"`
	Timestamp      int64  `json:"-"`
}

// SubscriptionChange represents a change to subscriptions
type SubscriptionChange struct {
	Add    []string `json:"add"`
	Remove []string `json:"remove"`
}

// SubscriptionResponse represents a response to subscription change request
type SubscriptionResponse struct {
	Add        []string   `json:"add"`
	Remove     []string   `json:"remove"`
	Timestamp  int64      `json:"timestamp"`
	UpdateURLs [][]string `json:"update_urls"` // Removed omitempty to ensure field is always present
}

// EpisodeAction represents an action performed on an episode
// First, create a struct for the JSON request format
type EpisodeActionRequest struct {
	Actions []EpisodeAction `json:"actions"`
}

// Then modify the EpisodeAction struct to use a flexible type for timestamp
type EpisodeAction struct {
	ActionID  int         `json:"-"`
	UserID    int         `json:"-"`
	DeviceID  int         `json:"-"`
	Podcast   string      `json:"podcast"`
	Episode   string      `json:"episode"`
	Device    string      `json:"device,omitempty"`
	Action    string      `json:"action"`
	Timestamp interface{} `json:"timestamp"` // Accept any type
	Started   *int        `json:"started,omitempty"`
	Position  *int        `json:"position,omitempty"`
	Total     *int        `json:"total,omitempty"`
}

// EpisodeActionResponse represents a response to episode action upload
type EpisodeActionResponse struct {
	Timestamp  int64      `json:"timestamp"`
	UpdateURLs [][]string `json:"update_urls"` // Removed omitempty
}

// EpisodeActionsResponse represents a response for episode actions retrieval
type EpisodeActionsResponse struct {
	Actions   []EpisodeAction `json:"actions"`
	Timestamp int64           `json:"timestamp"`
}

// PodcastList represents a user's podcast list
type PodcastList struct {
	ListID    int       `json:"-"`
	UserID    int       `json:"-"`
	Name      string    `json:"name"`
	Title     string    `json:"title"`
	CreatedAt time.Time `json:"-"`
	WebURL    string    `json:"web"`
	Podcasts  []Podcast `json:"-"`
}

// Podcast represents a podcast
type Podcast struct {
	URL           string `json:"url"`
	Title         string `json:"title,omitempty"`
	Description   string `json:"description,omitempty"`
	Website       string `json:"website,omitempty"`
	Subscribers   int    `json:"subscribers,omitempty"`
	LogoURL       string `json:"logo_url,omitempty"`
	ScaledLogoURL string `json:"scaled_logo_url,omitempty"`
	Author        string `json:"author,omitempty"`
	MygpoLink     string `json:"mygpo_link,omitempty"`
}

// Episode represents a podcast episode
type Episode struct {
	Title        string `json:"title"`
	URL          string `json:"url"`
	PodcastTitle string `json:"podcast_title"`
	PodcastURL   string `json:"podcast_url"`
	Description  string `json:"description"`
	Website      string `json:"website"`
	Released     string `json:"released"` // ISO 8601 format
	MygpoLink    string `json:"mygpo_link"`
}

// Setting represents a user setting
type Setting struct {
	SettingID    int       `json:"-"`
	UserID       int       `json:"-"`
	Scope        string    `json:"-"`
	DeviceID     int       `json:"-"`
	PodcastURL   string    `json:"-"`
	EpisodeURL   string    `json:"-"`
	SettingKey   string    `json:"-"`
	SettingValue string    `json:"-"`
	CreatedAt    time.Time `json:"-"`
	LastUpdated  time.Time `json:"-"`
}

// SettingsRequest represents a settings update request
type SettingsRequest struct {
	Set    map[string]interface{} `json:"set"`
	Remove []string               `json:"remove"`
}

// Tag represents a tag
type Tag struct {
	Title string `json:"title"`
	Tag   string `json:"tag"`
	Usage int    `json:"usage"`
}

// SyncDevicesResponse represents the sync status response
type SyncDevicesResponse struct {
	Synchronized    [][]string `json:"synchronized"`
	NotSynchronized []string   `json:"not-synchronized"`
}

// SyncDevicesRequest represents a sync status update request
type SyncDevicesRequest struct {
	Synchronize     [][]string `json:"synchronize"`
	StopSynchronize []string   `json:"stop-synchronize"`
}

// DeviceUpdateResponse represents a response to device updates request
type DeviceUpdateResponse struct {
	Add       []Podcast `json:"add"`
	Remove    []string  `json:"remove"`
	Updates   []Episode `json:"updates"`
	Timestamp int64     `json:"timestamp"`
}
