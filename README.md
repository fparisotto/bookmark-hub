# bookmark-rs

Manage and store your bookmarks offline.

## System design

- `backend`
  - exposes the functionality of adding/retrieving bookmarks for the user.
  -  process asynchronously new bookmarks requests, downloading its HTML content and images.
- `readability-api`
  - exposes the [readability](https://github.com/mozilla/readability) as HTTP API service, used by `backend` to clean up the HTML content.
- `web-spa`
  - is a [yew](https://yew.rs/) front-end application.
- [minio](https://min.io/) used as static content storage.
- [postgresql](https://www.postgresql.org/) application database.

## How to run

```bash
$ docker compose down --volumes && docker compose build && docker compose up
```

## E2E tests:

With the `docker-compose.yml` running, use [hurl](https://hurl.dev/)

```bash
$ hurl --verbose test.hurl
```
