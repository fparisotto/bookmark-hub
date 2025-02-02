# bookmark-hub

Manage and store your bookmarks offline.

## How to run

```bash
$ just build-container
$ docker compose up
```

## E2E tests:

With the `docker-compose.yml` running, use [hurl](https://hurl.dev/)

```bash
$ hurl --verbose --test test.hurl
```
