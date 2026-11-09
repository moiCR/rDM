const statusIndicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const interceptToggle = document.getElementById("intercept-toggle");
const notificationsToggle = document.getElementById("notifications-toggle");
const serverPortInput = document.getElementById("server-port");
const saveBtn = document.getElementById("save-btn");
const mediaSection = document.getElementById("media-section");
const mediaDivider = document.getElementById("media-divider");
const mediaList = document.getElementById("media-list");

let currentPort = 9654;

function updateStatus(connected) {
  if (connected) {
    statusIndicator.className = "status-pill connected";
    statusText.textContent = "Conectado";
  } else {
    statusIndicator.className = "status-pill disconnected";
    statusText.textContent = "Desconectado";
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
  return (title || "video")
    .replace(/^\(\d+\)\s*/, "")
    .replace(/\s*-\s*YouTube$/i, "")
    .replace(/[\\/:*?"<>|]/g, "_")
    .trim();
}

function extractYouTubeVideoId(urlStr) {
  if (!urlStr) return null;
  try {
    const parsed = new URL(urlStr);
    const host = parsed.hostname.toLowerCase();
    if (host === "youtu.be") {
      const id = parsed.pathname.split("/").filter(Boolean)[0];
      return id || null;
    }
    if (
      host === "youtube.com" ||
      host === "www.youtube.com" ||
      host === "m.youtube.com"
    ) {
      if (parsed.pathname === "/watch") {
        return parsed.searchParams.get("v");
      }
      if (
        parsed.pathname.startsWith("/shorts/") ||
        parsed.pathname.startsWith("/embed/")
      ) {
        return parsed.pathname.split("/").filter(Boolean)[1] || null;
      }
    }
  } catch {
    return null;
  }
  return null;
}

async function sendDirectToRdm(url, filename, referer) {
  const port = parseInt(serverPortInput.value, 10) || currentPort || 9654;
  try {
    const response = await fetch(`http://127.0.0.1:${port}/api/v1/download`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        url,
        filename: filename || null,
        referer: referer || null,
      }),
    });
    return response.ok;
  } catch {
    return false;
  }
}

function renderMediaItems(items, fallbackReferer) {
  mediaList.innerHTML = "";
  if (!items || items.length === 0) {
    mediaSection.classList.add("hidden");
    mediaDivider.classList.add("hidden");
    return;
  }

  mediaSection.classList.remove("hidden");
  mediaDivider.classList.remove("hidden");

  for (const item of items) {
    const row = document.createElement("div");
    row.className = "media-row";

    const info = document.createElement("div");
    info.className = "media-info";

    const title = document.createElement("span");
    title.className = "media-title";
    title.textContent = item.filename || "video.mp4";
    title.title = item.filename || item.url;

    const meta = document.createElement("span");
    meta.className = "media-meta";
    const sizeLabel =
      item.sizeFormatted || formatBytes(item.size_bytes) || "Video detectado";
    meta.textContent = `${item.ext || "VIDEO"} • ${sizeLabel}`;

    info.appendChild(title);
    info.appendChild(meta);

    const btn = document.createElement("button");
    btn.className = "save-btn";
    btn.textContent = "Descargar";

    btn.addEventListener("click", async () => {
      btn.disabled = true;
      btn.textContent = "Enviando...";
      const ok = await sendDirectToRdm(
        item.url,
        item.filename,
        item.referer || fallbackReferer || null
      );
      if (ok) {
        btn.textContent = "Enviado ✓";
      } else {
        btn.textContent = "Error";
      }
      setTimeout(() => {
        btn.disabled = false;
        btn.textContent = "Descargar";
      }, 1200);
    });

    row.appendChild(info);
    row.appendChild(btn);
    mediaList.appendChild(row);
  }
}

async function detectActiveTabMedia() {
  let backgroundMedia = [];
  let activeUrl = null;
  let activeTitle = null;

  await new Promise((resolve) => {
    try {
      chrome.runtime.sendMessage({ type: "GET_TAB_MEDIA" }, (res) => {
        void chrome.runtime.lastError;
        if (res && Array.isArray(res.media)) {
          backgroundMedia = res.media;
          activeUrl = res.tabUrl || null;
        }
        resolve();
      });
    } catch {
      resolve();
    }
  });

  if (chrome.tabs && chrome.tabs.query) {
    await new Promise((resolve) => {
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        void chrome.runtime.lastError;
        const tab = tabs && tabs[0];
        if (tab) {
          activeUrl = tab.url || activeUrl;
          activeTitle = tab.title || null;
        }
        resolve();
      });
    });
  }

  const ytVideoId = extractYouTubeVideoId(activeUrl);
  if (ytVideoId) {
    const cleanTitle = sanitizeTitle(activeTitle || "youtube_video");
    const fallbackItem = [
      {
        url: activeUrl,
        filename: `${cleanTitle}.mp4`,
        ext: "MP4",
        sizeFormatted: "YouTube Video",
        referer: activeUrl,
      },
    ];
    renderMediaItems(fallbackItem, activeUrl);

    const port = parseInt(serverPortInput.value, 10) || currentPort || 9654;
    try {
      const res = await fetch(
        `http://127.0.0.1:${port}/api/v1/youtube?v=${encodeURIComponent(ytVideoId)}`
      );
      if (res.ok) {
        const data = await res.json();
        if (data && Array.isArray(data.formats) && data.formats.length > 0) {
          renderMediaItems(data.formats, activeUrl);
          return;
        }
      }
    } catch {
      // Mantiene el fallbackItem ya renderizado
    }
    return;
  }

  if (backgroundMedia.length > 0) {
    renderMediaItems(backgroundMedia, activeUrl);
  }
}

function persistSettings(callback) {
  const port = parseInt(serverPortInput.value, 10) || 9654;
  currentPort = port;
  const newSettings = {
    interceptDownloads: interceptToggle.checked,
    showNotifications: notificationsToggle.checked,
    serverPort: port,
  };

  chrome.runtime.sendMessage({ type: "SAVE_SETTINGS", settings: newSettings }, () => {
    if (callback) callback();
    chrome.runtime.sendMessage({ type: "CHECK_STATUS" }, (res) => {
      if (!chrome.runtime.lastError && res) {
        updateStatus(res.connected);
      }
    });
  });
}

chrome.runtime.sendMessage({ type: "CHECK_STATUS" }, (response) => {
  if (chrome.runtime.lastError || !response) {
    updateStatus(false);
    detectActiveTabMedia();
    return;
  }

  updateStatus(response.connected);
  if (response.settings) {
    interceptToggle.checked = Boolean(response.settings.interceptDownloads);
    notificationsToggle.checked = Boolean(response.settings.showNotifications);
    currentPort = response.settings.serverPort || 9654;
    serverPortInput.value = currentPort;
  }
  detectActiveTabMedia();
});

interceptToggle.addEventListener("change", () => {
  persistSettings();
});

notificationsToggle.addEventListener("change", () => {
  persistSettings();
});

saveBtn.addEventListener("click", () => {
  saveBtn.disabled = true;
  persistSettings(() => {
    const btnText = saveBtn.querySelector(".btn-text");
    if (btnText) {
      btnText.textContent = "Listo ✓";
    }

    setTimeout(() => {
      saveBtn.disabled = false;
      if (btnText) {
        btnText.textContent = "Guardar";
      }
    }, 600);
  });
});
