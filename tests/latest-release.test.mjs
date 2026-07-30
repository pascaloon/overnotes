import test from 'node:test';
import assert from 'node:assert/strict';

import {
  findWindowsInstaller,
  updateLatestDownloadLinks,
} from '../latest-release.mjs';

const RELEASE_PAGE = 'https://github.com/pascaloon/overnotes/releases/latest';

test('findWindowsInstaller selects the versioned Windows setup asset from the latest release', () => {
  const release = {
    html_url: 'https://github.com/pascaloon/overnotes/releases/tag/v0.4.1',
    assets: [
      { name: 'checksums.txt', browser_download_url: 'https://example.test/checksums.txt' },
      { name: 'Overnotes_0.4.1_x64-setup.exe', browser_download_url: 'https://example.test/Overnotes_0.4.1_x64-setup.exe' },
      { name: 'overnotes.exe', browser_download_url: 'https://example.test/overnotes.exe' },
    ],
  };

  assert.equal(
    findWindowsInstaller(release),
    'https://example.test/Overnotes_0.4.1_x64-setup.exe',
  );
});

test('updateLatestDownloadLinks updates every marked link using GitHub latest-release metadata', async () => {
  const links = [{ href: RELEASE_PAGE }, { href: RELEASE_PAGE }, { href: RELEASE_PAGE }];
  const document = {
    querySelectorAll(selector) {
      assert.equal(selector, '[data-latest-download]');
      return links;
    },
  };
  const fetch = async () => ({
    ok: true,
    json: async () => ({
      assets: [
        { name: 'Overnotes_0.4.1_x64-setup.exe', browser_download_url: 'https://example.test/latest-installer.exe' },
      ],
    }),
  });

  const resolved = await updateLatestDownloadLinks({ document, fetch });

  assert.equal(resolved, 'https://example.test/latest-installer.exe');
  assert.deepEqual(links.map((link) => link.href), Array(3).fill(resolved));
});

test('updateLatestDownloadLinks keeps the safe latest-release fallback when metadata cannot be loaded', async () => {
  const links = [{ href: RELEASE_PAGE }];
  const document = { querySelectorAll: () => links };
  const fetch = async () => {
    throw new Error('network unavailable');
  };

  const resolved = await updateLatestDownloadLinks({ document, fetch });

  assert.equal(resolved, RELEASE_PAGE);
  assert.equal(links[0].href, RELEASE_PAGE);
});
