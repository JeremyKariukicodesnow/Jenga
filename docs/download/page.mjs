import {load, HISTORY} from './releases.mjs';
const get = id => document.getElementById(id);
const share = 'https://jeremykariukicodesnow.github.io/Jenga/download/?start=1';
get('copy').addEventListener('click', async () => {
  try { await navigator.clipboard.writeText(share); get('copy-status').textContent = 'Link copied.'; }
  catch { get('copy-status').textContent = share; }
});
const controller = new AbortController();
const timer = setTimeout(() => controller.abort(), 12000);
try {
  const model = await load(fetch, controller.signal);
  if (model.state === 'empty') {
    get('download-heading').textContent = 'Public installer not available yet';
    get('status').textContent = 'The 0.8.0 Windows binaries remain in draft pending the redistribution review. Source code and release status are public.';
  } else {
    const {release} = model;
    get('download-heading').textContent = `Jenga ${release.tag_name}`;
    get('channel').textContent = release.prerelease || release.tag_name.includes('-') ? 'EARLY-ACCESS / PRERELEASE' : 'PUBLISHED RELEASE';
    get('notes').href = `${HISTORY}/tag/${encodeURIComponent(release.tag_name)}`;
    get('notes').hidden = false;
    if (model.state === 'missing') {
      get('status').textContent = 'The newest release has no valid Windows installer attached. An older version will not be substituted automatically. Choose an older release explicitly if needed.';
    } else {
      const button = get('installer');
      button.href = model.installer.url;
      button.removeAttribute('aria-disabled');
      get('status').textContent = `Ready: ${model.installer.name} (${(model.installer.size / 1e6).toFixed(1)} MB). This downloads Setup.exe directly.`;
      for (const name of ['portable', 'checksums']) if (model[name]) { get(name).href = model[name].url; get(name).hidden = false; }
      if (new URLSearchParams(location.search).get('start') === '1') location.assign(model.installer.url);
    }
  }
} catch (error) {
  get('download-heading').textContent = 'Choose a release manually';
  get('status').textContent = error.name === 'AbortError' ? 'GitHub took too long to respond. All releases is still available.' : error.message;
} finally { clearTimeout(timer); }
