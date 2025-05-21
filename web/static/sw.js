const CACHE_NAME = "podcast-image-cache-v1";

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);

  // Check if it's an image request
  if (
    url.pathname.match(/\.(jpg|jpeg|png|gif|webp)$/i) ||
    url.hostname.includes("imgix.net") ||
    url.hostname.includes("simplecastcdn.com") ||
    url.hostname.includes("npr.org") // Add this for your NPR images
  ) {
    event.respondWith(
      caches.open(CACHE_NAME).then((cache) => {
        // Create a request without query parameters for cache matching
        const cacheUrl = url.origin + url.pathname;
        const cacheRequest = new Request(cacheUrl);

        return cache.match(cacheRequest).then((response) => {
          if (response) {
            console.log("Cache hit for:", cacheUrl);
            return response;
          }

          // If not in cache, fetch and store
          return fetch(event.request).then((networkResponse) => {
            // Store the response without query parameters
            cache.put(cacheRequest, networkResponse.clone());
            console.log("Cached image:", cacheUrl);
            return networkResponse;
          });
        });
      }),
    );
  }
});

// Enhanced cache cleanup and management
self.addEventListener("activate", (event) => {
  event.waitUntil(
    Promise.all([
      // Clear old caches
      caches.keys().then((cacheNames) => {
        return Promise.all(
          cacheNames.map((cacheName) => {
            if (cacheName !== CACHE_NAME) {
              console.log("Deleting old cache:", cacheName);
              return caches.delete(cacheName);
            }
          }),
        );
      }),
      // Take control of all clients immediately
      self.clients.claim(),
    ]),
  );
});

// Add install event to cache important images immediately
self.addEventListener("install", (event) => {
  self.skipWaiting();
});
