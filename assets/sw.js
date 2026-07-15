var cacheName = 'jonotune-v1';
var filesToCache = [
  './',
  './index.html',
  './jonotune.js',
  './jonotune_bg.wasm',
];

/* Start the service worker and cache all of the app's content */
self.addEventListener('install', function (e) {
  e.waitUntil(
    caches.open(cacheName).then(function (cache) {
      return cache.addAll(filesToCache);
    })
  );
});

/* Activate: clear old cache versions */
self.addEventListener('activate', function (e) {
  e.waitUntil(
    caches.keys().then(function (keys) {
      return Promise.all(
        keys
          .filter(function (key) { return key !== cacheName; })
          .map(function (key) { return caches.delete(key); })
      );
    })
  );
});

/* Serve cached content when offline, fetch fresh when online */
self.addEventListener('fetch', function (e) {
  e.respondWith(
    caches.match(e.request).then(function (response) {
      return response || fetch(e.request);
    })
  );
});
