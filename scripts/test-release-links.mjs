import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {select, load, REPOSITORY} from '../docs/download/releases.mjs';
function release(tag, options = {}) {
  return {tag_name: tag, published_at: '2026-10-15T00:00:00Z', draft: false, prerelease: false, assets: [{name: 'Jenga-windows-x64-Setup.exe', state: 'uploaded', size: 123, browser_download_url: `https://github.com/${REPOSITORY}/releases/download/${tag}/Jenga-windows-x64-Setup.exe`}], ...options};
}
test('latest public version includes previews but excludes drafts; old links remain exact', () => {
  const old = release('v0.7.1'), latest = release('v0.8.0', {prerelease: true});
  const model = select([old, release('v0.9.0', {draft: true}), latest]);
  assert.equal(model.release.tag_name, 'v0.8.0');
  assert.match(model.installer.url, /\/v0\.8\.0\//);
  assert.match(old.assets[0].browser_download_url, /\/v0\.7\.1\//);
});
test('numeric version ordering and prerelease precedence', () => {
  assert.equal(select([release('v0.9.0'), release('v0.10.0')]).release.tag_name, 'v0.10.0');
  assert.equal(select([release('v1.0.0-rc.10'), release('v1.0.0-rc.2')]).release.tag_name, 'v1.0.0-rc.10');
  assert.equal(select([release('v1.0.0-rc.10'), release('v1.0.0')]).release.tag_name, 'v1.0.0');
});
test('no stale fallback when newest installer is missing', () => assert.equal(select([release('v0.7.1'), release('v0.8.0', {assets: []})]).state, 'missing'));
test('empty or draft-only history offers no fake download', () => {
  assert.equal(select([]).state, 'empty');
  assert.equal(select([release('v0.8.0', {draft: true})]).state, 'empty');
});
test('only expected GitHub asset URL is accepted', () => {
  const r = release('v0.8.0');
  r.assets[0].browser_download_url = 'https://evil.example/Jenga-windows-x64-Setup.exe';
  assert.equal(select([r]).state, 'missing');
  r.assets[0].browser_download_url = `https://github.com/${REPOSITORY}/releases/download/v0.7.1/Jenga-windows-x64-Setup.exe`;
  assert.equal(select([r]).state, 'missing');
});
test('versioned installers are supported without the new alias', () => {
  const r = release('v0.8.0');
  r.assets[0].name = 'Jenga-0.8.0-windows-x64-Setup.exe';
  r.assets[0].browser_download_url = `https://github.com/${REPOSITORY}/releases/download/v0.8.0/${r.assets[0].name}`;
  assert.equal(select([r]).state, 'available');
});
test('pagination is included when selecting highest published version', async () => {
  let page = 0;
  const model = await load(async () => ({ok: true, headers: {get: () => page === 1 ? '<ignored>; rel="next"' : null}, json: async () => ++page === 1 ? [release('v0.7.1')] : [release('v0.8.0')]}));
  assert.equal(model.release.tag_name, 'v0.8.0');
  assert.equal(page, 2);
});
test('API errors do not become download links', async () => {
  await assert.rejects(load(async () => ({ok: false, status: 403})), /Release lookup failed/);
});

async function runPage(rows, ok = true) {
  const html = await readFile(new URL('../docs/download/index.html', import.meta.url), 'utf8');
  const elements = new Map([...html.matchAll(/\bid="([^"]+)"/g)].map(m => [m[1], {
    textContent: '', hidden: false, addEventListener() {}, removeAttribute() {},
  }]));
  const previous = {document: globalThis.document, location: globalThis.location, fetch: globalThis.fetch};
  let redirect;
  globalThis.document = {getElementById(id) { assert.ok(elements.has(id), `Missing HTML element ${id}`); return elements.get(id); }};
  globalThis.location = {search: '?start=1', assign(url) { redirect = url; }};
  globalThis.fetch = async () => ({ok, status: ok ? 200 : 403, headers: {get: () => null}, json: async () => rows});
  try {
    await import(`../docs/download/page.mjs?fixture=${Math.random()}`);
    return {elements, redirect};
  } finally {
    Object.assign(globalThis, previous);
  }
}
test('download page starts the exact newest EXE and labels early access', async () => {
  const page = await runPage([release('v0.7.1'), release('v0.8.0', {prerelease: true})]);
  assert.match(page.redirect, /\/v0\.8\.0\/Jenga-windows-x64-Setup\.exe$/);
  assert.equal(page.elements.get('installer').href, page.redirect);
  assert.match(page.elements.get('channel').textContent, /EARLY-ACCESS/);
});
test('download page cannot redirect a draft or missing installer', async () => {
  for (const rows of [[release('v0.8.0', {draft: true})], [release('v0.8.0', {assets: []})]]) {
    const page = await runPage(rows);
    assert.equal(page.redirect, undefined);
    assert.equal(page.elements.get('installer').href, undefined);
  }
});
test('download page leaves a manual route when GitHub fails', async () => {
  const page = await runPage([], false);
  assert.equal(page.redirect, undefined);
  assert.match(page.elements.get('status').textContent, /Release lookup failed/);
});

test('historical schematic-only downloads disclose missing PCB and 3D features', async () => {
  const page = await runPage([release('v0.6.2')]);
  assert.match(page.elements.get('details').textContent, /schematic-only/);
  assert.match(page.elements.get('details').textContent, /not included/);
});
