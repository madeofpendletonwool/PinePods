package api

import (
	"log"
	"pinepods/gpodder-api/internal/db"

	"github.com/gin-gonic/gin"
)

// Add or update in routes.go to ensure the Episode API routes are registered:

// RegisterRoutes registers all API routes
func RegisterRoutes(router *gin.RouterGroup, database *db.PostgresDB) {
	// Authentication endpoints
	log.Println("[INFO] Registering API routes...")
	authGroup := router.Group("/auth/:username")
	{
		authGroup.POST("/login.json", handleLogin(database))
		authGroup.POST("/logout.json", handleLogout(database))
	}

	// Device API
	log.Println("[INFO] Registering device routes")
	router.GET("/devices/:username.json", AuthenticationMiddleware(database), listDevices(database))
	router.POST("/devices/:username/:deviceid", AuthenticationMiddleware(database), updateDeviceData(database))
	router.GET("/updates/:username/:deviceid", AuthenticationMiddleware(database), getDeviceUpdates(database))

	// Subscriptions API
	subscriptionsGroup := router.Group("/subscriptions/:username")
	subscriptionsGroup.Use(AuthenticationMiddleware(database))
	{
		subscriptionsGroup.GET("/:deviceid", getSubscriptions(database))
		subscriptionsGroup.PUT("/:deviceid", updateSubscriptions(database))
		subscriptionsGroup.POST("/:deviceid", uploadSubscriptionChanges(database))
		// All subscriptions endpoint (since 2.11)
		subscriptionsGroup.GET(".json", getAllSubscriptions(database))
	}

	// Episode Actions API - FIXED ROUTE PATTERN
	log.Println("[INFO] Registering episode actions routes")
	// Register directly on the router without a group
	router.GET("/episodes/:username.json", AuthenticationMiddleware(database), getEpisodeActions(database))
	router.POST("/episodes/:username.json", AuthenticationMiddleware(database), uploadEpisodeActions(database))

	// Settings API
	settingsGroup := router.Group("/settings/:username")
	settingsGroup.Use(AuthenticationMiddleware(database))
	{
		settingsGroup.GET("/:scope.json", getSettings(database))
		settingsGroup.POST("/:scope.json", saveSettings(database))
	}

	// Podcast Lists API
	listsGroup := router.Group("/lists/:username")
	{
		listsGroup.GET(".json", getUserLists(database))
		listsGroup.POST("/create", AuthenticationMiddleware(database), createPodcastList(database))
		listGroup := listsGroup.Group("/list/:listname")
		{
			listGroup.GET("", getPodcastList(database))
			listGroup.PUT("", AuthenticationMiddleware(database), updatePodcastList(database))
			listGroup.DELETE("", AuthenticationMiddleware(database), deletePodcastList(database))
		}
	}

	// Favorite Episodes API
	router.GET("/favorites/:username.json", AuthenticationMiddleware(database), getFavoriteEpisodes(database))

	// Device Synchronization API
	syncGroup := router.Group("/sync-devices/:username")
	syncGroup.Use(AuthenticationMiddleware(database))
	{
		syncGroup.GET(".json", getSyncStatus(database))
		syncGroup.POST(".json", updateSyncStatus(database))
	}

	// Directory API (no auth required)
	router.GET("/tags/:count.json", getTopTags(database))
	router.GET("/tag/:tag/:count.json", getPodcastsForTag(database))
	router.GET("/data/podcast.json", getPodcastData(database))
	router.GET("/data/episode.json", getEpisodeData(database))

	// Suggestions API (auth required)
	router.GET("/suggestions/:count", AuthenticationMiddleware(database), getSuggestions(database))
}

// RegisterSimpleRoutes registers routes for the Simple API (v1)
func RegisterSimpleRoutes(router *gin.RouterGroup, database *db.PostgresDB) {
	// Toplist
	router.GET("/toplist/:number", getToplist(database))

	// Search
	router.GET("/search", podcastSearch(database))

	// Subscriptions (Simple API)
	router.GET("/subscriptions/:username/:deviceid", AuthenticationMiddleware(database), getSubscriptionsSimple(database))
	router.PUT("/subscriptions/:username/:deviceid", AuthenticationMiddleware(database), updateSubscriptionsSimple(database))
}
