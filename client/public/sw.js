// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

const CACHE_NAME = 'stackarr-shell-v1';
const SHELL_URLS = [
  '/app/',
  '/app/index.html',
];

// Install: cache app shell
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(SHELL_URLS))
  );
  self.skipWaiting();
});

// Activate: clean old caches
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    )
  );
  self.clients.claim();
});

// Fetch: network-first for API, cache-first for shell assets
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // API requests: network-first
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(
      fetch(event.request).catch(() => caches.match(event.request))
    );
    return;
  }

  // App shell: cache-first, fallback to network
  event.respondWith(
    caches.match(event.request).then((cached) => {
      if (cached) return cached;
      return fetch(event.request).then((response) => {
        // Cache JS/CSS assets on the fly
        if (response.ok && (url.pathname.endsWith('.js') || url.pathname.endsWith('.css'))) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
        }
        return response;
      });
    })
  );
});

// Push notification handler
self.addEventListener('push', (event) => {
  let data = { title: 'StackArr', body: 'New notification' };
  try {
    if (event.data) {
      data = event.data.json();
    }
  } catch {
    if (event.data) {
      data = { title: 'StackArr', body: event.data.text() };
    }
  }

  event.waitUntil(
    self.registration.showNotification(data.title || 'StackArr', {
      body: data.body || '',
      icon: '/app/icons/icon-192.png',
      badge: '/app/icons/icon-96.png',
      data: data.data || {},
      tag: data.tag || 'stackarr-notification',
    })
  );
});

// Notification click: open or focus the app
self.addEventListener('notificationclick', (event) => {
  event.notification.close();

  const urlToOpen = '/app/';

  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((clients) => {
      for (const client of clients) {
        if (client.url.includes('/app') && 'focus' in client) {
          return client.focus();
        }
      }
      return self.clients.openWindow(urlToOpen);
    })
  );
});
