const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;
const { LogicalSize, LogicalPosition } = window.__TAURI__.dpi;

const BAR_WIDTH = 10;
const FILLED = '\u2588';
const EMPTY = '\u2591';

function buildBar(utilization) {
  const pct = Math.max(0, Math.min(100, utilization));
  const filled = Math.round((pct / 100) * BAR_WIDTH);
  const empty = BAR_WIDTH - filled;
  return FILLED.repeat(filled) + EMPTY.repeat(empty);
}

function colorClass(utilization) {
  if (utilization > 90) return 'critical';
  if (utilization > 70) return 'warning';
  return '';
}

function setStarFresh() {
  const star = document.getElementById('star');
  if (!star) return;
  star.className = 'star fresh';
  setTimeout(function() {
    star.className = 'star';
    void star.offsetWidth;
    star.className = 'star stale';
  }, 50);
}

function setStarError() {
  const star = document.getElementById('star');
  if (!star) return;
  star.className = 'star error';
}

function formatTimeLeft(isoString) {
  if (!isoString) return '';
  const now = Date.now();
  const reset = new Date(isoString).getTime();
  const diffMs = reset - now;
  if (diffMs <= 0) return '';
  const hours = Math.floor(diffMs / (1000 * 60 * 60));
  if (hours >= 24) {
    return Math.floor(hours / 24) + 'd';
  }
  return hours + 'h';
}

function renderBucket(prefix, bucket) {
  const barEl = document.getElementById('bar-' + prefix);
  const pctEl = document.getElementById('pct-' + prefix);
  const bucketEl = document.getElementById('bucket-' + prefix);

  if (!bucket) {
    bucketEl.style.display = 'none';
    return;
  }

  bucketEl.style.display = 'inline-flex';
  const cls = colorClass(bucket.utilization);

  barEl.textContent = buildBar(bucket.utilization);
  barEl.className = 'bar ' + cls;

  pctEl.textContent = Math.floor(bucket.utilization) + '%';
  pctEl.className = 'pct ' + cls;
}

function renderUsage(data) {
  document.getElementById('no-key-msg').style.display = 'none';
  document.getElementById('loading-msg').style.display = 'none';
  document.getElementById('usage-display').style.display = 'flex';

  renderBucket('session', data.session);
  renderBucket('weekly-all', data.weekly_all);
  renderBucket('weekly-sonnet', data.weekly_sonnet);

  var timeLeftEl = document.getElementById('time-left');
  if (timeLeftEl && data.weekly_all) {
    if (data.weekly_all.utilization >= 30) {
      timeLeftEl.textContent = formatTimeLeft(data.weekly_all.resets_at);
    } else {
      timeLeftEl.textContent = '';
    }
  }

  setStarFresh();
  resizeToContent();
}

function showNoKeyUI() {
  document.getElementById('no-key-msg').style.display = 'flex';
  document.getElementById('usage-display').style.display = 'none';
  resizeToContent();
}

async function resizeToContent() {
  await new Promise(function(r) { requestAnimationFrame(r); });
  const wrapper = document.getElementById('wrapper');
  const w = wrapper.scrollWidth + 4;
  const h = wrapper.scrollHeight + 4;
  const win = getCurrentWindow();
  await win.setSize(new LogicalSize(w, h));
}

// Custom drag implementation — bypasses broken native drag on transparent macOS windows
function setupDrag(win) {
  var dragging = false;
  var startScreenX, startScreenY;
  var startWinX, startWinY;

  document.addEventListener('mousedown', function(e) {
    if (e.button !== 0) return;
    // Don't drag from buttons
    if (e.target.closest('button')) return;

    dragging = true;
    startScreenX = e.screenX;
    startScreenY = e.screenY;

    win.outerPosition().then(function(pos) {
      startWinX = pos.x;
      startWinY = pos.y;
    });

    document.body.style.cursor = 'grabbing';
    e.preventDefault();
  });

  document.addEventListener('mousemove', function(e) {
    if (!dragging) return;
    var dx = e.screenX - startScreenX;
    var dy = e.screenY - startScreenY;
    win.setPosition(new LogicalPosition(startWinX + dx, startWinY + dy));
    e.preventDefault();
  });

  document.addEventListener('mouseup', function(e) {
    if (!dragging) return;
    dragging = false;
    document.body.style.cursor = '';

    // Save final position
    win.outerPosition().then(function(pos) {
      invoke('save_widget_position', { x: pos.x, y: pos.y });
    });
  });
}

async function init() {
  var win = getCurrentWindow();

  // Custom drag
  setupDrag(win);

  // Close button — kill process
  document.getElementById('close-btn').addEventListener('click', function(e) {
    e.preventDefault();
    e.stopPropagation();
    invoke('exit_app');
  });

  // Usage updates from polling
  await listen('usage-updated', function(event) {
    renderUsage(event.payload);
  });

  await listen('usage-error', function() {
    setStarError();
  });

  // Initial fetch
  document.getElementById('loading-msg').style.display = 'flex';
  try {
    const data = await invoke('refresh_usage');
    if (data.session || data.weekly_all || data.weekly_sonnet) {
      renderUsage(data);
    } else {
      document.getElementById('loading-msg').style.display = 'none';
      showNoKeyUI();
    }
  } catch (e) {
    document.getElementById('loading-msg').style.display = 'none';
    showNoKeyUI();
    setStarError();
  }
}

document.addEventListener('DOMContentLoaded', init);
