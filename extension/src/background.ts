import { resolveLanguage, translations, type Language } from "./i18n";

interface Settings {
  language: Language;
  interceptDownloads: boolean;
  serverHost: string;
  serverPort: number;
  showNotifications: boolean;
}

interface MediaInfo {
  url: string;
  dedupeKey: string;
  filename: string;
  genericName: boolean;
  fileExt: string;
  sizeBytes: number;
  sizeFormatted: string | null;
  ext: string;
  referer?: string | null;
}

interface DownloadOptions {
  url: string;
  filename?: string | null;
  referer?: string | null;
  videoSize?: number | null;
  audioUrl?: string | null;
  audioSize?: number | null;
  mp3Bitrate?: number | null;
}

const DEFAULT_SETTINGS: Settings = {
  language: "en",
  interceptDownloads: true,
  serverHost: "127.0.0.1",
  serverPort: 9654,
  showNotifications: true,
};

let cachedSettings: Settings = { ...DEFAULT_SETTINGS };
const tabMediaMap = new Map<number, MediaInfo[]>();

async function refreshSettingsCache(): Promise<Settings> {
  const result = await chrome.storage.local.get(DEFAULT_SETTINGS);
  cachedSettings = { ...DEFAULT_SETTINGS, ...result, language: resolveLanguage(result.language) };
  return cachedSettings;
}

void refreshSettingsCache();

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local") {
    for (const [key, change] of Object.entries(changes)) {
      if (key in DEFAULT_SETTINGS && change.newValue !== undefined) {
        (cachedSettings as unknown as Record<string, unknown>)[key] = change.newValue;
      }
    }
    if (changes.language) {
      cachedSettings.language = resolveLanguage(changes.language.newValue);
      updateContextMenu();
    }
  }
});

function updateContextMenu() {
  chrome.contextMenus.update("rdm-download-link", {
    title: translations[cachedSettings.language].downloadWith,
  }, () => { void chrome.runtime.lastError; });
}

function getServerUrl(settings: Settings, endpoint: string): string {
  return `http://${settings.serverHost}:${settings.serverPort}${endpoint}`;
}

async function pingServer(settings: Settings): Promise<boolean> {
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 1200);
    const response = await fetch(getServerUrl(settings, "/api/v1/ping"), {
      signal: controller.signal,
    });
    clearTimeout(timeoutId);
    return response.ok;
  } catch {
    return false;
  }
}

async function sendDownloadToRdm(options: DownloadOptions): Promise<{
  success: boolean;
  data?: { file_name?: string };
  error?: string;
}> {
  const settings = await refreshSettingsCache();
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 4000);
    const response = await fetch(getServerUrl(settings, "/api/v1/download"), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        url: options.url,
        filename: options.filename || null,
        referer: options.referer || null,
        video_size: options.videoSize || null,
        audio_url: options.audioUrl || null,
        audio_size: options.audioSize || null,
        mp3_bitrate: options.mp3Bitrate || null,
      }),
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    if (!response.ok) {
      throw new Error(`Server returned HTTP ${response.status}`);
    }

    const data = await response.json();
    if (settings.showNotifications) {
      chrome.notifications.create({
        type: "basic",
        iconUrl: "icons/icon128.png",
        title: "rDM",
        message: `${translations[settings.language].captured} ${data.file_name || options.filename || translations[settings.language].file}`,
      });
    }
    return { success: true, data };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function formatBytes(bytes: number): string | null {
  if (!bytes || bytes <= 0) return null;
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function sanitizeTitle(title: string): string {
  return title
    .replace(/^\(\d+\)\s*/, "")
    .replace(/\s*-\s*YouTube$/i, "")
    .replace(/[\\/:*?"<>|]/g, "_")
    .trim();
}

function extractMediaInfo(
  url: string,
  headers?: chrome.webRequest.HttpHeader[]
): MediaInfo | null {
  let contentType = "";
  let contentLength = 0;
  let dispositionName: string | null = null;

  for (const h of headers || []) {
    const name = h.name.toLowerCase();
    if (name === "content-type") {
      contentType = (h.value || "").toLowerCase().split(";")[0].trim();
    } else if (name === "content-length") {
      contentLength = parseInt(h.value || "0", 10) || 0;
    } else if (name === "content-disposition" && h.value) {
      const match = h.value.match(/filename\*?=(?:UTF-8'')?["']?([^"';]+)["']?/i);
      if (match && match[1]) {
        try {
          dispositionName = decodeURIComponent(match[1].trim());
        } catch {
          dispositionName = match[1].trim();
        }
      }
    }
  }

  let parsedUrl: URL;
  try {
    parsedUrl = new URL(url);
  } catch {
    return null;
  }

  const pathname = parsedUrl.pathname.toLowerCase();
  if (pathname.endsWith(".ts") || pathname.endsWith(".m4s")) {
    return null;
  }

  const queryMime = (parsedUrl.searchParams.get("mime") || "").toLowerCase();
  if (
    pathname.endsWith("/videoplayback") &&
    (queryMime.startsWith("video/") || queryMime.startsWith("audio/"))
  ) {
    return null;
  }

  const mediaExtensions = [
    ".mp4",
    ".webm",
    ".mkv",
    ".mov",
    ".m3u8",
    ".mp3",
    ".m4a",
    ".flac",
  ];
  const matchedExt = mediaExtensions.find((ext) => pathname.endsWith(ext));

  const isManifest =
    matchedExt === ".m3u8" ||
    contentType === "application/vnd.apple.mpegurl" ||
    contentType === "application/x-mpegurl";

  const isVideoOrAudioMime =
    contentType.startsWith("video/") ||
    contentType.startsWith("audio/") ||
    isManifest;

  if (!isVideoOrAudioMime && !matchedExt) {
    return null;
  }

  if (!isManifest && contentLength > 0 && contentLength < 102400) {
    return null;
  }

  let extLabel = "VIDEO";
  if (isManifest) {
    extLabel = "HLS";
  } else if (matchedExt) {
    extLabel = matchedExt.slice(1).toUpperCase();
  } else if (contentType.includes("/")) {
    const sub = contentType.split("/")[1].replace("x-", "").toUpperCase();
    extLabel = sub.length <= 5 ? sub : "VIDEO";
  }

  let rawName = dispositionName;
  if (!rawName) {
    const segments = parsedUrl.pathname.split("/").filter(Boolean);
    const lastSeg = segments[segments.length - 1] || "video";
    try {
      rawName = decodeURIComponent(lastSeg);
    } catch {
      rawName = lastSeg;
    }
  }

  const fallbackExt = isManifest ? ".m3u8" : matchedExt || ".mp4";
  const baseWithoutExt = rawName.replace(/\.[a-z0-9]{2,5}$/i, "").toLowerCase();
  const isGeneric = [
    "video",
    "videoplayback",
    "master",
    "index",
    "playlist",
    "stream",
  ].includes(baseWithoutExt);

  if (!/\.[a-z0-9]{2,5}$/i.test(rawName)) {
    rawName = `${rawName}${fallbackExt}`;
  }

  return {
    url,
    dedupeKey: url,
    filename: rawName,
    genericName: isGeneric,
    fileExt: fallbackExt,
    sizeBytes: contentLength,
    sizeFormatted: formatBytes(contentLength),
    ext: extLabel,
  };
}

function updateTabBadge(tabId: number, tabUrl?: string | null) {
  const list = tabMediaMap.get(tabId) || [];
  let count = list.length;
  if (tabUrl && /(?:youtube\.com\/(?:watch\?v=|shorts\/)|youtu\.be\/)/.test(tabUrl)) {
    count = Math.max(count, 1);
  }
  const text = count > 0 ? String(count) : "";
  chrome.action.setBadgeText({ tabId, text }, () => {
    void chrome.runtime.lastError;
  });
  if (count > 0) {
    chrome.action.setBadgeBackgroundColor({ tabId, color: "#223c61" }, () => {
      void chrome.runtime.lastError;
    });
  }
}

chrome.webRequest.onHeadersReceived.addListener(
  (details) => {
    if (details.tabId < 0) return;
    if (!details.url || !details.url.startsWith("http")) return;

    const info = extractMediaInfo(details.url, details.responseHeaders);
    if (!info) return;

    const list = tabMediaMap.get(details.tabId) || [];
    const existingIndex = list.findIndex(
      (item) => item.dedupeKey === info.dedupeKey
    );
    if (existingIndex !== -1) {
      if (info.sizeBytes > list[existingIndex].sizeBytes) {
        list[existingIndex] = { ...list[existingIndex], ...info };
      }
      return;
    }

    list.unshift({
      ...info,
      referer: details.initiator || (details as { documentUrl?: string }).documentUrl || null,
    });

    if (list.length > 8) {
      list.length = 8;
    }

    tabMediaMap.set(details.tabId, list);
    updateTabBadge(details.tabId);
  },
  { urls: ["<all_urls>"] },
  ["responseHeaders"]
);

chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.status === "loading" && changeInfo.url) {
    tabMediaMap.delete(tabId);
    updateTabBadge(tabId, changeInfo.url);
  } else if (changeInfo.status === "complete" && tab?.url) {
    updateTabBadge(tabId, tab.url);
  }
});

chrome.tabs.onRemoved.addListener((tabId) => {
  tabMediaMap.delete(tabId);
});

chrome.runtime.onInstalled.addListener(async () => {
  await refreshSettingsCache();
  chrome.contextMenus.create({
    id: "rdm-download-link",
    title: translations[cachedSettings.language].downloadWith,
    contexts: ["link", "image", "video", "audio"],
  });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === "rdm-download-link") {
    const targetUrl = info.linkUrl || info.srcUrl;
    if (targetUrl) {
      const referer = tab?.url || null;
      const res = await sendDownloadToRdm({
        url: targetUrl,
        filename: null,
        referer,
      });
      if (!res.success) {
        chrome.notifications.create({
          type: "basic",
          iconUrl: "icons/icon128.png",
          title: translations[cachedSettings.language].unavailable,
          message: translations[cachedSettings.language].connectionError,
        });
      }
    }
  }
});

const ignoredDownloadIds = new Set<number>();

if (chrome.downloads && chrome.downloads.onDeterminingFilename) {
  chrome.downloads.onDeterminingFilename.addListener((item, suggest) => {
    if (ignoredDownloadIds.has(item.id)) {
      return false;
    }

    if (!cachedSettings.interceptDownloads) {
      return false;
    }

    const url = item.finalUrl || item.url;
    if (!url || !url.startsWith("http")) {
      return false;
    }

    ignoredDownloadIds.add(item.id);
    chrome.downloads.pause(item.id, () => {
      void chrome.runtime.lastError;
    });

    (async () => {
      const res = await sendDownloadToRdm({
        url,
        filename: item.filename,
        referer: item.referrer || null,
      });
      if (res.success) {
        chrome.downloads.cancel(item.id, () => {
          void chrome.runtime.lastError;
          chrome.downloads.erase({ id: item.id }, () => {
            void chrome.runtime.lastError;
          });
        });
      } else {
        chrome.downloads.resume(item.id, () => {
          void chrome.runtime.lastError;
          suggest();
        });
      }
    })();

    return true;
  });
} else if (chrome.downloads && chrome.downloads.onCreated) {
  chrome.downloads.onCreated.addListener((item) => {
    if (ignoredDownloadIds.has(item.id) || !cachedSettings.interceptDownloads) {
      return;
    }
    const url = item.finalUrl || item.url;
    if (!url || !url.startsWith("http")) {
      return;
    }
    ignoredDownloadIds.add(item.id);
    chrome.downloads.pause(item.id, () => {
      void chrome.runtime.lastError;
    });
    (async () => {
      const res = await sendDownloadToRdm({
        url,
        filename: item.filename,
        referer: item.referrer || null,
      });
      if (res.success) {
        chrome.downloads.cancel(item.id, () => {
          void chrome.runtime.lastError;
          chrome.downloads.erase({ id: item.id }, () => {
            void chrome.runtime.lastError;
          });
        });
      } else {
        chrome.downloads.resume(item.id, () => {
          void chrome.runtime.lastError;
        });
      }
    })();
  });
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message.type === "CHECK_STATUS") {
    (async () => {
      const settings = await refreshSettingsCache();
      const isAlive = await pingServer(settings);
      sendResponse({ connected: isAlive, settings });
    })();
    return true;
  }

  if (message.type === "SAVE_SETTINGS") {
    (async () => {
      cachedSettings = { ...cachedSettings, ...message.settings, language: resolveLanguage(message.settings?.language) };
      await chrome.storage.local.set(message.settings);
      sendResponse({ success: true });
    })();
    return true;
  }

  if (message.type === "GET_TAB_MEDIA") {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      const activeTab = tabs && tabs[0];
      if (!activeTab || activeTab.id === undefined) {
        sendResponse({ media: [], tabUrl: null, tabTitle: null });
        return;
      }
      const rawList = tabMediaMap.get(activeTab.id) || [];
      const tabTitle = activeTab.title ? sanitizeTitle(activeTab.title) : null;
      const media = rawList.map((item) => {
        if (item.genericName && tabTitle) {
          return {
            ...item,
            filename: `${tabTitle}${item.fileExt || ".mp4"}`,
          };
        }
        return item;
      });
      sendResponse({
        media,
        tabUrl: activeTab.url || null,
        tabTitle,
      });
    });
    return true;
  }

  if (message.type === "DOWNLOAD_MEDIA") {
    (async () => {
      const res = await sendDownloadToRdm({
        url: message.url,
        filename: message.filename,
        referer: message.referer,
        videoSize: message.videoSize,
        audioUrl: message.audioUrl,
        audioSize: message.audioSize,
        mp3Bitrate: message.mp3Bitrate,
      });
      sendResponse(res);
    })();
    return true;
  }
});
