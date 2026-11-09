const statusIndicator = document.getElementById("status-indicator");
const statusText = document.getElementById("status-text");
const interceptToggle = document.getElementById("intercept-toggle");
const notificationsToggle = document.getElementById("notifications-toggle");
const serverPortInput = document.getElementById("server-port");
const saveBtn = document.getElementById("save-btn");

function updateStatus(connected) {
  if (connected) {
    statusIndicator.className = "status-pill connected";
    statusText.textContent = "Conectado";
  } else {
    statusIndicator.className = "status-pill disconnected";
    statusText.textContent = "Desconectado";
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
