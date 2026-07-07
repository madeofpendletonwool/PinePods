## Title
Web: 20 i18n keys referenced in code are missing from en.json

## Status upstream
Not filed as its own issue - this surfaced as a CI failure ("i18n String Coverage") on the fork-sync PR that caught this fork's `main` up to upstream. The gap is pre-existing on upstream `main` itself (accumulated across many commits over time), not introduced by that sync.

## Description

`.github/scripts/check_i18n_coverage.py` (added upstream to enforce that every `i18n.t("...")` call has a matching entry in `web/src/translations/en.json`) found 20 keys referenced in the Rust web frontend that aren't defined:

- `failed_to_find_podcast_metadata` (note: no namespace prefix, unlike its neighbors in `episode.rs` which all use `episode.*` - possibly a naming slip in the original commit, kept as-is here since renaming it would need to be verified against every consumer)
- `nextcloud_options.click_refresh`, `.gpodder_device_mgmt`, `.loading_default_device`, `.no_default_device_set`, `.no_statistics_available`, `.no_sync_configured`, `.testing`
- `oauth_callback.date_format_iso8601`, `.date_format_julian`
- `oidc.environment`, `.failed_to_update_provider`, `.github_no_standard_scopes`, `.github_provider_detected`, `.github_scopes`, `.google_scopes`, `.standard_oidc_scopes`
- `podcast_index_matching.loading_podcasts`, `.no_matches_found`, `.try_manual_search`

The first 19 are what the CI check flagged on the fork-sync PR (diff-mode only checks keys introduced in a diff); running the script in full-scan mode against the whole tree turned up one more pre-existing gap (`oidc.failed_to_update_provider`) that wasn't part of that particular diff.

## Fix

Added all 20 keys to `en.json`. Wording was chosen by reading each call site's surrounding UI context and matching the tone/casing of neighboring keys already in the same section; two (`oauth_callback.date_format_iso8601`/`.date_format_julian`) reuse the exact existing text from `login.date_format_iso8601`/`.date_format_julian`, since `oauth_callback.rs`'s date-format dropdown is a duplicate of the one on the login page.

Only `en.json` was touched - other locale files are left for translators, consistent with how the check script itself describes the fix ("Add each key to web/src/translations/en.json... add the same key to all other locale files (or leave for translators)").

Not in scope here: the script also reports 252 "orphaned" keys (defined in en.json but no longer referenced anywhere) - that's a separate, pre-existing cleanup concern, left untouched.
