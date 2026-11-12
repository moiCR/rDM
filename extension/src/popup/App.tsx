import React, { useEffect, useState } from "react";

import { resolveLanguage, translations, type Language } from "../i18n";

interface Settings {
  language: Language;
  interceptDownloads: boolean;
  serverHost: string;
  serverPort: number;
  showNotifications: boolean;
}

interface YouTubeFormat {
  category: "video" | "audio";
  url: string;
  filename: string;
  quality: string;
  ext: string;
  size_bytes: number;
  video_size: number;
  audio_url: string | null;
  audio_size: number;
  mp3_bitrate: number | null;
}

interface SniffedMedia {
  url: string;
  filename: string;
  ext: string;
  sizeBytes: number;
  sizeFormatted: string | null;
  referer?: string | null;
}

function formatSize(bytes: number): string {
  if (!bytes || bytes <= 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function extractYouTubeVideoId(url: string | null): string | null {
  if (!url) return null;
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.toLowerCase();
    if (host.endsWith("youtube.com")) {
      const v = parsed.searchParams.get("v");
      if (v && v.length === 11) return v;
      if (parsed.pathname.startsWith("/shorts/")) {
        const id = parsed.pathname.split("/")[2];
        if (id && id.length === 11) return id;
      }
    } else if (host === "youtu.be") {
      const id = parsed.pathname.slice(1).split("/")[0];
      if (id && id.length === 11) return id;
    }
  } catch {
    return null;
  }
  return null;
}

export function App() {
  const [view, setView] = useState<"home" | "settings">("home");
  const [formatTab, setFormatTab] = useState<"video" | "audio">("video");
  const [connected, setConnected] = useState<boolean>(false);
  const [settings, setSettings] = useState<Settings>({
    language: "en",
    interceptDownloads: true,
    serverHost: "127.0.0.1",
    serverPort: 9654,
    showNotifications: true,
  });
  const [portInput, setPortInput] = useState<string>("9654");
  const [saveState, setSaveState] = useState<"save" | "saving" | "saved">("save");
  const text = translations[settings.language];

  useEffect(() => {
    document.documentElement.lang = settings.language;
  }, [settings.language]);

  const [tabUrl, setTabUrl] = useState<string | null>(null);
  const [tabTitle, setTabTitle] = useState<string | null>(null);
  const [ytFormats, setYtFormats] = useState<YouTubeFormat[]>([]);
  const [sniffedMedia, setSniffedMedia] = useState<SniffedMedia[]>([]);
  const [loadingMedia, setLoadingMedia] = useState<boolean>(true);
  const [downloadingKey, setDownloadingKey] = useState<string | null>(null);

  useEffect(() => {
    chrome.runtime.sendMessage({ type: "CHECK_STATUS" }, (response) => {
      if (chrome.runtime.lastError || !response) {
        setConnected(false);
        return;
      }
      setConnected(Boolean(response.connected));
      const loadedSettings: Settings = {
        language: resolveLanguage(response.settings?.language),
        interceptDownloads: response.settings?.interceptDownloads ?? true,
        serverHost: response.settings?.serverHost ?? "127.0.0.1",
        serverPort: response.settings?.serverPort ?? 9654,
        showNotifications: response.settings?.showNotifications ?? true,
      };
      setSettings(loadedSettings);
      setPortInput(String(loadedSettings.serverPort));

      chrome.runtime.sendMessage({ type: "GET_TAB_MEDIA" }, async (tabRes) => {
        if (chrome.runtime.lastError || !tabRes) {
          setLoadingMedia(false);
          return;
        }
        const activeUrl: string | null = tabRes.tabUrl || null;
        const activeTitle: string | null = tabRes.tabTitle || null;
        const captured: SniffedMedia[] = Array.isArray(tabRes.media) ? tabRes.media : [];

        setTabUrl(activeUrl);
        setTabTitle(activeTitle);
        setSniffedMedia(captured);

        const ytVideoId = extractYouTubeVideoId(activeUrl);
        if (ytVideoId) {
          try {
            const resp = await fetch(
              `http://${loadedSettings.serverHost}:${loadedSettings.serverPort}/api/v1/youtube?v=${encodeURIComponent(ytVideoId)}`
            );
            if (resp.ok) {
              const data = await resp.json();
              if (Array.isArray(data?.formats) && data.formats.length > 0) {
                setYtFormats(data.formats);
              }
            }
          } catch {
            // Ignore connection error; fallback to offline state
          }
        } else if (captured.length === 0) {
          setView("settings");
        }
        setLoadingMedia(false);
      });
    });
  }, []);

  const handleDownloadYouTube = (fmt: YouTubeFormat) => {
    const key = `${fmt.category}-${fmt.quality}`;
    setDownloadingKey(key);
    chrome.runtime.sendMessage(
      {
        type: "DOWNLOAD_MEDIA",
        url: fmt.url,
        filename: fmt.filename,
        referer: tabUrl,
        videoSize: fmt.video_size || null,
        audioUrl: fmt.audio_url || null,
        audioSize: fmt.audio_size || null,
        mp3Bitrate: fmt.mp3_bitrate || null,
      },
      (res) => {
        if (res && res.success) {
          setDownloadingKey(`done-${key}`);
          setTimeout(() => window.close(), 350);
        } else {
          setDownloadingKey(`err-${key}`);
          setTimeout(() => setDownloadingKey(null), 1500);
        }
      }
    );
  };

  const handleDownloadSniffed = (item: SniffedMedia, idx: number) => {
    const key = `sniff-${idx}`;
    setDownloadingKey(key);
    chrome.runtime.sendMessage(
      {
        type: "DOWNLOAD_MEDIA",
        url: item.url,
        filename: item.filename,
        referer: item.referer || tabUrl,
      },
      (res) => {
        if (res && res.success) {
          setDownloadingKey(`done-${key}`);
          setTimeout(() => window.close(), 350);
        } else {
          setDownloadingKey(`err-${key}`);
          setTimeout(() => setDownloadingKey(null), 1500);
        }
      }
    );
  };

  const handleToggleSetting = (key: "interceptDownloads" | "showNotifications", val: boolean) => {
    const updated = { ...settings, [key]: val };
    setSettings(updated);
    chrome.runtime.sendMessage({ type: "SAVE_SETTINGS", settings: updated });
  };

  const handleSaveSettings = () => {
    const parsedPort = parseInt(portInput, 10);
    const validPort =
      parsedPort >= 1024 && parsedPort <= 65535 ? parsedPort : 9654;
    const updated: Settings = {
      ...settings,
      serverPort: validPort,
    };
    setSettings(updated);
    setPortInput(String(validPort));
    setSaveState("saving");
    chrome.runtime.sendMessage({ type: "SAVE_SETTINGS", settings: updated }, () => {
      chrome.runtime.sendMessage({ type: "CHECK_STATUS" }, (res) => {
        setConnected(Boolean(res?.connected));
        setSaveState("saved");
        setTimeout(() => setSaveState("save"), 1100);
      });
    });
  };

  const videoFormats = ytFormats.filter((f) => f.category === "video");
  const audioFormats = ytFormats.filter((f) => f.category === "audio");
  const hasAnyMedia = ytFormats.length > 0 || sniffedMedia.length > 0;
  const videoDisplayTitle =
    (ytFormats[0]?.filename
      ? ytFormats[0].filename.replace(/\.(mp4|mp3)$/i, "")
      : tabTitle) || text.detectedVideo;

  return (
    <>
      <div className="ambient-glow" />
      <main className="modal-card">
        {view === "home" ? (
          <>
            <div className="top-bar">
              <div className="top-bar-left">
                <span className="top-title" title={videoDisplayTitle}>
                  {hasAnyMedia ? videoDisplayTitle : "rDM"}
                </span>
              </div>
              <button
                className="icon-btn"
                title={text.settings}
                onClick={() => setView("settings")}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="3" />
                  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
                </svg>
              </button>
            </div>

            {loadingMedia ? (
              <div className="empty-state">{text.analyzing}</div>
            ) : ytFormats.length > 0 ? (
              <>
                <div className="pill-tabs">
                  <button
                    className={`pill-tab ${formatTab === "video" ? "active" : ""}`}
                    onClick={() => setFormatTab("video")}
                  >
                    {text.video}
                  </button>
                  <button
                    className={`pill-tab ${formatTab === "audio" ? "active" : ""}`}
                    onClick={() => setFormatTab("audio")}
                  >
                    {text.audio}
                  </button>
                </div>

                <div className="quality-list">
                  {(formatTab === "video" ? videoFormats : audioFormats).map((fmt) => {
                    const key = `${fmt.category}-${fmt.quality}`;
                    const isSending = downloadingKey === key;
                    const isDone = downloadingKey === `done-${key}`;
                    const isErr = downloadingKey === `err-${key}`;
                    const sizeText = formatSize(fmt.size_bytes);

                    return (
                      <div className="quality-row" key={key}>
                        <div className="quality-info">
                          <div className="quality-label-row">
                            <span className="quality-title">{fmt.quality}</span>
                            <span className="quality-badge">{fmt.ext}</span>
                          </div>
                          <span className="quality-meta">
                            {sizeText
                              ? `${fmt.ext} • ${sizeText}`
                              : fmt.ext}
                          </span>
                        </div>
                        <button
                          className="action-btn"
                          disabled={isSending || isDone}
                          onClick={() => handleDownloadYouTube(fmt)}
                        >
                          {isSending
                            ? "..."
                            : isDone
                              ? text.sent
                              : isErr
                                ? text.error
                                : text.download}
                        </button>
                      </div>
                    );
                  })}
                </div>
              </>
            ) : sniffedMedia.length > 0 ? (
              <div className="quality-list">
                {sniffedMedia.map((item, idx) => {
                  const key = `sniff-${idx}`;
                  const isSending = downloadingKey === key;
                  const isDone = downloadingKey === `done-${key}`;
                  const isErr = downloadingKey === `err-${key}`;

                  return (
                    <div className="quality-row" key={key}>
                      <div className="quality-info">
                        <div className="quality-label-row">
                          <span className="quality-title" title={item.filename}>
                            {item.filename}
                          </span>
                          <span className="quality-badge">{item.ext}</span>
                        </div>
                        <span className="quality-meta">
                          {item.sizeFormatted
                            ? `${item.ext} • ${item.sizeFormatted}`
                            : item.ext}
                        </span>
                      </div>
                      <button
                        className="action-btn"
                        disabled={isSending || isDone}
                        onClick={() => handleDownloadSniffed(item, idx)}
                      >
                        {isSending
                          ? "..."
                          : isDone
                            ? text.sent
                            : isErr
                              ? text.error
                              : text.download}
                      </button>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="empty-state">
                {text.noVideos}
              </div>
            )}

            <div className="divider" />
            <div className="footer-row">
              <div className={`status-indicator ${connected ? "online" : "offline"}`}>
                <span className="status-dot" />
                <span>{connected ? text.connected : text.disconnected}</span>
              </div>
            </div>
          </>
        ) : (
          <>
            <div className="top-bar">
              <div className="top-bar-left">
                {hasAnyMedia && (
                  <button
                    className="icon-btn"
                    title={text.back}
                    onClick={() => setView("home")}
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="15 18 9 12 15 6" />
                    </svg>
                  </button>
                )}
                <span className="top-title">{text.settings}</span>
              </div>
            </div>

            <div className="setting-row">
              <label className="setting-title" htmlFor="language">{text.language}</label>
              <select
                id="language"
                className="language-select"
                value={settings.language}
                onChange={(event) => {
                  const updated = { ...settings, language: resolveLanguage(event.target.value) };
                  setSettings(updated);
                  chrome.runtime.sendMessage({ type: "SAVE_SETTINGS", settings: updated });
                }}
              >
                <option value="en">English</option>
                <option value="es">Español</option>
              </select>
            </div>

            <div className="divider" />

            <div className="setting-row">
              <div className="setting-info">
                <span className="setting-title">{text.intercept}</span>
                <span className="setting-desc">{text.interceptDescription}</span>
              </div>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={settings.interceptDownloads}
                  onChange={(e) =>
                    handleToggleSetting("interceptDownloads", e.target.checked)
                  }
                />
                <span className="slider" />
              </label>
            </div>

            <div className="divider" />

            <div className="setting-row">
              <div className="setting-info">
                <span className="setting-title">{text.notifications}</span>
                <span className="setting-desc">{text.notificationsDescription}</span>
              </div>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={settings.showNotifications}
                  onChange={(e) =>
                    handleToggleSetting("showNotifications", e.target.checked)
                  }
                />
                <span className="slider" />
              </label>
            </div>

            <div className="divider" />

            <div className="setting-row">
              <div className="setting-info">
                <span className="setting-title">{text.port}</span>
                <span className="setting-desc">{text.portDescription}</span>
              </div>
              <div className="port-box">
                <span className="port-prefix">:</span>
                <input
                  type="number"
                  min={1024}
                  max={65535}
                  value={portInput}
                  onChange={(e) => setPortInput(e.target.value)}
                />
              </div>
            </div>

            <div className="divider" />

            <div className="footer-row">
              <div className={`status-indicator ${connected ? "online" : "offline"}`}>
                <span className="status-dot" />
                <span>{connected ? text.connected : text.disconnected}</span>
              </div>
              <button className="action-btn" onClick={handleSaveSettings}>
                {saveState === "saving" ? "..." : text[saveState]}
              </button>
            </div>
          </>
        )}
      </main>
    </>
  );
}
