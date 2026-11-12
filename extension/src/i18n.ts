export type Language = "en" | "es";

export function resolveLanguage(value: unknown): Language {
  return value === "es" ? "es" : "en";
}

export const translations = {
  "en": {
    "settings": "Settings",
    "save": "Save",
    "saved": "Saved",
    "detectedVideo": "Video detected",
    "analyzing": "Checking available qualities...",
    "video": "Video MP4",
    "audio": "Audio MP3",
    "sent": "Sent",
    "error": "Error",
    "download": "Download",
    "noVideos": "No videos detected in this tab.",
    "connected": "Connected",
    "disconnected": "Disconnected",
    "back": "Back to qualities",
    "intercept": "Intercept downloads",
    "interceptDescription": "Automatically capture browser downloads",
    "notifications": "Notifications",
    "notificationsDescription": "Notify when a download starts",
    "port": "Connection port",
    "portDescription": "Local rDM server",
    "language": "Language",
    "downloadWith": "Download with rDM",
    "captured": "Download captured:",
    "file": "file",
    "unavailable": "rDM unavailable",
    "connectionError": "Could not connect to rDM. Is the app running?",
    "extensionName": "rDM - Download Manager Integration",
    "extensionDescription": "Official rDM integration to capture and accelerate browser downloads."
  },
  "es": {
    "settings": "Ajustes",
    "save": "Guardar",
    "saved": "Guardado",
    "detectedVideo": "Video detectado",
    "analyzing": "Analizando calidades disponibles...",
    "video": "Video MP4",
    "audio": "Audio MP3",
    "sent": "Enviado",
    "error": "Error",
    "download": "Descargar",
    "noVideos": "No se detectaron videos en esta pestaña.",
    "connected": "Conectado",
    "disconnected": "Desconectado",
    "back": "Volver a calidades",
    "intercept": "Interceptar descargas",
    "interceptDescription": "Captura automática del navegador",
    "notifications": "Notificaciones",
    "notificationsDescription": "Avisar al iniciar descarga",
    "port": "Puerto de enlace",
    "portDescription": "Servidor local de rDM",
    "language": "Idioma",
    "downloadWith": "Descargar con rDM",
    "captured": "Descarga capturada:",
    "file": "archivo",
    "unavailable": "rDM no disponible",
    "connectionError": "No se pudo conectar con rDM. ¿Está la aplicación abierta?",
    "extensionName": "rDM - Integración del gestor de descargas",
    "extensionDescription": "Integración oficial de rDM para interceptar y acelerar descargas desde el navegador."
  }
};
