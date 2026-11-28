All notable changes to rDM are documented in this file.

## v1.3.2

### Fixes

* **KDE Plasma Duplicate Titlebar.** Requested client-side window decorations (`WindowDecorations::Client`) with a transparent surface on Linux so KDE Plasma (KWin) no longer draws a native server-side titlebar above rDM's custom titlebar, and gated Linux window controls and drag handlers on active client-side decorations.
* **Linux Rounded Window Corners.** Configured a `12px` client-side window corner radius with theme-matched borders across light and dark modes, aligned the root layout, titlebar, close-button hover state, and modal backdrops to the window's active corner radii (keeping corners square when tiled or maximized), and removed redundant inner page and sidebar backgrounds that overlapped the rounded corners on GNOME and KDE.
