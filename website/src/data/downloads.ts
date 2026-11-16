export const repository = 'https://github.com/moiCR/rDM';
export const releasePage = `${repository}/releases/latest`;
export const releaseApi = 'https://api.github.com/repos/moiCR/rDM/releases/latest';

export const applications = [
  { label: 'Windows', detail: 'Installer · x64', format: 'installer', icon: 'windows' },
  { label: 'Windows portable', detail: 'ZIP · x64', format: 'portable', icon: 'windows' },
  { label: 'Linux', detail: 'AppImage · x64', format: 'appimage', icon: 'linux' },
  { label: 'Debian / Ubuntu', detail: 'DEB · amd64', format: 'deb', icon: 'debian' },
  { label: 'Fedora', detail: 'RPM · x64', format: 'rpm', icon: 'fedora' },
] as const;

export const extensions = [
  { label: 'Chrome', icon: 'chrome', asset: 'rdm-chrome.zip' },
  { label: 'Firefox', icon: 'firefox', asset: 'rdm-firefox.zip' },
] as const;

export const assetPatterns: Record<string, RegExp> = {
  installer: /^rDM-Setup-.*-x64\.exe$/i,
  portable: /^rDM-.*-portable-x64\.zip$/i,
  appimage: /x86_64\.AppImage$/i,
  deb: /amd64\.deb$/i,
  rpm: /x86_64\.rpm$/i,
};
