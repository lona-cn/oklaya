(() => {
  'use strict';

  function mount(t) {
    const bridge = window.__TAURI__?.core;
    if (!bridge?.invoke) return null;
    const invoke = bridge.invoke.bind(bridge);
    const panel = document.querySelector('#mobile-controls');
    const status = document.querySelector('#device-status');
    const error = document.querySelector('#device-error');
    const portInput = document.querySelector('#lan-port');
    const download = document.querySelector('#download-model');
    const load = document.querySelector('#load-model');
    const start = document.querySelector('#start-lan');
    const stop = document.querySelector('#stop-lan');
    const details = document.querySelector('#lan-details');
    const urlInput = document.querySelector('#lan-url');
    const tokenInput = document.querySelector('#lan-token');
    const feedback = document.querySelector('#copy-feedback');
    let lastStatus = null;
    let busy = false;
    let statusSequence = 0;

    function render(info = lastStatus) {
      if (!info) return;
      lastStatus = info;
      status.textContent = busy ? t('deviceBusy') : [
        t(info.model_ready ? 'modelReady' : 'modelNotReady'),
        ...(info.provider ? [t('provider', info.provider)] : []),
        info.lan_port == null ? t('lanStopped') : t('lanRunning', info.lan_port),
      ].join(' · ');
      const running = info.lan_port != null;
      start.hidden = running;
      stop.hidden = !running;
      details.hidden = !running;
      portInput.disabled = running || busy;
      download.disabled = busy;
      load.disabled = busy || !!info.model_ready;
      start.disabled = busy || !info.model_ready;
      stop.disabled = busy;
      urlInput.value = running ? info.lan_url || '' : '';
      tokenInput.value = running ? info.lan_token || '' : '';
      if (!running) feedback.textContent = '';
    }

    async function refresh() {
      if (busy || document.visibilityState === 'hidden') return;
      const sequence = ++statusSequence;
      try {
        const info = await invoke('mobile_status');
        if (sequence === statusSequence && !busy) render(info);
      } catch (cause) {
        if (sequence !== statusSequence) return;
        error.textContent = typeof cause === 'string' ? cause : cause?.message || t('modelActionFailed');
        error.hidden = false;
      }
    }

    async function action(command, args) {
      if (busy) return;
      ++statusSequence;
      busy = true;
      error.hidden = true;
      render();
      try {
        render(await invoke(command, args));
      } catch (cause) {
        error.textContent = typeof cause === 'string' ? cause : cause?.message || t('modelActionFailed');
        error.hidden = false;
      } finally {
        busy = false;
        render();
      }
    }

    panel.hidden = false;
    download.addEventListener('click', () => action('mobile_download_model'));
    load.addEventListener('click', () => action('mobile_load_model'));
    start.addEventListener('click', () => {
      const port = Number(portInput.value);
      if (!portInput.value || !Number.isInteger(port) || port < 1 || port > 65535) {
        error.textContent = t('invalidPort');
        error.hidden = false;
        portInput.focus();
        return;
      }
      action('mobile_start_lan', { port });
    });
    stop.addEventListener('click', () => action('mobile_stop_lan'));
    panel.querySelectorAll('[data-copy]').forEach(button => button.addEventListener('click', async () => {
      const input = document.getElementById(button.dataset.copy);
      try {
        await navigator.clipboard.writeText(input.value);
        feedback.textContent = t('copied');
      } catch (_) {
        input.focus();
        input.select();
        feedback.textContent = t('selectToCopy');
      }
    }));
    window.addEventListener('focus', refresh);
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') refresh();
    });
    refresh();
    return { invoke, localize: () => render() };
  }

  window.LayaMobile = Object.freeze({ mount });
})();
