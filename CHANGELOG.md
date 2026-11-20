All notable changes to rDM are documented in this file.

## v1.2.0

### Features & Improvements

* **Directory-Based Routing with `gpui-pages`.** Migrated navigation from `gpui-router` to `gpui-pages`, implementing Dashboard and Downloads as native compile-time discovered pages under `src/pages/` with a persistent `RootLayout` that preserves sidebar, dock, focus, and keyboard shortcuts across page switches.
* **Page Entrance Transitions.** Added slide-up fade entrance animations to Dashboard and Downloads pages via `Page::options()`.
* **Dedicated Global Modals.** Moved global overlay dialogs (Settings, New Download, and Delete Download) into a dedicated `modals` module and removed the legacy `views` directory and unused `/new` route stub.
* **Unified Button Components.** Replaced custom pill button wrappers with shared `gpui-kit` button components across Settings, Downloads, and Delete Download dialogs for consistent styling and interaction.
* **Updated GPUI Stack.** Upgraded `gpui-pre` and `gpui-kit` dependencies and simplified window initialization and router attachment on Windows and Linux.
* **Browser Extension Privacy Policy & Website Updates.** Added `PRIVACY.md` detailing data handling for the rDM Browser Extension, linked the Firefox extension to Mozilla Add-ons on the website, and added the rDM favicon.
