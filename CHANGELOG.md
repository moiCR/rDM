All notable changes to rDM are documented in this file.

## v1.1.0

### Features & Improvements

* **Responsive Download Cards.** Cards use equal-width columns that fill the available space and adapt when resizing the window or toggling the sidebar. Incomplete rows keep the same column widths.
* **Readable File Names and Sizes.** Limit displayed file names to 24 characters, preserving the extension when shortening long names, and reserve space for the file size.
* **Expanded Advanced Download Options.** Set a speed limit in KiB/s and an expected SHA-256 or MD5 checksum directly in the new-download dialog. Custom headers and cookies are available in a collapsible network section, with input validation.
* **Animated Download Dialog.** Animate dialog opening and closing, Advanced section height, and chevron rotation. Network options expand with a single height animation. Inputs and textareas retain 24 px rounded corners.
* **Automatic FFmpeg Setup.** Reuse an existing FFmpeg installation or download a private copy when conversion or audio/video merging first requires it. Added a Download FFmpeg button in advanced settings, preparation status, download progress, and retry after setup errors.
* **Verified FFmpeg Downloads.** Pin downloaded binaries to FFmpeg 6.1.1 from ffmpeg-static, verify their SHA-256 checksum before installation, and retain license and source information. Automatic setup supports Linux x64/ARM64, Windows x64, and macOS Intel/Apple Silicon without administrator privileges.
* **Refresh Expired Links.** Update the web address of a paused or failed download and continue from where it left off without losing progress.
* **File Integrity Verification.** Optionally verify completed downloads using SHA-256 or MD5 checksums to make sure files are intact and unmodified.
* **Proxy Support.** Connect through HTTP, HTTPS, or SOCKS5 proxies, with optional username and password authentication.
* **Custom Headers and Cookies.** Send custom headers and cookies for downloads that require login sessions or special access.
* **Per-Download Speed Limits.** Set and adjust speed limits for individual downloads at any time, in addition to the global speed limit.
* **Expanded Command-Line Options.** Added support for checksums, custom headers, cookies, per-download speed limits, and link refreshing in the CLI.

### Fixes

* **Safe Link Replacement.** Prevent updating a download link if the new link points to a file of a different size, protecting partial downloads from corruption.
* **Instant Network Settings.** Apply proxy and user-agent configuration changes to new and resumed downloads without needing to restart the app.
