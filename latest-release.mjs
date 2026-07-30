const LATEST_RELEASE_API = 'https://api.github.com/repos/pascaloon/overnotes/releases/latest';
const LATEST_RELEASE_PAGE = 'https://github.com/pascaloon/overnotes/releases/latest';

export function findWindowsInstaller(release) {
  const assets = Array.isArray(release?.assets) ? release.assets : [];
  const installer = assets.find((asset) => /setup\.exe$/i.test(asset?.name ?? ''))
    ?? assets.find((asset) => /\.exe$/i.test(asset?.name ?? ''));

  return installer?.browser_download_url ?? null;
}

export async function updateLatestDownloadLinks({ document, fetch }) {
  const links = document.querySelectorAll('[data-latest-download]');

  try {
    const response = await fetch(LATEST_RELEASE_API, {
      headers: { Accept: 'application/vnd.github+json' },
    });
    if (!response.ok) throw new Error(`GitHub API returned ${response.status}`);

    const downloadUrl = findWindowsInstaller(await response.json());
    if (!downloadUrl) throw new Error('Latest release has no Windows executable');

    links.forEach((link) => {
      link.href = downloadUrl;
    });
    return downloadUrl;
  } catch (error) {
    console.warn('Could not resolve the latest Overnotes installer:', error);
    return LATEST_RELEASE_PAGE;
  }
}

if (typeof window !== 'undefined' && typeof document !== 'undefined') {
  void updateLatestDownloadLinks({ document, fetch: window.fetch.bind(window) });
}
