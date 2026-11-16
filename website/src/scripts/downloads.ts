import { assetPatterns, releaseApi, repository } from '../data/downloads';

interface ReleaseAsset {
  name: string;
  browser_download_url: string;
}

function isReleaseAsset(value: unknown): value is ReleaseAsset {
  if (typeof value !== 'object' || value === null) return false;
  const asset = value as Record<string, unknown>;
  return typeof asset.name === 'string' &&
    typeof asset.browser_download_url === 'string' &&
    asset.browser_download_url.startsWith(`${repository}/releases/download/`);
}

export function detectSystem(userAgent: string, platform: string, touchPoints: number): 'Windows' | 'Linux' | null {
  if (/Android|iPhone|iPad|iPod/i.test(userAgent) || (platform === 'MacIntel' && touchPoints > 1)) return null;
  if (/Windows/i.test(userAgent)) return 'Windows';
  if (/Linux/i.test(userAgent)) return 'Linux';
  return null;
}

function configurePrimaryDownload() {
  const system = detectSystem(navigator.userAgent, navigator.platform, navigator.maxTouchPoints);
  const primary = document.querySelector<HTMLAnchorElement>('#primary-download');
  if (!system || !primary) return;
  primary.dataset.format = system === 'Windows' ? 'installer' : 'appimage';
  primary.setAttribute('aria-label', `Download for ${system}`);
  for (const icon of document.querySelectorAll<HTMLElement>('[data-primary-icon]')) {
    icon.hidden = icon.dataset.primaryIcon !== system;
  }
}

async function resolveDownloads() {
  try {
    const response = await fetch(releaseApi);
    if (!response.ok) return;
    const release: unknown = await response.json();
    if (typeof release !== 'object' || release === null || !('assets' in release) || !Array.isArray(release.assets)) return;
    const assets = release.assets.filter(isReleaseAsset);
    let matched = 0;
    for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-format]')) {
      const pattern = assetPatterns[link.dataset.format ?? ''];
      const asset = assets.find((item) => pattern?.test(item.name));
      if (!asset) continue;
      link.href = asset.browser_download_url;
      matched += 1;
    }
    const status = document.getElementById('release-status');
    if (status && matched > 0) status.textContent = 'Direct downloads from the latest release on GitHub.';
  } catch {
    return;
  }
}

configurePrimaryDownload();
void resolveDownloads();
