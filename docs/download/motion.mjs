const button = document.getElementById('motion');
const preference = matchMedia('(prefers-reduced-motion: reduce)');
let paused = false;
try { paused = localStorage.getItem('jenga-download-motion') === 'paused'; } catch { /* Storage may be unavailable. */ }
function render() {
  const enabled = !preference.matches && !paused;
  document.documentElement.dataset.motion = enabled ? 'on' : 'off';
  button.hidden = false;
  button.disabled = preference.matches;
  button.textContent = preference.matches ? 'Reduced motion' : enabled ? 'Pause animation' : 'Enable animation';
  button.setAttribute('aria-pressed', String(enabled));
}
button.addEventListener('click', () => {
  paused = !paused;
  try { localStorage.setItem('jenga-download-motion', paused ? 'paused' : 'on'); } catch { /* Preference still applies for this visit. */ }
  render();
});
preference.addEventListener('change', render);
render();
