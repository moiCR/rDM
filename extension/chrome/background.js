const DEFAULT_SETTINGS = {
  interceptDownloads: true,
  serverHost: "127.0.0.1",
  serverPort: 9654,
  showNotifications: true,
};

let cachedSettings = { ...DEFAULT_SETTINGS };

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
});
