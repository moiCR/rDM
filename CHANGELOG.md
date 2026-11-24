All notable changes to rDM are documented in this file.

## v1.3.1

### Features & Improvements

* **Built-In Update Signature Verification.** Embedded the default rDM Minisign public key so builds verify signed update manifests and installers out of the box, while keeping compile-time key overrides via `RDM_UPDATER_PUBLIC_KEY`.

### Fixes

* **Linux Titlebar Window Dragging.** Unified the titlebar drag region so dragging the window and double-clicking to maximize work across both sides of the centered search bar without dead zones.
* **Linux System Tray Download Interception.** Fixed a crash when capturing downloads from the browser extension while rDM is closed to the system tray on Linux, and reset window-scoped modal states cleanly when reopening the window.
