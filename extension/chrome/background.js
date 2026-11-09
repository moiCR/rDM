const DEFAULT_SETTINGS = {
  interceptDownloads: true,
  serverHost: "127.0.0.1",
  serverPort: 9654,
  showNotifications: true,
};

let cachedSettings = { ...DEFAULT_SETTINGS };
const tabMediaMap = new Map();

async function refreshSettingsCache() {
  const result = await chrome.storage.local.get(DEFAULT_SETTINGS);
  cachedSettings = { ...DEFAULT_SETTINGS, ...result };
  return cachedSettings;
}

refreshSettingsCache();

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local") {
    for (const [key, change] of Object.entries(changes)) {
      if (key in DEFAULT_SETTINGS && change.newValue !== undefined) {
        cachedSettings[key] = change.newValue;
      }
    }
  }
});

function getServerUrl(settings, endpoint) {
  return `http://${settings.serverHost}:${settings.serverPort}${endpoint}`;
}

async function pingServer(settings) {
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

async function sendDownloadToRdm(url, filename, referer) {
  const settings = await refreshSettingsCache();
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 3000);
    const response = await fetch(getServerUrl(settings, "/api/v1/download"), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        url,
        filename: filename || null,
        referer: referer || null,
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
        message: `Descarga capturada: ${data.file_name || filename || "archivo"}`,
      });
    }
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

function formatBytes(bytes) {
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

function sanitizeTitle(title) {
  return title
    .replace(/^\(\d+\)\s*/, "")
    .replace(/\s*-\s*YouTube$/i, "")
    .replace(/[\\/:*?"<>|]/g, "_")
    .trim();
}

function extractMediaInfo(url, headers) {
  let contentType = "";
  let contentLength = 0;
  let dispositionName = null;

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

  let parsedUrl;
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
  const isGoogleVideo =
    pathname.endsWith("/videoplayback") &&
    (queryMime.startsWith("video/") || queryMime.startsWith("audio/"));

  if (isGoogleVideo) {
    const clen = parseInt(parsedUrl.searchParams.get("clen") || "0", 10) || contentLength;
    const cleanUrlObj = new URL(url);
    cleanUrlObj.searchParams.delete("range");
    cleanUrlObj.searchParams.delete("rn");
    cleanUrlObj.searchParams.delete("rbuf");
    cleanUrlObj.searchParams.delete("ump");
    cleanUrlObj.searchParams.delete("sabr");

    const isAudio = queryMime.startsWith("audio/");
    const subFormat = queryMime.includes("mp4")
      ? isAudio
        ? "m4a"
        : "mp4"
      : isAudio
        ? "webm"
        : "webm";

    return {
      url: cleanUrlObj.toString(),
      dedupeKey: `${parsedUrl.origin}${parsedUrl.pathname}?itag=${parsedUrl.searchParams.get("itag") || subFormat}`,
      filename: `videoplayback.${subFormat}`,
      genericName: true,
      fileExt: `.${subFormat}`,
      sizeBytes: clen,
      sizeFormatted: formatBytes(clen),
      ext: isAudio ? "AUDIO" : subFormat.toUpperCase(),
    };
  }

  const mediaExtensions = [".mp4", ".webm", ".mkv", ".mov", ".m3u8", ".mp3", ".m4a", ".flac"];
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
  const isGeneric = ["video", "videoplayback", "master", "index", "playlist", "stream"].includes(
    baseWithoutExt
  );

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

function updateTabBadge(tabId) {
  const list = tabMediaMap.get(tabId) || [];
  const text = list.length > 0 ? String(list.length) : "";
  chrome.action.setBadgeText({ tabId, text }, () => {
    void chrome.runtime.lastError;
  });
  if (list.length > 0) {
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
    const existingIndex = list.findIndex((item) => item.dedupeKey === info.dedupeKey);
    if (existingIndex !== -1) {
      if (info.sizeBytes > list[existingIndex].sizeBytes) {
        list[existingIndex] = { ...list[existingIndex], ...info };
      }
      return;
    }

    list.unshift({
      ...info,
      referer: details.initiator || details.documentUrl || null,
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

chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status === "loading" && changeInfo.url) {
    tabMediaMap.delete(tabId);
    updateTabBadge(tabId);
  }
});

chrome.tabs.onRemoved.addListener((tabId) => {
  tabMediaMap.delete(tabId);
});

chrome.runtime.onInstalled.addListener(() => {
  refreshSettingsCache();
  chrome.contextMenus.create({
    id: "rdm-download-link",
    title: "Descargar con rDM",
    contexts: ["link", "image", "video", "audio"],
  });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === "rdm-download-link") {
    const targetUrl = info.linkUrl || info.srcUrl;
    if (targetUrl) {
      const referer = tab?.url || null;
      const res = await sendDownloadToRdm(targetUrl, null, referer);
      if (!res.success) {
        chrome.notifications.create({
          type: "basic",
          iconUrl: "icons/icon128.png",
          title: "rDM no disponible",
          message: "No se pudo conectar con rDM. ¿Está la aplicación abierta?",
        });
      }
    }
  }
});

const ignoredDownloadIds = new Set();

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
      const res = await sendDownloadToRdm(url, item.filename, item.referrer || null);
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
      const res = await sendDownloadToRdm(url, item.filename, item.referrer || null);
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
      cachedSettings = { ...cachedSettings, ...message.settings };
      await chrome.storage.local.set(message.settings);
      sendResponse({ success: true });
    })();
    return true;
  }

  if (message.type === "GET_TAB_MEDIA") {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      const activeTab = tabs && tabs[0];
      if (!activeTab || activeTab.id === undefined) {
        sendResponse({ media: [], tabUrl: null });
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
      sendResponse({ media, tabUrl: activeTab.url || null });
    });
    return true;
  }

  if (message.type === "DOWNLOAD_MEDIA") {
    (async () => {
      const res = await sendDownloadToRdm(
        message.url,
        message.filename,
        message.referer
      );
      sendResponse(res);
    })();
    return true;
  }
});
