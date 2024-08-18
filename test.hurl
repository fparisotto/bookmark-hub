# sign-up
POST http://localhost:3000/api/v1/auth/sign-up
{
  "email": "test@gmail.com",
  "password": "1234",
  "password_confirmation": "1234"
}

HTTP/1.1 200
[Captures]
user_id: jsonpath "$.id"


# sign-in
POST http://localhost:3000/api/v1/auth/sign-in
{
  "email": "test@gmail.com",
  "password": "1234"
}

HTTP/1.1 200
[Captures]
token: jsonpath "$.access_token"


# get user profile
GET http://localhost:3000/api/v1/auth/user-profile
Authorization: Bearer {{token}}

HTTP/1.1 200
[Asserts]
jsonpath "$.email" == "test@gmail.com"


# post a bookmark
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tokio.rs/tokio/topics/tracing",
  "tags": ["Rust", "Tokio"]
}

HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tokio.rs/tokio/topics/tracing"
jsonpath "$.tags[*]" includes "Rust"
jsonpath "$.tags[*]" includes "Tokio"


# wait until bookmark completed
GET http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
[Options]
retry: 10

HTTP/1.1 200
[Captures]
bookmark_id: jsonpath "$.bookmarks[0].bookmark_id"


# get bookmark by id
GET http://localhost:3000/api/v1/bookmarks/{{bookmark_id}}
Authorization: Bearer {{token}}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmark_id" == {{bookmark_id}}


# set tags to bookmark
POST http://localhost:3000/api/v1/bookmarks/{{bookmark_id}}/tags
Authorization: Bearer {{token}}
{
  "tags": ["rust", "tokio"]
}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmark_id" == {{bookmark_id}}
jsonpath "$.url" == "https://tokio.rs/tokio/topics/tracing"
jsonpath "$.tags" count == 2
jsonpath "$.tags[*]" includes "rust"
jsonpath "$.tags[*]" includes "tokio"


# append tags to bookmark
PATCH http://localhost:3000/api/v1/bookmarks/{{bookmark_id}}/tags
Authorization: Bearer {{token}}
{
  "tags": ["tracing"]
}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmark_id" == {{bookmark_id}}
jsonpath "$.url" == "https://tokio.rs/tokio/topics/tracing"
jsonpath "$.tags" count == 3
jsonpath "$.tags[*]" includes "rust"
jsonpath "$.tags[*]" includes "tokio"
jsonpath "$.tags[*]" includes "tracing"


# post a new bookmark
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tokio.rs/blog/2022-02-announcing-tokio-metrics",
  "tags": ["rust", "tokio", "metrics"]
}

HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tokio.rs/blog/2022-02-announcing-tokio-metrics"
jsonpath "$.tags[*]" includes "rust"
jsonpath "$.tags[*]" includes "tokio"
jsonpath "$.tags[*]" includes "metrics"


# wait until the new bookmark completed
GET http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
[Options]
retry: 10

HTTP/1.1 200
[Asserts]
body contains "https://tokio.rs/blog/2022-02-announcing-tokio-metrics"

# get_all_tags
GET http://localhost:3000/api/v1/tags
Authorization: Bearer {{token}}

HTTP/1.1 200
[Asserts]
jsonpath "$.tags" count == 4
jsonpath "$.tags[?(@.tag=='rust')].count" includes 2
jsonpath "$.tags[?(@.tag=='tokio')].count" includes 2
jsonpath "$.tags[?(@.tag=='tracing')].count" includes 1
jsonpath "$.tags[?(@.tag=='metrics')].count" includes 1


# get_bookmarks_by_tag
GET http://localhost:3000/api/v1/tags/metrics
Authorization: Bearer {{token}}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmarks" count == 1
jsonpath "$.bookmarks[0].url" == "https://tokio.rs/blog/2022-02-announcing-tokio-metrics"


# search by query
POST http://localhost:3000/api/v1/search
Authorization: Bearer {{token}}
{
  "query": "tracing"
}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmarks" count == 1
jsonpath "$.bookmarks[0].url" == "https://tokio.rs/tokio/topics/tracing"
jsonpath "$.bookmarks[0].search_match" contains "tracing"


# search by query with and
POST http://localhost:3000/api/v1/search
Authorization: Bearer {{token}}
{
  "query": "tracing & tokio"
}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmarks" count == 1
jsonpath "$.bookmarks[0].url" == "https://tokio.rs/tokio/topics/tracing"
jsonpath "$.bookmarks[0].search_match" contains "tracing"
jsonpath "$.bookmarks[0].search_match" contains "Tokio"


# search by phrase
POST http://localhost:3000/api/v1/search
Authorization: Bearer {{token}}
{
  "query": "\"we are happy to announce the initial release of tokio-metrics\""
}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmarks" count == 1
jsonpath "$.bookmarks[0].url" == "https://tokio.rs/blog/2022-02-announcing-tokio-metrics"
jsonpath "$.bookmarks[0].search_match" contains "tokio"
jsonpath "$.bookmarks[0].search_match" contains "metrics"


# empty search should return all results
POST http://localhost:3000/api/v1/search
Authorization: Bearer {{token}}
{}

HTTP/1.1 200
[Asserts]
jsonpath "$.bookmarks" count == 2


# post a new bookmark with images
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tailscale.com/blog/how-nat-traversal-works/",
  "tags": ["network"]
}

HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tailscale.com/blog/how-nat-traversal-works/"
jsonpath "$.tags[0]" == "network"


# post the same bookmark again, should update tags
POST http://localhost:3000/api/v1/bookmarks
Authorization: Bearer {{token}}
{
  "url": "https://tailscale.com/blog/how-nat-traversal-works/",
  "tags": ["network", "nat"]
}

HTTP/1.1 201
[Asserts]
jsonpath "$.url" == "https://tailscale.com/blog/how-nat-traversal-works/"
jsonpath "$.tags[*]" includes "network"
jsonpath "$.tags[*]" includes "nat"
