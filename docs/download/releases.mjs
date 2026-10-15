export const REPOSITORY = 'JeremyKariukicodesnow/Jenga';
export const HISTORY = `https://github.com/${REPOSITORY}/releases`;

export function version(tag) {
  const match = /^v(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(tag ?? '');
  if (!match) return null;
  const numbers = match.slice(1, 4).map(Number);
  if (numbers.some(n => !Number.isSafeInteger(n))) return null;
  return {numbers, pre: match[4]?.split('.') ?? []};
}
function compare(a, b) {
  const av = version(a.tag_name), bv = version(b.tag_name);
  for (let i = 0; i < 3; i++) if (av.numbers[i] !== bv.numbers[i]) return av.numbers[i] > bv.numbers[i] ? -1 : 1;
  if (!av.pre.length && bv.pre.length) return -1;
  if (av.pre.length && !bv.pre.length) return 1;
  for (let i = 0; i < Math.max(av.pre.length, bv.pre.length); i++) {
    const x = av.pre[i], y = bv.pre[i];
    if (x === y) continue;
    if (x === undefined) return 1;
    if (y === undefined) return -1;
    const numericX = /^\d+$/.test(x), numericY = /^\d+$/.test(y);
    if (numericX && numericY) return BigInt(x) > BigInt(y) ? -1 : 1;
    if (numericX !== numericY) return numericX ? 1 : -1;
    return x > y ? -1 : 1;
  }
  return Date.parse(b.published_at) - Date.parse(a.published_at);
}
export function asset(release, names) {
  for (const name of names) {
    const found = release.assets?.find(a => a.name === name && a.state === 'uploaded' && a.size > 0);
    if (!found) continue;
    try {
      const url = new URL(found.browser_download_url);
      const expected = `/${REPOSITORY}/releases/download/${encodeURIComponent(release.tag_name)}/${encodeURIComponent(name)}`;
      if (url.origin === 'https://github.com' && url.pathname === expected && !url.search && !url.hash) return {...found, url: url.href};
    } catch { /* Untrusted/malformed release metadata must not become a link. */ }
  }
  return null;
}
export function select(releases) {
  const publicReleases = releases.filter(r => !r.draft && r.published_at && version(r.tag_name)).sort(compare);
  if (!publicReleases.length) return {state: 'empty'};
  const release = publicReleases[0], tag = release.tag_name.slice(1);
  const installer = asset(release, ['Jenga-windows-x64-Setup.exe', `Jenga-${tag}-windows-x64-Setup.exe`]);
  // Never silently substitute an old EXE when the newest release is incomplete.
  return {state: installer ? 'available' : 'missing', release, installer,
    portable: asset(release, ['Jenga-windows-x64-portable.zip', `Jenga-${tag}-windows-x64-portable.zip`]),
    checksums: asset(release, ['SHA256SUMS'])};
}
export async function load(fetcher = fetch, signal) {
  const all = [];
  for (let page = 1; page <= 10; page++) {
    const response = await fetcher(`https://api.github.com/repos/${REPOSITORY}/releases?per_page=100&page=${page}`, {signal, credentials: 'omit', cache: 'no-store', headers: {Accept: 'application/vnd.github+json'}});
    if (!response.ok) throw new Error(`Release lookup failed (${response.status}). Use All releases; GitHub may be unavailable or rate-limiting this browser.`);
    const rows = await response.json();
    if (!Array.isArray(rows)) throw new Error('Unexpected release response. Use All releases.');
    all.push(...rows);
    if (!/rel="next"/.test(response.headers.get('link') ?? '')) return select(all);
  }
  throw new Error('Release history is too large for this lookup. Choose a release manually.');
}
