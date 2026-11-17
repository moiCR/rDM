# rDM Browser Extension — Privacy Policy

**Last updated:** November 17, 2026

## Overview

rDM is an open-source download manager with a browser extension that integrates Microsoft Edge with the locally installed rDM desktop application.

## Data Access and Processing

The extension accesses download URLs, associated filenames, referring page URLs, and HTTP response metadata to detect downloadable files and media resources.

Detected media information is processed locally to provide download management functionality.

## Local Application Communication

The extension communicates with the rDM desktop application through a local HTTP connection, using 127.0.0.1 and port 9654 by default.

Download URLs and relevant metadata are transferred to the desktop application when downloads are captured or requested by the user.

## Local Storage

The extension stores user preferences locally, including language, download interception settings, notification preferences, and desktop application connection settings.

## Data Retention

Detected media information is maintained in the extension's runtime memory and cleared when the associated browser tab is closed or navigates to another page.

The rDM desktop application may retain download information as part of its download management functionality.

## User Controls

Users can disable automatic download interception and manage extension preferences. Users may also remove the extension and its locally stored settings through Microsoft Edge.

## Contact

For privacy-related questions, please open an issue at:

https://github.com/moiCR/rDM/issues
