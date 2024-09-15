# bookmark-hub

Manage and store your bookmarks offline.

## System design

- `backend` allows users to create an account, save bookmarks, and download them for offline consumption.
- `web-spa` [yew](https://yew.rs/) front-end application.
- `readability-api` exposes the [readability](https://github.com/mozilla/readability) as HTTP API service, used by `backend` to clean up the HTML content.
- [postgresql](https://www.postgresql.org/) as application database.

## How to run

```bash
$ docker compose down --volumes && docker compose build && docker compose up
```

## E2E tests:

With the `docker-compose.yml` running, use [hurl](https://hurl.dev/)

```bash
$ hurl --verbose --test test.hurl
```
