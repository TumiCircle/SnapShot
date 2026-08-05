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
let currentCfg = null;

function clampInt(v, lo, hi) {
  const n = parseInt(v, 10);
  if (isNaN(n)) return lo;
  return Math.min(hi, Math.max(lo, n));
}

function segVal(el) {
  if (!el) return 0;
  const b = el.querySelector('button.active');
  return b ? parseInt(b.dataset.val, 10) : 0;
}

function setSeg(el, val) {
  if (!el) return;
  let best = null;
  let bestDiff = Infinity;
  el.querySelectorAll('button').forEach(b => {
    const v = parseInt(b.dataset.val, 10);
    const diff = Math.abs(v - val);
    if (diff < bestDiff) {
      bestDiff = diff;
      best = b;
    }
  });
  if (best) {
    el.querySelectorAll('button').forEach(b => b.classList.remove('active'));
    best.classList.add('active');
  }
}

// ---- i18n ----
const I18N = {
  en: {
    appName: 'PIXELSNAP',
    controlPanel: '// CONTROL PANEL',
    heroTitle: 'PIXEL SNAP',
    heroSub1: '> CATCH THE EPIC MOMENT',
    heroSub2: 'SAVE IT FOREVER',
    snap: 'SNAP!',
    snapHint: 'PRESS HOTKEY ANYWHERE TO CAPTURE',
    captureMode: '▸ CAPTURE MODE',
    imageMode: 'IMAGE SNAPSHOT',
    videoMode: 'VIDEO SNAPSHOT',
    motionMode: 'MOTION PHOTO',
    jpgQuality: 'JPG QUALITY:',
    duration: 'DURATION:',
    sec: 'SEC',
    framerate: 'FRAMERATE:',
    fps: 'FPS',
    output: '▸ OUTPUT',
    saveDirectory: 'SAVE DIRECTORY',
    browse: 'BROWSE',
    open: 'OPEN',
    filenamePrefix: 'FILENAME PREFIX',
    thumbnailSize: 'THUMBNAIL SIZE',
    hotkeys: '▸ HOTKEYS',
    image: 'IMAGE',
    video: 'VIDEO',
    motion: 'MOTION',
    hotkeyPlaceholder: 'Click to record...',
    recording: '▸ RECORDING',
    recordSystemAudio: 'RECORD SYSTEM AUDIO',
    recPosition: 'REC POSITION',
    videoBitrate: 'VIDEO BITRATE',
    toastDuration: 'TOAST DURATION',
    appearance: '▸ APPEARANCE',
    windowTransparency: 'WINDOW TRANSPARENCY',
    uiTransparency: 'UI TRANSPARENCY',
    starfieldDensity: 'STARFIELD DENSITY',
    starTwinkleSpeed: 'STAR TWINKLE SPEED',
    meteorRate: 'METEOR RATE',
    language: 'LANGUAGE',
    behavior: '▸ BEHAVIOR',
    hideOnCapture: 'HIDE ON CAPTURE',
    saveThumbnails: 'SAVE THUMBNAILS LOCALLY',
    autoOpenFolder: 'AUTO-OPEN FOLDER AFTER CAPTURE',
    enableCaptureSound: 'ENABLE CAPTURE SOUND',
    showToast: 'SHOW TOAST NOTIFICATION',
    startMinimized: 'START MINIMIZED TO TRAY',
    closeToTray: "CLOSE TO TRAY (DON'T EXIT)",
    reset: 'RESET ALL SETTINGS',
    systemOnline: 'SYSTEM ONLINE',
    capturing: 'CAPTURING...',
    snapping: 'SNAPPING...',
    completed: 'COMPLETED',
    saved: 'SAVED',
    error: 'ERROR',
    off: 'OFF',
    low: 'LOW',
    med: 'MED',
    high: 'HIGH',
  },
  zh: {
    appName: '像素快拍',
    controlPanel: '// 控制面板',
    heroTitle: '像素快拍',
    heroSub1: '> 捕捉精彩瞬间',
    heroSub2: '永久保存',
    snap: '快拍!',
    snapHint: '任意位置按快捷键即可捕获',
    captureMode: '▸ 捕获模式',
    imageMode: '图片快照',
    videoMode: '视频快照',
    motionMode: '动图照片',
    jpgQuality: 'JPG 质量:',
    duration: '时长:',
    sec: '秒',
    framerate: '帧率:',
    fps: '帧/秒',
    output: '▸ 输出设置',
    saveDirectory: '保存目录',
    browse: '浏览',
    open: '打开',
    filenamePrefix: '文件名前缀',
    thumbnailSize: '缩略图大小',
    hotkeys: '▸ 快捷键',
    image: '图片',
    video: '视频',
    motion: '动图',
    hotkeyPlaceholder: '点击录制...',
    recording: '▸ 录制设置',
    recordSystemAudio: '录制系统声音',
    recPosition: '录制位置',
    videoBitrate: '视频码率',
    toastDuration: '提示时长',
    appearance: '▸ 外观',
    windowTransparency: '窗口透明度',
    uiTransparency: '界面透明度',
    starfieldDensity: '星空密度',
    starTwinkleSpeed: '星星闪烁速度',
    meteorRate: '流星频率',
    language: '语言',
    behavior: '▸ 行为',
    hideOnCapture: '捕获时隐藏窗口',
    saveThumbnails: '本地保存缩略图',
    autoOpenFolder: '捕获后自动打开文件夹',
    enableCaptureSound: '启用捕获提示音',
    showToast: '显示完成通知',
    startMinimized: '启动时最小化到托盘',
    closeToTray: '关闭到托盘（不退出）',
    reset: '重置所有设置',
    systemOnline: '系统就绪',
    capturing: '正在捕获...',
    snapping: '快拍中...',
    completed: '完成',
    saved: '已保存',
    error: '错误',
    off: '关',
    low: '低',
    med: '中',
    high: '高',
  },
};

function currentLang() {
  return currentCfg && currentCfg.language === 'zh' ? 'zh' : 'en';
}

function t(key) {
  const dict = I18N[currentLang()] || I18N.en;
  return dict[key] != null ? dict[key] : key;
}

function applyLanguage() {
  const dict = I18N[currentLang()] || I18N.en;
  $$('[data-i18n]').forEach(el => {
    const key = el.dataset.i18n;
    if (dict[key] != null) el.textContent = dict[key];
  });
  $$('[data-i18n-placeholder]').forEach(el => {
    const key = el.dataset.i18nPlaceholder;
    if (dict[key] != null) el.placeholder = dict[key];
  });
  const sbText = $('#sb-text');
  if (sbText && !sbText.dataset.busy) sbText.textContent = t('systemOnline');
}

function translateError(msg) {
  if (currentLang() !== 'zh' || !msg) return msg;
  const map = [
    ['No valid foreground window', '未找到有效的前台窗口，请先点击目标窗口'],
    ['Timed out waiting for first GIF frame', '等待 GIF 首帧超时'],
    ['Not enough frames captured', '采集到的帧数不足'],
    ['Video encoding timed out', '视频编码超时'],
    ['Capture timed out', '捕获超时'],
    ['Window closed during capture', '捕获期间窗口被关闭'],
    ['Save failed', '保存失败'],
    ['Failed to create file', '创建文件失败'],
    ['Failed to start capture', '启动捕获失败'],
  ];
  for (const [en, zh] of map) {
    if (msg.includes(en)) return zh;
  }
  return msg;
}

// ---- Starfield (density + twinkle speed from settings) ----
function buildStarfield() {
  const sf = $('#starfield');
  if (!sf) return;
  const density = segVal($('#star-density'));
  const twinkle = segVal($('#star-twinkle'));
  const count = Math.round(20 + density * 0.8);
  sf.innerHTML = '';
  const palette = [null, null, null, 'mint', 'pink', 'yellow'];
  for (let i = 0; i < count; i++) {
    const s = document.createElement('div');
    const size = Math.random() < 0.7 ? 1 : (Math.random() < 0.6 ? 2 : 3);
    s.className = 'star s' + size;
    const tint = palette[Math.floor(Math.random() * palette.length)];
    if (tint) s.classList.add(tint);
    s.style.left = Math.random() * 100 + '%';
    s.style.top = Math.random() * 100 + '%';
    if (twinkle > 0) {
      const dur = Math.max(0.4, 1.8 - twinkle / 100).toFixed(2);
      s.style.setProperty('--twinkle-dur', dur + 's');
      // Negative delays start each star at a random phase so they never blink
      // in sync.
      s.style.animationDelay = (-Math.random() * 2).toFixed(2) + 's';
    } else {
      s.style.animation = 'none';
    }
    sf.appendChild(s);
  }
}

// ---- Meteor shower (rate from settings) ----
let meteorTimer = null;
function startMeteors() {
  const sf = $('#starfield');
  if (!sf) return;
  if (meteorTimer) {
    clearInterval(meteorTimer);
    meteorTimer = null;
  }
  const rate = segVal($('#meteor-rate'));
  if (rate <= 0) return;
  const interval = Math.round(5200 - rate * 42);
  function makeMeteor() {
    const m = document.createElement('div');
    m.className = 'meteor';
    m.style.top = Math.random() * -30 + '%';
    m.style.left = Math.random() * 100 + '%';
    m.style.animation = 'meteorFall ' + (Math.random() * 0.5 + 0.9).toFixed(2) + 's linear forwards';
    m.style.animationDelay = (Math.random() * 0.5).toFixed(2) + 's';
    if (Math.random() > 0.5) {
      m.style.background = 'linear-gradient(90deg, transparent, rgba(255,110,180,0.7) 72%, #fff)';
      m.style.boxShadow = '0 0 8px 1px rgba(255,110,180,0.85), 0 0 22px 2px rgba(255,110,180,0.35)';
    } else {
      m.style.background = 'linear-gradient(90deg, transparent, rgba(127,255,212,0.7) 72%, #fff)';
      m.style.boxShadow = '0 0 8px 1px rgba(127,255,212,0.85), 0 0 22px 2px rgba(127,255,212,0.35)';
    }
    sf.appendChild(m);
    setTimeout(() => m.remove(), 2000);
  }
  meteorTimer = setInterval(() => {
    if (Math.random() < 0.75) makeMeteor();
  }, Math.max(700, interval));
}

// ---- Appearance (window/UI transparency + starfield) ----
function applyAppearance() {
  const winT = segVal($('#win-transparency'));
  const uiT = segVal($('#ui-transparency'));
  document.documentElement.style.setProperty('--win-alpha', (1 - winT / 100).toFixed(3));
  document.documentElement.style.setProperty('--ui-opacity', (1 - uiT / 100).toFixed(3));
  buildStarfield();
  startMeteors();
}

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
$('#video-bitrate').addEventListener('input', (e) => {
  $('#video-bitrate-label').textContent = e.target.value + ' Mbps';
});

// ---- Appearance segmented controls ----
['#win-transparency', '#ui-transparency', '#star-density', '#star-twinkle', '#meteor-rate'].forEach(id => {
  const el = $(id);
  if (!el) return;
  el.querySelectorAll('button').forEach(btn => {
    btn.addEventListener('click', () => {
      el.querySelectorAll('button').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      applyAppearance();
      scheduleSave();
    });
  });
});

// ---- Language picker ----
const langBtns = $$('#lang-picker button');
langBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    langBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    if (!currentCfg) currentCfg = {};
    currentCfg.language = btn.dataset.lang;
    applyLanguage();
    scheduleSave();
  });
});

// Initial starfield before config loads (defaults)
buildStarfield();
startMeteors();
applyLanguage();

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
  snapBtn.textContent = t('snapping');
  snapBtn.classList.add('recording');
  if (sbDot) sbDot.classList.add('rec');
  if (sbText) sbText.textContent = t('capturing');
  invoke('take_screenshot', { mode: activeMode, hideWindow: true })
    .then(() => {
      snapBtn.textContent = t('completed');
      setTimeout(() => {
        snapBtn.textContent = origText;
        snapBtn.classList.remove('recording');
      }, 1000);
    })
    .catch((err) => {
      snapBtn.textContent = t('error');
      snapBtn.classList.remove('recording');
      snapBtn.style.background = 'var(--accent-red)';
      if (sbDot) sbDot.classList.remove('rec');
      if (sbText) sbText.textContent = t('error');
      setTimeout(() => {
        snapBtn.textContent = origText;
        snapBtn.style.background = '';
        if (sbText) sbText.textContent = t('systemOnline');
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
    video_bitrate: (parseInt($('#video-bitrate').value) || 8) * 1000000,
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
    window_transparency: segVal($('#win-transparency')),
    ui_transparency: segVal($('#ui-transparency')),
    starfield_density: segVal($('#star-density')),
    star_twinkle_speed: segVal($('#star-twinkle')),
    meteor_rate: segVal($('#meteor-rate')),
    language: ($('#lang-picker button.active') || { dataset: {} }).dataset.lang || 'en',
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
  $('#video-bitrate').value = Math.round((cfg.video_bitrate || 8000000) / 1000000);
  $('#video-bitrate-label').textContent = $('#video-bitrate').value + ' Mbps';
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
  setSeg($('#win-transparency'), cfg.window_transparency ?? 0);
  setSeg($('#ui-transparency'), cfg.ui_transparency ?? 0);
  setSeg($('#star-density'), cfg.starfield_density ?? 50);
  setSeg($('#star-twinkle'), cfg.star_twinkle_speed ?? 50);
  setSeg($('#meteor-rate'), cfg.meteor_rate ?? 50);
  const lang = cfg.language === 'zh' ? 'zh' : 'en';
  langBtns.forEach(b => b.classList.toggle('active', b.dataset.lang === lang));
  currentCfg = cfg;
  applyAppearance();
  applyLanguage();

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
    if (sbText) sbText.textContent = t('completed');
    setTimeout(() => { if (sbText) sbText.textContent = t('systemOnline'); }, 2000);
  });
  event.listen('capture-error', (e) => {
    const sbDot = $('#sb-dot');
    const sbText = $('#sb-text');
    if (sbDot) sbDot.classList.remove('rec');
    if (sbText) sbText.textContent = translateError(String(e.payload || t('error'))).slice(0, 42);
    setTimeout(() => { if (sbText) sbText.textContent = t('systemOnline'); }, 5000);
  });
  event.listen('save-completed', () => {
    const sbText = $('#sb-text');
    if (sbText) sbText.textContent = t('saved');
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
  currentCfg = cfg;
  applyConfig(cfg);
}).catch(() => {
  updateFmtUI();
});
