# bookmark-rs

Manage and store your bookmarks offline.

_This is a toy project of full-stack [Rust](https://www.rust-lang.org/)
application, built for learning propuses, not intended to be used in
production._

## Brief explanation of system design

- `public-api` exposes the functionality of adding/retrieving bookmarks for the
  user.
- `daemon` process asynchronously new bookmarks requests, downloading its HTML
  content and images.
- `readability-api` exposes the
  [readability](https://github.com/mozilla/readability) as HTTP API service,
  used by `daemon` to clean up the HTML content.
- `web-spa` is a [yew](https://yew.rs/) front-end application.
- [minio](https://min.io/) used as static content storage and
  [postgresql](https://www.postgresql.org/) as the application database.

## How to run

```bash
$ docker compose down --volumes && docker compose build && docker compose up
```

## How to run E2E tests:

With the `docker-compose.yml` running, use [hurl](https://hurl.dev/)

```bash
$ hurl --verbose test.hurl
```

## How to run the `web-spa`

Soon, `web-spa` will be on its own docker container, but for now, you need to
follow the setup instructions in
[yew intruduction page](https://yew.rs/docs/getting-started/introduction).

After that, you should run the following:

```bash
$ cd web-spa; trunk serve
```
