const byId = (id) => document.getElementById(id);
const translations = {
  en: {
    title: 'Laya · Operations console', skip: 'Skip to controls', operations: '/ Operations',
    language: 'Language', localAdmin: 'Local admin', serviceStatus: 'Service status',
    model: 'Model', device: 'Device', controls: 'Service controls', nextStart: 'Changes apply on next start',
    multilingual: 'Multilingual', englishModel: 'English', typedDecisions: 'Typed decisions',
    autoSelect: 'Auto select', start: 'Start service', stop: 'Stop service',
    liveMetrics: 'Live metrics', totalRequests: 'Total requests', succeeded: 'Succeeded',
    failedMetric: 'Failed', avgLatency: 'Avg. latency', provider: 'Provider', uptime: 'Uptime',
    recentRequests: 'Recent requests', historyNote: 'Last 50 · metadata only',
    time: 'Time', result: 'Result', duration: 'Duration', questions: 'Questions',
    historyLabel: 'Recent request history', checking: 'Checking…', connecting: 'Connecting to the local administrator.',
    disconnected: 'Disconnected', running: 'Online', starting: 'Starting', failed: 'Failed',
    stopped: 'Offline', unknown: 'Unknown', ready: 'Inference API ready for requests.',
    startingMessage: 'Starting the local inference process.', stoppedMessage: 'Start a local process to begin inference.',
    preparing: 'Preparing model and initializing inference (the CLI downloads missing models automatically)…',
    awaiting: 'Awaiting service', live: 'Live · refreshes every 3s',
    noDisplay: 'No requests to display.', noRecorded: 'No requests recorded yet.',
    success: 'Success', error: 'Error', requestFailed: (code) => `Request failed (${code})`,
    invalidRequest: 'Expected JSON with a valid model and device.',
    alreadyRunning: 'Stop the owned inference service before starting another.',
    portInUse: 'The inference API address is already in use; no process was started.',
    missingBinary: 'The laya executable was not found at the configured path.',
    spawnFailed: 'Could not launch the configured laya executable.',
    notRunningStop: 'There is no owned inference service to stop.',
    stopFailed: 'Could not stop the owned inference service.',
    notRunningTelemetry: 'No owned inference service is running.',
    apiNotReadyOwned: 'The owned inference API has not become available yet.',
    apiNotReady: 'The inference API has not become available yet.',
    apiTelemetryUnavailable: 'The inference API telemetry is unavailable.',
    apiTelemetryInvalid: 'The inference API returned invalid telemetry.'
  },
  zh: {
    title: 'Laya · 运行控制台', skip: '跳转到服务控制', operations: '/ 运行管理',
    language: '语言', localAdmin: '本地管理', serviceStatus: '服务状态',
    model: '模型', device: '设备', controls: '服务控制', nextStart: '更改将在下次启动时生效',
    multilingual: '多语言', englishModel: '英语', typedDecisions: '类型化决策',
    autoSelect: '自动选择', start: '启动服务', stop: '停止服务',
    liveMetrics: '实时指标', totalRequests: '请求总数', succeeded: '成功', failedMetric: '失败',
    avgLatency: '平均延迟', provider: '提供方', uptime: '运行时间',
    recentRequests: '最近请求', historyNote: '最近 50 条 · 仅元数据',
    time: '时间', result: '结果', duration: '耗时', questions: '问题数',
    historyLabel: '最近请求记录', checking: '正在检查…', connecting: '正在连接本地管理服务。',
    disconnected: '连接已断开', running: '运行中', starting: '启动中', failed: '启动失败',
    stopped: '已停止', unknown: '未知', ready: '推理 API 已就绪，可接收请求。',
    startingMessage: '正在启动本地推理进程。', stoppedMessage: '启动本地进程以开始推理。',
    preparing: '正在准备模型并初始化推理（CLI 会自动下载缺失的模型）…',
    awaiting: '等待服务启动', live: '实时 · 每 3 秒刷新',
    noDisplay: '暂无请求可显示。', noRecorded: '尚无请求记录。',
    success: '成功', error: '错误', requestFailed: (code) => `请求失败（${code}）`,
    invalidRequest: '需要包含有效模型和设备的 JSON。',
    alreadyRunning: '请先停止当前推理服务，再启动另一个。',
    portInUse: '推理 API 地址已被占用；未启动进程。',
    missingBinary: '在配置的路径下找不到 laya 可执行文件。',
    spawnFailed: '无法启动配置的 laya 可执行文件。',
    notRunningStop: '没有可停止的本地推理服务。',
    stopFailed: '无法停止本地推理服务。',
    notRunningTelemetry: '本地推理服务尚未运行。',
    apiNotReadyOwned: '本地推理 API 尚未就绪。',
    apiNotReady: '推理 API 尚未就绪。',
    apiTelemetryUnavailable: '推理 API 遥测数据不可用。',
    apiTelemetryInvalid: '推理 API 返回了无效的遥测数据。'
  }
};
const serverMessages = {
  'Preparing model and initializing inference (the CLI downloads missing models automatically)…': 'preparing',
  'Expected JSON with a valid model and device.': 'invalidRequest',
  'Stop the owned inference service before starting another.': 'alreadyRunning',
  'The inference API address is already in use; no process was started.': 'portInUse',
  'The laya executable was not found at the configured path.': 'missingBinary',
  'Could not launch the configured laya executable.': 'spawnFailed',
  'There is no owned inference service to stop.': 'notRunningStop',
  'Could not stop the owned inference service.': 'stopFailed',
  'No owned inference service is running.': 'notRunningTelemetry',
  'The owned inference API has not become available yet.': 'apiNotReadyOwned',
  'The inference API has not become available yet.': 'apiNotReady',
  'The inference API telemetry is unavailable.': 'apiTelemetryUnavailable',
  'The inference API returned invalid telemetry.': 'apiTelemetryInvalid'
};
function initialLanguage() {
  try {
    const stored = localStorage.getItem('laya-language');
    if (stored === 'en' || stored === 'zh') return stored;
  } catch (_) { /* Storage can be disabled. */ }
  const preferred = navigator.languages?.[0] || navigator.language || '';
  return /^zh(?:-|$)/i.test(preferred) ? 'zh' : 'en';
}
let language = initialLanguage();
let pending = false;
let lastStatus = null;
let lastTelemetry = null;
let statusError = null;
let telemetryError = null;
let actionError = null;
const t = (key) => translations[language][key];
const locale = () => language === 'zh' ? 'zh-CN' : 'en-US';
const number = (value) => typeof value === 'number' && Number.isFinite(value) ? value.toLocaleString(locale()) : '—';
const localizedMessage = (message) => serverMessages[message] ? t(serverMessages[message]) : message;
const errorMessage = (error) => error.fallbackStatus == null ? localizedMessage(error.message) : t('requestFailed')(error.fallbackStatus);

function emptyRow(message) {
  const row = document.createElement('tr');
  const cell = document.createElement('td');
  cell.colSpan = 5;
  cell.className = 'empty';
  cell.textContent = message;
  row.append(cell);
  return row;
}

function clearTelemetry() {
  for (const id of ['total', 'success', 'failed', 'latency', 'provider']) byId(id).textContent = '—';
  byId('api-state').textContent = telemetryError ? errorMessage(telemetryError) : t('awaiting');
  byId('request-rows').replaceChildren(emptyRow(t('noDisplay')));
}

async function request(path, options) {
  const response = await fetch(path, { cache: 'no-store', ...options });
  const data = await response.json();
  if (!response.ok) {
    if (data.error?.message) throw new Error(data.error.message);
    const error = new Error('Request failed');
    error.fallbackStatus = response.status;
    throw error;
  }
  return data;
}

function renderStatus(status) {
  const active = status.state === 'starting' || status.state === 'running';
  byId('state-panel').dataset.state = status.state;
  byId('state-name').textContent = t(['running', 'starting', 'failed', 'stopped'].includes(status.state) ? status.state : 'unknown');
  byId('status-symbol').textContent = status.state === 'running' ? '●' : status.state === 'starting' ? '◐' : '○';
  byId('state-message').textContent = status.message ? localizedMessage(status.message) :
    t(status.state === 'running' ? 'ready' : status.state === 'starting' ? 'startingMessage' : 'stoppedMessage');
  byId('active-model').textContent = status.model || '—';
  byId('active-device').textContent = status.device || '—';
  byId('active-pid').textContent = status.pid ?? '—';
  byId('provider').textContent = status.health?.provider || '—';
  byId('uptime').textContent = status.uptime_seconds == null ? '—' : `${number(status.uptime_seconds)}s`;
  byId('start').disabled = active;
  byId('stop').disabled = !active;
  byId('model').disabled = active;
  byId('device').disabled = active;
}

function renderTelemetry(telemetry) {
  const stats = telemetry.stats;
  byId('total').textContent = number(stats.total_requests);
  byId('success').textContent = number(stats.succeeded);
  byId('failed').textContent = number(stats.failed);
  byId('latency').textContent = typeof stats.average_latency_ms === 'number'
    ? `${stats.average_latency_ms.toLocaleString(locale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 })} ms` : '—';
  byId('api-state').textContent = t('live');
  const rows = (telemetry.history.items || []).map((item) => {
    const row = document.createElement('tr');
    const time = new Date(item.at_unix_ms);
    const values = [item.id, Number.isFinite(time.getTime()) ? time.toLocaleString(locale()) : '—',
      item.status === 'ok' ? t('success') : t('error'), `${number(item.duration_ms)} ms`, number(item.question_count)];
    for (let i = 0; i < values.length; i++) {
      const cell = document.createElement('td');
      cell.textContent = values[i];
      if (i === 2) cell.className = item.status === 'ok' ? 'result-ok' : 'result-error';
      row.append(cell);
    }
    return row;
  });
  byId('request-rows').replaceChildren(...(rows.length ? rows : [emptyRow(t('noRecorded'))]));
}

function renderCurrent() {
  document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
  document.title = t('title');
  byId('language').value = language;
  for (const element of document.querySelectorAll('[data-i18n]')) element.textContent = t(element.dataset.i18n);
  byId('language').setAttribute('aria-label', t('language'));
  byId('state-panel').setAttribute('aria-label', t('serviceStatus'));
  document.querySelector('.table-wrap').setAttribute('aria-label', t('historyLabel'));
  if (lastStatus) renderStatus(lastStatus);
  else {
    byId('state-panel').dataset.state = statusError ? 'disconnected' : 'checking';
    byId('state-name').textContent = t(statusError ? 'disconnected' : 'checking');
    byId('state-message').textContent = statusError ? errorMessage(statusError) : t('connecting');
  }
  if (lastTelemetry) renderTelemetry(lastTelemetry);
  else clearTelemetry();
  byId('action-message').textContent = actionError ? errorMessage(actionError) : '';
}

async function refresh() {
  if (pending || document.hidden) return;
  pending = true;
  try {
    const status = await request('/admin/status');
    lastStatus = status;
    statusError = null;
    renderStatus(status);
    if (status.state === 'running') {
      try {
        lastTelemetry = await request('/admin/telemetry');
        telemetryError = null;
        renderTelemetry(lastTelemetry);
      } catch (error) {
        lastTelemetry = null;
        telemetryError = error;
        clearTelemetry();
      }
    } else {
      lastTelemetry = null;
      telemetryError = null;
      clearTelemetry();
    }
  } catch (error) {
    lastStatus = null;
    lastTelemetry = null;
    statusError = error;
    telemetryError = null;
    clearTelemetry();
    byId('state-panel').dataset.state = 'disconnected';
    byId('state-name').textContent = t('disconnected');
    byId('state-message').textContent = errorMessage(error);
    for (const id of ['active-model', 'active-device', 'active-pid', 'uptime']) byId(id).textContent = '—';
    byId('start').disabled = true;
    byId('stop').disabled = true;
  } finally { pending = false; }
}

byId('language').addEventListener('change', (event) => {
  language = event.target.value === 'zh' ? 'zh' : 'en';
  try { localStorage.setItem('laya-language', language); } catch (_) { /* Storage can be disabled. */ }
  renderCurrent();
});
byId('launch-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  actionError = null;
  byId('action-message').textContent = '';
  byId('start').disabled = true;
  try {
    await request('/admin/start', { method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ model: byId('model').value, device: byId('device').value }) });
    await refresh();
  } catch (error) {
    actionError = error;
    byId('action-message').textContent = errorMessage(error);
    await refresh();
  }
});
byId('stop').addEventListener('click', async () => {
  actionError = null;
  byId('action-message').textContent = '';
  byId('stop').disabled = true;
  try { await request('/admin/stop', { method: 'POST' }); }
  catch (error) { actionError = error; byId('action-message').textContent = errorMessage(error); }
  await refresh();
});
document.addEventListener('visibilitychange', () => { if (!document.hidden) refresh(); });
renderCurrent();
refresh();
setInterval(refresh, 3000);
