All notable changes to rDM are documented in this file.

## v1.3.0

### Features & Improvements

* **Global App Search (`Ctrl+K`).** Added a unified search bar centered in the titlebar that expands smoothly in place into a single theme-integrated container to search across downloads, pages, settings, and quick actions with full keyboard navigation.
* **Download Inspector with Card-to-Modal Morph.** Clicking any download card transforms it smoothly into an inspector modal displaying per-segment progress bars, live transfer details, URL copy, destination path, speed limit, checksum, referer, error logs, and quick actions.
* **Notification Center & Native OS Notifications.** Added a `/notifications` page above Settings in the sidebar and dock with unread badges, persisted notification history, quick clear/mark-read controls, and native Windows/Linux notifications (configurable via a switch in Advanced Settings) for download start, completion, failure, and app updates.
* **Update Changelogs & Error Log Details.** Expanding a notification card morphs it into a detail modal displaying full release changelogs with direct update installation for available updates, or detailed error logs with copy, retry, and inspect actions for failed downloads.
* **Managed `yt-dlp` Media Extraction.** Added on-demand, SHA-256 verified `yt-dlp` installation in Advanced Settings (reusing system `PATH` binaries when present) and media format probing in the New Download dialog to choose video resolutions or audio-only MP3 before downloading.
* **Modal & Card Transition Polish.** Added proper title truncation in modal headers so long file names never overlap close buttons, preserved file extensions in inspector titles, matched modal heights dynamically to their content, and crossfaded card content smoothly on return.
