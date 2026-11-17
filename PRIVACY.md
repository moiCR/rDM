# rDM Browser Extension — Privacy Policy

**Last updated:** November 17, 2026

## Overview

rDM is an open-source download manager with a browser extension that integrates Microsoft Edge with the locally installed rDM desktop application.

## Data Access and Processing

The extension accesses download and media URLs, associated filenames, referring page URLs or origins, active tab URLs and titles, and HTTP response metadata such as content type, content length, and content disposition. This information is used to detect downloadable resources, display available media, suggest filenames, and send downloads to rDM.

URLs and filenames may contain personal information or temporary access tokens. The extension uses this information only to provide its download management features. It does not build browsing profiles, sell user data, or use data for advertising, analytics, or tracking. It does not read browser passwords, form inputs, or browser cookies through the cookies API.

## Local Application Communication

The extension communicates with the rDM desktop application through a local HTTP connection, using 127.0.0.1 and port 9654 by default.

Download URLs and relevant metadata are transferred to the desktop application when downloads are captured or requested by the user.

Automatic download interception is enabled by default and can be disabled in the extension settings. Transfers can include a filename, referring URL, audio URL, media sizes, and selected audio bitrate when applicable. Connection checks are also sent to the configured rDM endpoint.

When the extension popup is opened on a supported YouTube video page, it sends the video identifier to the desktop application to retrieve available formats, even before a download is selected.

The extension uses HTTP for its desktop connection. The default loopback address keeps this communication on the same device. If the connection host is changed, requests are sent to that configured host.

## External Services and Data Sharing

The extension does not send download or browsing information to a developer-operated server. Its application requests go to the configured rDM endpoint.

The desktop application connects to the websites and content servers needed to resolve media and download requested files. For YouTube format lookup, it contacts YouTube using the video identifier. These services receive the network information needed to handle those requests, including the device's public IP address, and may process information under their own privacy policies. Local processing by the extension does not mean that subsequent desktop application requests stay on the device.

## Browser Permissions

The Microsoft Edge extension requests these permissions for its features:

- **downloads:** detect, pause, and transfer browser downloads to rDM; cancel and remove the browser download entry after a successful handoff, or resume it if the handoff fails.
- **contextMenus:** provide the download action for links, images, video, and audio.
- **storage:** save extension preferences locally.
- **notifications:** show download handoff and connection notifications, which may include a filename.
- **webRequest and access to all URLs:** inspect resource URLs and response headers to detect media across websites.
- **tabs:** read tab URLs and titles for media display, filename suggestions, and referring page information.
- **localhost and loopback host access:** communicate with the locally installed desktop application.

## Local Storage

The extension stores user preferences locally, including language, download interception settings, notification preferences, and desktop application connection settings.

## Data Retention

Detected media information is maintained in the extension's runtime memory, with up to eight entries per tab. Entries are removed when the tab closes or when the extension receives a new-page loading event with a changed URL. Runtime information can also be lost when the browser stops or restarts the extension's background service worker. The extension does not persist a browsing history or media detection history to local storage.

Preferences remain in browser local storage until changed or removed, including by uninstalling the extension.

The rDM desktop application may retain download information as part of its download management functionality.

## User Controls

Users can disable automatic download interception and manage extension preferences. Users may also remove the extension and its locally stored settings through Microsoft Edge.

Disabling interception stops automatic download handoffs; it does not disable media detection. To stop all extension activity, disable or uninstall the extension through `edge://extensions`. Website access can also be managed through Microsoft Edge's extension controls.

Uninstalling the extension does not delete files or download records already held by the desktop application. Manage those separately in rDM and on your device.

## Changes to This Policy

This policy may be updated when the extension's data handling changes. Updates will be published in this document with a revised last-updated date.

## Contact

For privacy-related questions, please open an issue at:

[rDM GitHub issues](https://github.com/moiCR/rDM/issues)

Issues are public. Do not include private download URLs, access tokens, or other sensitive information in a privacy question.
