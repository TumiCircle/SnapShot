const { invoke } = window.__TAURI__.core;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;
const { listen } = window.__TAURI__.event;

const $ = (sel) => document.querySelector(sel);
let currentCfg = null;
let isCapturing = false;
const mainWindow = getCurrentWebviewWindow();

(function buildStars() {
  const sf = $('#starfield');
  if (!sf) return;
  for (let i = 0; i < 50; i++) {
    const s = document.createElement('div');
    s.className = 'star';
    s.style.left = Math.random() * 100 + '%';
    s.style.top = Math.random() * 100 + '%';
    s.style.opacity = Math.random() * 0.6 + 0.3;
    if (Math.random() > 0.6) s.classList.add('blink');
    sf.appendChild(s);
  }
})();

let activeMode = 'image';
let activeFmt = 'png';
let activeMotionFmt = 'gif';

const modeBlocks = document.querySelectorAll('.mode-block');
modeBlocks.forEach(block => {
  const head = block.querySelector('.mode-head');
  head.addEventListener('click', () => {
    activeMode = block.dataset.mode;
    modeBlocks.forEach(b => b.classList.remove('active'));
    block.classList.add('active');
    updateTags();
    saveConfig();
  });
});

function updateTags() {
  const motionTag = document.getElementById('motion-tag');
  const videoTag = document.getElementById('video-tag');
  if (motionTag) motionTag.textContent = activeMotionFmt.toUpperCase();
  if (videoTag) videoTag.textContent = 'AVI';
}

const fmtBtns = document.querySelectorAll('#fmt-picker button');
const jpgQualityRow = document.getElementById('jpg-quality-row');
fmtBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    fmtBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    activeFmt = btn.dataset.fmt;
    jpgQualityRow.style.opacity = btn.dataset.fmt === 'jpg' ? '1' : '0.45';
    saveConfig();
  });
});
jpgQualityRow.style.opacity = '0.45';

const motionFmtBtns = document.querySelectorAll('#motion-fmt-picker button');
motionFmtBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    motionFmtBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    activeMotionFmt = btn.dataset.mfmt;
    updateTags();
    saveConfig();
  });
});

document.getElementById('quality').addEventListener('input', (e) => {
  document.getElementById('qv').textContent = e.target.value;
  saveConfig();
});
document.getElementById('toast-dur').addEventListener('input', (e) => {
  document.getElementById('toast-dur-label').textContent = (e.target.value / 1000).toFixed(1) + 's';
  saveConfig();
});
['vid-dur','vid-fps','mot-dur','mot-fps'].forEach(id => {
  const el = document.getElementById(id);
  if (el) el.addEventListener('change', saveConfig);
});
document.getElementById('sound-on').addEventListener('change', saveConfig);
document.getElementById('save-dir').addEventListener('change', saveConfig);

let recordingHotkey = false;
const hotkeyInput = document.getElementById('hotkey');
const hotkeyWrap = document.getElementById('hotkey-wrap');
hotkeyInput.addEventListener('click', () => {
  recordingHotkey = true;
  hotkeyWrap.classList.add('recording');
  hotkeyInput.value = 'RECORDING... press combo';
  hotkeyInput.style.color = 'var(--accent-pink)';
});
window.addEventListener('keydown', (e) => {
  if (!recordingHotkey) return;
  e.preventDefault();
  const parts = [];
  if (e.ctrlKey) parts.push('Control');
  if (e.metaKey) parts.push('Command');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  const key = e.key;
  const modifiers = ['Control','Meta','Alt','Shift'];
  if (!modifiers.includes(key)) {
    let k = key;
    if (k === ' ') k = 'Space';
    if (k.length === 1) k = k.toUpperCase();
    parts.push(k);
    if (parts.length < 2) {
      hotkeyInput.value = parts.join('+') + ' (need modifier)';
      return;
    }
    const accelerator = parts.join('+').replace('Control','CommandOrControl');
    hotkeyInput.value = accelerator;
    recordingHotkey = false;
    hotkeyWrap.classList.remove('recording');
    hotkeyInput.style.color = '';
    saveConfig();
  }
});

document.getElementById('btn-close').addEventListener('click', () => { mainWindow.hide(); });
document.getElementById('btn-min').addEventListener('click', () => { mainWindow.minimize(); });

document.getElementById('btn-browse').addEventListener('click', async () => {
  try {
    const selected = await invoke('select_folder');
    if (selected) {
      document.getElementById('save-dir').value = selected;
      saveConfig();
    }
  } catch(e) { console.error(e); }
});
document.getElementById('btn-open').addEventListener('click', () => {
  const path = document.getElementById('save-dir').value;
  if (path) invoke('open_folder', { path });
});

const snapBtn = document.getElementById('btn-snap-test');
snapBtn.addEventListener('click', async () => {
  if (isCapturing) return;
  isCapturing = true;
  snapBtn.textContent = '...';
  snapBtn.classList.add('recording');
  try {
    await invoke('take_screenshot');
  } catch(e) {
    console.error('Screenshot failed:', e);
    isCapturing = false;
    snapBtn.textContent = 'SNAP!';
    snapBtn.classList.remove('recording');
    setStatus('ERROR');
    setTimeout(() => setStatus('READY'), 2000);
  }
});

function resetSnapBtn() {
  isCapturing = false;
  snapBtn.textContent = 'SNAP!';
  snapBtn.classList.remove('recording');
}

document.querySelectorAll('input[type=number]').forEach(inp => {
  inp.addEventListener('keydown', e => {
    const allowed = [8,9,27,13,37,39,36,35,46];
    if (allowed.indexOf(e.keyCode) !== -1) return;
    if ((e.keyCode < 48 || e.keyCode > 57) && (e.keyCode < 96 || e.keyCode > 105)) e.preventDefault();
  });
});

function setStatus(text, color) {
  const st = document.getElementById('status-text');
  const sd = document.getElementById('status-dot');
  if (st) st.textContent = text;
  if (sd) {
    sd.classList.remove('rec');
    if (color === 'rec') sd.classList.add('rec');
  }
}

function showRecOverlay(show, elapsedSec) {
  const overlay = document.getElementById('rec-overlay');
  const timer = document.getElementById('rec-timer');
  if (!overlay) return;
  if (show) {
    overlay.classList.add('visible');
    if (timer) timer.textContent = (elapsedSec|0) + 's';
  } else {
    overlay.classList.remove('visible');
  }
}

listen('recording-started', (e) => {
  isCapturing = true;
  setStatus('RECORDING', 'rec');
  snapBtn.textContent = 'REC';
  snapBtn.classList.add('recording');
  showRecOverlay(true, 0);
});

listen('recording-tick', (e) => {
  const data = e.payload || {};
  const elapsed = data.elapsed_sec || 0;
  const timer = document.getElementById('rec-timer');
  if (timer) timer.textContent = elapsed.toFixed(1) + 's';
  setStatus('REC ' + elapsed.toFixed(1) + 's', 'rec');
});

listen('recording-stopped', (e) => {
  showRecOverlay(false);
});

listen('capture-complete', (e) => {
  setStatus('CAPTURED');
  resetSnapBtn();
  setTimeout(() => setStatus('READY'), 2000);
});

listen('capture-error', (e) => {
  setStatus('ERROR');
  resetSnapBtn();
  setTimeout(() => setStatus('READY'), 2000);
});

function collectConfig() {
  return {
    mode: activeMode,
    image_format: activeFmt,
    jpeg_quality: parseInt(document.getElementById('quality').value) || 90,
    video_duration: parseInt(document.getElementById('vid-dur').value) || 10,
    video_fps: parseInt(document.getElementById('vid-fps').value) || 20,
    motion_duration: parseInt(document.getElementById('mot-dur').value) || 3,
    motion_fps: parseInt(document.getElementById('mot-fps').value) || 15,
    motion_format: activeMotionFmt,
    save_dir: document.getElementById('save-dir').value || '',
    hotkey: hotkeyInput.value || 'CommandOrControl+Shift+S',
    sound_enabled: document.getElementById('sound-on').checked,
    toast_duration: parseInt(document.getElementById('toast-dur').value) || 2500,
  };
}

let saveTimeout = null;
function saveConfig() {
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(async () => {
    const cfg = collectConfig();
    try {
      await invoke('save_config', { config: cfg });
    } catch(e) { console.error('Save config failed:', e); }
  }, 300);
}

async function loadConfig() {
  try {
    const cfg = await invoke('get_config');
    currentCfg = cfg;
    activeMode = cfg.mode || 'image';
    activeFmt = (cfg.image_format && cfg.image_format.toLowerCase() === 'jpg') ? 'jpg' : 'png';
    activeMotionFmt = (cfg.motion_format && cfg.motion_format.toLowerCase() === 'jpg') ? 'jpg' : 'gif';

    modeBlocks.forEach(b => b.classList.remove('active'));
    const ab = document.querySelector(`.mode-block[data-mode="${activeMode}"]`);
    if (ab) ab.classList.add('active');

    fmtBtns.forEach(b => b.classList.remove('active'));
    const afb = document.querySelector(`#fmt-picker button[data-fmt="${activeFmt}"]`);
    if (afb) afb.classList.add('active');
    jpgQualityRow.style.opacity = activeFmt === 'jpg' ? '1' : '0.45';

    motionFmtBtns.forEach(b => b.classList.remove('active'));
    const amfb = document.querySelector(`#motion-fmt-picker button[data-mfmt="${activeMotionFmt}"]`);
    if (amfb) amfb.classList.add('active');
    updateTags();

    document.getElementById('quality').value = cfg.jpeg_quality || 90;
    document.getElementById('qv').textContent = cfg.jpeg_quality || 90;
    document.getElementById('vid-dur').value = cfg.video_duration || 10;
    document.getElementById('vid-fps').value = cfg.video_fps || 20;
    document.getElementById('mot-dur').value = cfg.motion_duration || 3;
    document.getElementById('mot-fps').value = cfg.motion_fps || 15;
    document.getElementById('save-dir').value = cfg.save_dir || '';
    hotkeyInput.value = cfg.hotkey || 'CommandOrControl+Shift+S';
    document.getElementById('sound-on').checked = cfg.sound_enabled !== false;
    document.getElementById('toast-dur').value = cfg.toast_duration || 2500;
    document.getElementById('toast-dur-label').textContent = ((cfg.toast_duration||2500)/1000).toFixed(1)+'s';
  } catch(e) { console.error('Load config failed:', e); }
}

async function init() {
  await loadConfig();
  setStatus('READY');
}
init().catch(console.error);
