package utils

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"strings"
	"time"

	"github.com/mmcdole/gofeed"
)

// PodcastValues represents metadata extracted from a podcast feed
type PodcastValues struct {
	Title        string `json:"title"`
	ArtworkURL   string `json:"artwork_url"`
	Author       string `json:"author"`
	Categories   string `json:"categories"`
	Description  string `json:"description"`
	EpisodeCount int    `json:"episode_count"`
	FeedURL      string `json:"feed_url"`
	WebsiteURL   string `json:"website_url"`
	Explicit     bool   `json:"explicit"`
	UserID       int    `json:"user_id"`
}

// GetPodcastValues fetches and parses a podcast feed
func GetPodcastValues(feedURL string, userID int, username string, password string) (*PodcastValues, error) {
	log.Printf("[INFO] Fetching podcast data from feed: %s", feedURL)

	// Create a feed parser with custom configuration
	fp := gofeed.NewParser()

	// Set a reasonable timeout to prevent hanging
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	// Parse the feed
	feed, err := fp.ParseURLWithContext(feedURL, ctx)
	if err != nil {
		log.Printf("[ERROR] Failed to parse feed %s: %v", feedURL, err)

		// Return minimal data even when failing
		return &PodcastValues{
			Title:        feedURL,
			Description:  fmt.Sprintf("Podcast with feed: %s", feedURL),
			FeedURL:      feedURL,
			UserID:       userID,
			EpisodeCount: 0,
		}, err
	}

	// Initialize podcast values
	podcastValues := &PodcastValues{
		Title:        feed.Title,
		FeedURL:      feedURL,
		UserID:       userID,
		EpisodeCount: len(feed.Items),
	}

	// Extract basic data
	if feed.Description != "" {
		podcastValues.Description = feed.Description
	}

	if feed.Author != nil && feed.Author.Name != "" {
		podcastValues.Author = feed.Author.Name
	}

	if feed.Link != "" {
		podcastValues.WebsiteURL = feed.Link
	}

	// Extract artwork URL
	if feed.Image != nil && feed.Image.URL != "" {
		podcastValues.ArtworkURL = feed.Image.URL
	}

	// Process iTunes extensions if available
	extensions := feed.Extensions
	if extensions != nil {
		if itunesExt, ok := extensions["itunes"]; ok {
			// Check for iTunes author
			if itunesAuthor, exists := itunesExt["author"]; exists && len(itunesAuthor) > 0 {
				if podcastValues.Author == "" && itunesAuthor[0].Value != "" {
					podcastValues.Author = itunesAuthor[0].Value
				}
			}

			// Check for iTunes image
			if itunesImage, exists := itunesExt["image"]; exists && len(itunesImage) > 0 {
				if podcastValues.ArtworkURL == "" && itunesImage[0].Attrs["href"] != "" {
					podcastValues.ArtworkURL = itunesImage[0].Attrs["href"]
				}
			}

			// Check for explicit content
			if itunesExplicit, exists := itunesExt["explicit"]; exists && len(itunesExplicit) > 0 {
				explicitValue := strings.ToLower(itunesExplicit[0].Value)
				podcastValues.Explicit = explicitValue == "yes" || explicitValue == "true"
			}

			// Check for categories
			if itunesCategories, exists := itunesExt["category"]; exists && len(itunesCategories) > 0 {
				categories := make(map[string]string)

				for i, category := range itunesCategories {
					if category.Attrs["text"] != "" {
						categories[fmt.Sprintf("%d", i+1)] = category.Attrs["text"]

						// A simplified approach for subcategories
						// Many iTunes category extensions have nested category elements
						// directly within them as attributes
						if subCategoryText, hasSubCategory := category.Attrs["subcategory"]; hasSubCategory {
							categories[fmt.Sprintf("%d.1", i+1)] = subCategoryText
						}
					}
				}

				// Serialize categories to JSON string if we found any
				if len(categories) > 0 {
					categoriesJSON, err := json.Marshal(categories)
					if err == nil {
						podcastValues.Categories = string(categoriesJSON)
					} else {
						log.Printf("[WARNING] Failed to serialize categories: %v", err)
						podcastValues.Categories = "{}"
					}
				}
			}

			// Check for iTunes summary
			if itunesSummary, exists := itunesExt["summary"]; exists && len(itunesSummary) > 0 {
				if podcastValues.Description == "" && itunesSummary[0].Value != "" {
					podcastValues.Description = itunesSummary[0].Value
				}
			}
		}
	}

	// Fill in defaults for missing values
	if podcastValues.Title == "" {
		podcastValues.Title = feedURL
	}

	if podcastValues.Description == "" {
		podcastValues.Description = fmt.Sprintf("Podcast feed: %s", feedURL)
	}

	if podcastValues.Author == "" {
		podcastValues.Author = "Unknown Author"
	}

	if podcastValues.Categories == "" {
		podcastValues.Categories = "{}"
	}

	log.Printf("[INFO] Successfully parsed podcast feed: %s, title: %s, episodes: %d",
		feedURL, podcastValues.Title, podcastValues.EpisodeCount)

	return podcastValues, nil
}
