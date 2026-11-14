# Fish Audio API credit fixtures

`api_credit.json` is the shape of `GET https://api.fish.audio/wallet/self/api-credit`, taken from a
real account on 2026-11-14 with the ids replaced by fake values and no credentials.

The fetcher authenticates with `Authorization: Bearer <key>`. Only `credit` (dollars left) and
`cumulative_top_up` (dollars ever added) are read. Both are decimal strings, not numbers. A bad key
gets HTTP 401 with `{"status":401,"message":"Invalid token"}`.
