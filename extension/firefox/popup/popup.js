const statusIndicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const interceptToggle = document.getElementById("intercept-toggle");
const notificationsToggle = document.getElementById("notifications-toggle");
const serverPortInput = document.getElementById("server-port");
const saveBtn = document.getElementById("save-btn");
const mediaSection = document.getElementById("media-section");
const mediaDivider = document.getElementById("media-divider");
const mediaList = document.getElementById("media-list");

function updateStatus(connected) {
  if (connected) {
    statusIndicator.className = "status-pill connected";
    statusText.textContent = "Conectado";
  } else {
    statusIndicator.className = "status-pill disconnected";
    statusText.textContent = "Desconectado";
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
    const sizeLabel = item.sizeFormatted || "Video detectado";
    meta.textContent = `${item.ext || "VIDEO"} • ${sizeLabel}`;

    info.appendChild(title);
    info.appendChild(meta);

    const btn = document.createElement("button");
    btn.className = "save-btn";
    btn.textContent = "Descargar";

    btn.addEventListener("click", () => {
      btn.disabled = true;
      btn.textContent = "Enviando...";
      chrome.runtime.sendMessage(
        {
          type: "DOWNLOAD_MEDIA",
          url: item.url,
          filename: item.filename,
          referer: item.referer || fallbackReferer || null,
        },
        (res) => {
          if (!chrome.runtime.lastError && res && res.success) {
            btn.textContent = "Enviado ✓";
          } else {
            btn.textContent = "Error";
          }
          setTimeout(() => {
            btn.disabled = false;
            btn.textContent = "Descargar";
          }, 1200);
        }
      );
    });

    row.appendChild(info);
    row.appendChild(btn);
    mediaList.appendChild(row);
  }
}

function persistSettings(callback) {
  const port = parseInt(serverPortInput.value, 10) || 9654;
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
    return;
  }

  updateStatus(response.connected);
  if (response.settings) {
    interceptToggle.checked = Boolean(response.settings.interceptDownloads);
    notificationsToggle.checked = Boolean(response.settings.showNotifications);
    serverPortInput.value = response.settings.serverPort || 9654;
  }
});

chrome.runtime.sendMessage({ type: "GET_TAB_MEDIA" }, (response) => {
  if (!chrome.runtime.lastError && response && Array.isArray(response.media)) {
    renderMediaItems(response.media, response.tabUrl);
  }
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
