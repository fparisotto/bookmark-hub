# Local bootstrap: create an admin user and seed 3 tag-less bookmarks.
#
# Run against a healthy server:
#   hurl --verbose bootstrap.hurl
#
# Credentials: admin / admin1234
# (the server enforces a minimum password length of 8, so "admin" alone is rejected)
#
# Idempotent: sign-up tolerates a 400 (user already exists) so this can be
# re-run without resetting the database.

# sign-up admin (200 on a fresh db, 400 if the user already exists)
POST http://localhost:3000/api/v1/auth/sign-up
{
  "username": "admin",
  "password": "admin1234",
  "password_confirmation": "admin1234"
}
HTTP *

# sign-in as admin
POST http://localhost:3000/api/v1/auth/sign-in
{
  "username": "admin",
  "password": "admin1234"
}
HTTP/1.1 200
[Captures]
token: jsonpath "$.access_token"

# bookmark 1 - no tags
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tokio.rs/tokio/topics/tracing",
  "tags": []
}
HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tokio.rs/tokio/topics/tracing"

# bookmark 2 - no tags
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tokio.rs/blog/2022-02-announcing-tokio-metrics",
  "tags": []
}
HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tokio.rs/blog/2022-02-announcing-tokio-metrics"

# bookmark 3 - no tags
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tailscale.com/blog/how-nat-traversal-works/",
  "tags": []
}
HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tailscale.com/blog/how-nat-traversal-works/"