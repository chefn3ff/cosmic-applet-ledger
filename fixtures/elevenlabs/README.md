# ElevenLabs subscription fixtures

These sanitized responses exercise `GET https://api.elevenlabs.io/v1/user/subscription`
without credentials or personal data.

The fetcher authenticates with an `xi-api-key: <key>` header (ElevenLabs does not use Bearer
auth). Only `tier`, `character_count`, `character_limit`, `next_character_count_reset_unix` and
`status` are read. `character_count` is credits used this period. A zero `character_limit`
reports 0% instead of dividing by zero, and a missing reset field leaves the reset time empty.
The responses contain only synthetic values and are not raw captures.
