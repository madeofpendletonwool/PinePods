const CACHE_NAME = "podcast-image-cache-v1";

self.addEventListener("fetch", (event) => {
  if (
    event.request.url.match(/\.(jpg|jpeg|png|gif|webp)$/i) ||
    event.request.url.includes("imgix.net") ||
    event.request.url.includes("simplecastcdn.com")
  ) {
    event.respondWith(
      caches.open(CACHE_NAME).then((cache) => {
        return cache.match(event.request).then((response) => {
          const fetchPromise = fetch(event.request).then((networkResponse) => {
            cache.put(event.request, networkResponse.clone());
            return networkResponse;
          });
          return response || fetchPromise;
        });
      }),
    );
  }
});

// Clean up old caches
self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((cacheNames) => {
      return Promise.all(
        cacheNames.map((cacheName) => {
          if (cacheName !== CACHE_NAME) {
            return caches.delete(cacheName);
          }
        }),
      );
    }),
  );
});
