// ============ PixelSnap App Logic ============
const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : async () => {};
const event = TAURI ? TAURI.event : null;
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

let activeMode = 'image';
let activeFmt = 'png';
let activeRecPos = 'top-left';
let cooldown = false;

// ---- Starfield (50 twinkling stars) ----
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

// ---- Meteor shower ----
(function spawnMeteors() {
  const sf = $('#starfield');
  if (!sf) return;
  function makeMeteor() {
    const m = document.createElement('div');
    m.className = 'meteor';
    m.style.top = Math.random() * -30 + '%';
    m.style.left = Math.random() * 120 + 20 + '%';
    m.style.animation = 'meteorFall ' + (Math.random() * 0.6 + 0.8) + 's linear forwards';
    if (Math.random() > 0.5) {
      m.style.background = 'linear-gradient(90deg, transparent, var(--accent-pink), #fff)';
      m.style.boxShadow = '0 0 6px var(--accent-pink), 0 0 12px rgba(255,110,180,0.4)';
    } else {
      m.style.background = 'linear-gradient(90deg, transparent, var(--accent-mint), #fff)';
      m.style.boxShadow = '0 0 6px var(--accent-mint), 0 0 12px rgba(127,255,212,0.4)';
    }
    sf.appendChild(m);
    setTimeout(() => m.remove(), 1600);
  }
  setInterval(() => {
    if (Math.random() > 0.35) makeMeteor();
  }, 2200);
})();

// ---- Mode block selection ----
const modeBlocks = $$('.mode-block');
modeBlocks.forEach(block => {
  const head = block.querySelector('.mode-head');
  head.addEventListener('click', () => {
    activeMode = block.dataset.mode;
    modeBlocks.forEach(b => b.classList.remove('active'));
    block.classList.add('active');
    scheduleSave();
  });
});

// ---- Format picker (PNG/JPG) ----
const fmtBtns = $$('#fmt-picker button');
const imgTag = $('#image-tag');
const jpgQualityRow = $('#jpg-quality-row');

function updateFmtUI() {
  fmtBtns.forEach(b => b.classList.remove('active'));
  const btn = document.querySelector(`#fmt-picker button[data-fmt="${activeFmt}"]`);
  if (btn) btn.classList.add('active');
  if (imgTag) imgTag.textContent = activeFmt.toUpperCase();
  if (jpgQualityRow) {
    jpgQualityRow.style.opacity = activeFmt === 'jpg' ? '1' : '0.45';
    jpgQualityRow.style.pointerEvents = activeFmt === 'jpg' ? 'auto' : 'none';
  }
}

fmtBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    fmtBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    activeFmt = btn.dataset.fmt;
    updateFmtUI();
    scheduleSave();
  });
});

// ---- JPG quality slider ----
$('#quality').addEventListener('input', (e) => {
  $('#qv').textContent = e.target.value;
  scheduleSave();
});

// ---- Rec position picker ----
const recPosBtns = $$('#rec-pos-picker button');
recPosBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    recPosBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    activeRecPos = btn.dataset.pos;
    scheduleSave();
  });
});

// ---- Sliders ----
$('#toast-dur').addEventListener('input', (e) => {
  $('#toast-dur-label').textContent = (e.target.value / 1000).toFixed(1) + 's';
});
$('#thumb-size').addEventListener('input', (e) => {
  $('#thumb-size-label').textContent = e.target.value + 'px';
});
$('#ui-opacity').addEventListener('input', (e) => {
  const val = e.target.value;
  $('#ui-opacity-label').textContent = val + '%';
  document.body.style.setProperty('--ui-opacity', (val / 100).toString());
});

// ---- Hotkey recording ----
let recordingHotkey = null;

function startRecording(wrap, input) {
  if (recordingHotkey) stopRecording(recordingHotkey, false);
  recordingHotkey = wrap;
  wrap.classList.add('recording');
  input.value = 'RECORDING...';
  input.style.color = '';
}
function stopRecording(wrap, success) {
  wrap.classList.remove('recording');
  if (!success && wrap.querySelector('input').dataset.pending) {
    wrap.querySelector('input').value = wrap.querySelector('input').dataset.pending;
  }
  wrap.querySelector('input').dataset.pending = '';
  if (recordingHotkey === wrap) recordingHotkey = null;
}

$('#hotkey-image').addEventListener('click', function() {
  startRecording($('#hk-image-wrap'), this);
});
$('#hotkey-video').addEventListener('click', function() {
  startRecording($('#hk-video-wrap'), this);
});
$('#hotkey-motion').addEventListener('click', function() {
  startRecording($('#hk-motion-wrap'), this);
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
  const modifiers = ['Control', 'Meta', 'Alt', 'Shift'];
  if (!modifiers.includes(key)) {
    let k = key;
    if (k === ' ') k = 'Space';
    if (k.length === 1) k = k.toUpperCase();
    parts.push(k);
    const input = recordingHotkey.querySelector('input');
    if (parts.length < 2) {
      input.value = parts.join('+') + ' (need modifier)';
      return;
    }
    let accelerator = parts.join('+').replace('Control', 'CommandOrControl');
    input.value = accelerator;
    input.dataset.pending = accelerator;
    stopRecording(recordingHotkey, true);
    scheduleSave();
  }
});

// ---- Title bar buttons ----
$('#btn-min').addEventListener('click', () => {
  invoke('minimize_window').catch(() => {});
});
$('#btn-close').addEventListener('click', () => {
  invoke('close_window').catch(() => {});
});

// ---- Open folder ----
$('#btn-open').addEventListener('click', () => {
  invoke('open_save_folder').catch(() => {});
});

// ---- Browse folder ----
$('#btn-browse').addEventListener('click', async () => {
  try {
    let selected = null;
    if (TAURI && TAURI.dialog) {
      selected = await TAURI.dialog.open({ directory: true, title: 'Select Save Folder' });
    }
    if (selected) {
      const path = typeof selected === 'string' ? selected : (Array.isArray(selected) ? selected[0] : null);
      if (path) {
        $('#save-dir').value = path;
        scheduleSave();
      }
    }
  } catch(e) {
    invoke('browse_folder').then(dir => {
      if (dir) {
        $('#save-dir').value = dir;
        scheduleSave();
      }
    }).catch(() => {});
  }
});

// ---- Reset button ----
$('#btn-reset').addEventListener('click', () => {
  if (!confirm('Reset all settings to defaults?')) return;
  invoke('reset_config').then(cfg => {
    applyConfig(cfg);
    scheduleSave();
  }).catch(() => {});
});

// ---- SNAP Button ----
const snapBtn = $('#btn-snap-test');
snapBtn.addEventListener('click', () => {
  if (cooldown) return;
  cooldown = true;
  setTimeout(() => cooldown = false, 500);
  const origText = snapBtn.textContent;
  const sbDot = $('#sb-dot');
  const sbText = $('#sb-text');
  snapBtn.textContent = 'SNAPPING...';
  snapBtn.classList.add('recording');
  if (sbDot) sbDot.classList.add('rec');
  if (sbText) sbText.textContent = 'CAPTURING...';
  invoke('take_screenshot', { mode: activeMode, hideWindow: true })
    .then(() => {
      snapBtn.textContent = 'COMPLETED';
      setTimeout(() => {
        snapBtn.textContent = origText;
        snapBtn.classList.remove('recording');
      }, 1000);
    })
    .catch((err) => {
      snapBtn.textContent = 'ERROR';
      snapBtn.classList.remove('recording');
      snapBtn.style.background = 'var(--accent-red)';
      if (sbDot) sbDot.classList.remove('rec');
      if (sbText) sbText.textContent = 'ERROR';
      setTimeout(() => {
        snapBtn.textContent = origText;
        snapBtn.style.background = '';
        if (sbText) sbText.textContent = 'SYSTEM ONLINE';
      }, 2000);
    });
});

// ---- Tauri events ----
let saveTimer;
function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(doSaveConfig, 400);
}
async function doSaveConfig() {
  try {
    await invoke('save_config', { config: collectConfig() });
  } catch(e) { console.error('Save config failed:', e); }
}

// Auto-save on changes
document.addEventListener('change', scheduleSave);
document.addEventListener('input', (e) => {
  if (e.target.type === 'range' || e.target.type === 'text' || e.target.type === 'number' || e.target.type === 'checkbox') scheduleSave();
});

function collectConfig() {
  const recPosBtn = $('#rec-pos-picker button.active');
  return {
    mode: activeMode,
    image_format: activeFmt,
    jpeg_quality: parseInt($('#quality').value) || 90,
    video_duration: parseInt($('#vid-dur').value) || 3,
    video_fps: parseInt($('#vid-fps').value) || 30,
    motion_duration: parseInt($('#mot-dur').value) || 3,
    motion_fps: parseInt($('#mot-fps').value) || 15,
    save_dir: $('#save-dir').value || '',
    filename_prefix: $('#filename-prefix').value.trim(),
    thumbnail_size: parseInt($('#thumb-size').value) || 128,
    show_toast: $('#show-toast').checked,
    toast_duration: parseInt($('#toast-dur').value) || 2500,
    auto_open_folder: $('#auto-open-folder').checked,
    sound_enabled: $('#sound-on').checked,
    start_minimized: $('#start-minimized').checked,
    hide_on_capture: $('#hide-on-capture').checked,
    save_thumbnail: $('#save-thumbnail').checked,
    record_system_audio: $('#record-audio').checked,
    rec_position: recPosBtn ? recPosBtn.dataset.pos : 'top-left',
    close_to_tray: $('#close-to-tray').checked,
    ui_opacity: parseInt($('#ui-opacity').value) || 100,
    hotkey_image: $('#hotkey-image').dataset.pending || $('#hotkey-image').value || 'CommandOrControl+Shift+S',
    hotkey_video: $('#hotkey-video').dataset.pending || $('#hotkey-video').value || 'CommandOrControl+Shift+V',
    hotkey_motion: $('#hotkey-motion').dataset.pending || $('#hotkey-motion').value || 'CommandOrControl+Shift+M',
  };
}

function applyConfig(cfg) {
  if (!cfg) return;
  activeMode = cfg.mode || 'image';
  activeFmt = (cfg.image_format && (cfg.image_format.toLowerCase() === 'jpg' || cfg.image_format.toLowerCase() === 'jpeg')) ? 'jpg' : 'png';
  activeRecPos = cfg.rec_position || cfg.rec_corner || 'top-left';

  modeBlocks.forEach(b => b.classList.remove('active'));
  const ab = document.querySelector(`.mode-block[data-mode="${activeMode}"]`);
  if (ab) ab.classList.add('active');

  updateFmtUI();

  recPosBtns.forEach(b => b.classList.remove('active'));
  const arb = document.querySelector(`#rec-pos-picker button[data-pos="${activeRecPos}"]`);
  if (arb) arb.classList.add('active');

  // JPG quality
  const jq = cfg.jpeg_quality || 90;
  $('#quality').value = jq;
  $('#qv').textContent = jq;

  $('#vid-dur').value = cfg.video_duration || 3;
  $('#vid-fps').value = cfg.video_fps || 30;
  $('#mot-dur').value = cfg.motion_duration || 3;
  $('#mot-fps').value = cfg.motion_fps || 15;
  $('#save-dir').value = cfg.save_dir || '';
  $('#filename-prefix').value = cfg.filename_prefix || '';
  $('#thumb-size').value = cfg.thumbnail_size || 128;
  $('#thumb-size-label').textContent = (cfg.thumbnail_size || 128) + 'px';
  $('#show-toast').checked = cfg.show_toast !== false;
  $('#toast-dur').value = cfg.toast_duration || 2500;
  $('#toast-dur-label').textContent = ((cfg.toast_duration || 2500) / 1000).toFixed(1) + 's';
  $('#auto-open-folder').checked = cfg.auto_open_folder !== false;
  $('#sound-on').checked = cfg.sound_enabled !== false;
  $('#start-minimized').checked = !!cfg.start_minimized;
  $('#hide-on-capture').checked = cfg.hide_on_capture !== false;
  $('#save-thumbnail').checked = !!cfg.save_thumbnail;
  $('#record-audio').checked = cfg.record_system_audio !== false;
  $('#close-to-tray').checked = cfg.close_to_tray !== false;
  $('#ui-opacity').value = cfg.ui_opacity || 100;
  $('#ui-opacity-label').textContent = (cfg.ui_opacity || 100) + '%';
  document.body.style.setProperty('--ui-opacity', ((cfg.ui_opacity || 100) / 100).toString());

  if (cfg.hotkey_image) { $('#hotkey-image').value = cfg.hotkey_image; }
  if (cfg.hotkey_video) { $('#hotkey-video').value = cfg.hotkey_video; }
  if (cfg.hotkey_motion) { $('#hotkey-motion').value = cfg.hotkey_motion; }
}

// ---- Listen for events from Rust ----
if (event) {
  event.listen('capture-started', () => {
    const sbDot = $('#sb-dot');
    if (sbDot) sbDot.classList.add('rec');
  });
  event.listen('capture-completed', () => {
    const sbDot = $('#sb-dot');
    const sbText = $('#sb-text');
    if (sbDot) sbDot.classList.remove('rec');
    if (sbText) sbText.textContent = 'COMPLETED';
    setTimeout(() => { if (sbText) sbText.textContent = 'SYSTEM ONLINE'; }, 2000);
  });
  event.listen('capture-error', (e) => {
    const sbDot = $('#sb-dot');
    const sbText = $('#sb-text');
    if (sbDot) sbDot.classList.remove('rec');
    if (sbText) sbText.textContent = 'ERROR';
    setTimeout(() => { if (sbText) sbText.textContent = 'SYSTEM ONLINE'; }, 3000);
  });
  event.listen('save-completed', () => {
    const sbText = $('#sb-text');
    if (sbText) sbText.textContent = 'SAVED';
  });
}

// ---- Number inputs: digits only ----
$$('input[type=number]').forEach(inp => {
  inp.addEventListener('keydown', e => {
    const allowed = [8,9,27,13,37,39,36,35,46];
    if (allowed.indexOf(e.keyCode) !== -1) return;
    if ((e.keyCode < 48 || e.keyCode > 57) && (e.keyCode < 96 || e.keyCode > 105)) e.preventDefault();
  });
});

// ---- Load config ----
invoke('load_config').then(cfg => {
  applyConfig(cfg);
}).catch(() => {
  updateFmtUI();
});
