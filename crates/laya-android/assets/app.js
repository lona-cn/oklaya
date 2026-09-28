(() => {
  'use strict';

  const messages = {
    en: {
      pageTitle: 'Laya — Decision workspace', skipLink: 'Skip to workspace', brandTitle: 'Decision workspace',
      workspace: 'Workspace', navQuestion: 'Question', navResult: 'Result', language: 'Language',
      composeTitle: 'Define the situation', stateLabel: 'Current state', stateHint: 'Facts and context to consider.',
      statePlaceholder: 'Describe the situation, constraints, and relevant details…', typeLabel: 'What kind of answer?',
      typeChoice: 'Choose between options', typeScore: 'Score criteria', typeNoul: 'Yes or no',
      instructionsLabel: 'Instructions', instructionsHint: 'Ask one focused question.',
      instructionsPlaceholder: 'Which option best fits the situation?', options: 'Options', criteria: 'Criteria',
      optionsHint: 'Give each option a unique name and explain what it means.',
      criteriaHint: 'Add the dimensions you want the model to score.', addOption: '+ Add option',
      addCriterion: '+ Add criterion', optionName: 'Option name', criterion: 'Criterion',
      optionExample: 'e.g. Path A', criterionExample: 'e.g. Impact', description: 'What it means',
      descriptionPlaceholder: 'Describe this option', remove: 'Remove', removeOption: id => `Remove option ${id}`,
      removeCriterion: id => `Remove criterion ${id}`, runDecision: 'Run decision', reading: 'Reading…',
      resultTitle: 'The reading', idle: 'Ready when you are.', pending: 'Working through your question…',
      ready: 'Decision ready.', errorStatus: 'Could not complete the decision.',
      emptyResult: 'Answer appears here after you run a decision.', decision: 'DECISION',
      result: 'Result', recommended: 'Recommended option', overallScore: 'Overall score', yes: 'Yes', no: 'No',
      probabilityYes: probability => `Probability of yes · ${probability}`, confidence: 'Confidence',
      answerConfidence: 'Answer confidence', actionProbability: 'Action probability',
      breakdown: 'Probability breakdown', unexpectedAnswer: 'The API returned an unexpected answer.',
      unknownAnswer: 'The API returned an unknown answer type.', missingInstructions: 'Write instructions for your question.',
      missingOption: 'Add at least one option.', missingCriterion: 'Add at least one criterion.',
      emptyCriterion: 'Fill in every criterion or remove empty rows.',
      incompleteOption: 'Each option needs a name and description.',
      duplicateOption: name => `Option names must be unique: ${name}`, missingState: 'Describe the current state.',
      requestFailed: status => `The request failed (${status}).`, genericError: 'Something went wrong. Please try again.',
      offline: 'The inference service is unavailable. Please try again later.',
      connectionFailed: 'Could not connect to the server. Please try again.',
      deviceTitle: 'On-device inference', deviceHint: 'Download the model once, then load it to run decisions on this device.',
      downloadModel: 'Download model', loadModel: 'Load model', lanTitle: 'Share on your LAN',
      lanCaveat: 'Only start on a trusted network. The server stops if this app loses focus or goes into the background, and cannot persist there. HTTP is unencrypted: other devices on the network may see the token and your questions.',
      lanNeedsModel: 'Load the model before starting the LAN server.',
      portLabel: 'Port (1–65535)', startLan: 'Start LAN server', stopLan: 'Stop LAN server',
      urlLabel: 'LAN URL', tokenLabel: 'Bearer token', copyUrl: 'Copy URL', copyToken: 'Copy token',
      tokenWarning: 'Share this token only with people you trust. Anyone holding it can submit predictions while the server is running.',
      browserLanTitle: 'Connect to this device',
      browserLanHint: "Enter the bearer token shown on the Android device. It stays in this tab's memory and is sent only with predictions.",
      browserLanCaveat: 'Use only on a trusted LAN. HTTP is unencrypted; the token and your questions can be seen on the network.',
      modelReady: 'Model ready', modelNotReady: 'Model not loaded', provider: name => `Provider: ${name}`,
      lanRunning: port => `LAN server running on port ${port}`, lanStopped: 'LAN server stopped',
      deviceBusy: 'Working…', invalidPort: 'Choose a whole-number port from 1 to 65535.',
      missingToken: 'Enter the bearer token shown on the Android device.',
      invalidToken: 'The bearer token was rejected. Check the token on the Android device.',
      copied: 'Copied to clipboard.', selectToCopy: 'Clipboard unavailable. Select and copy the highlighted text.',
      modelActionFailed: 'Could not complete the device action.',
    },
    zh: {
      pageTitle: 'Laya — 决策工作台', skipLink: '跳转到工作台', brandTitle: '决策工作台',
      workspace: '工作台', navQuestion: '问题', navResult: '结果', language: '语言',
      composeTitle: '描述情况', stateLabel: '当前状态', stateHint: '提供需要考虑的事实和背景。',
      statePlaceholder: '描述情况、限制条件和相关细节…', typeLabel: '需要哪种回答？',
      typeChoice: '从选项中选择', typeScore: '评估标准', typeNoul: '是或否',
      instructionsLabel: '提问说明', instructionsHint: '提出一个明确的问题。',
      instructionsPlaceholder: '哪个选项最适合当前情况？', options: '选项', criteria: '标准',
      optionsHint: '为每个选项取一个不同的名称，并说明其含义。',
      criteriaHint: '添加需要模型评分的维度。', addOption: '+ 添加选项',
      addCriterion: '+ 添加标准', optionName: '选项名称', criterion: '标准',
      optionExample: '例如：方案 A', criterionExample: '例如：影响', description: '含义',
      descriptionPlaceholder: '描述这个选项', remove: '移除', removeOption: id => `移除选项 ${id}`,
      removeCriterion: id => `移除标准 ${id}`, runDecision: '开始决策', reading: '分析中…',
      resultTitle: '分析结果', idle: '等待提交问题。', pending: '正在分析你的问题…',
      ready: '决策已完成。', errorStatus: '无法完成决策。',
      emptyResult: '运行决策后，答案将在这里显示。', decision: '决策',
      result: '结果', recommended: '推荐选项', overallScore: '总体评分', yes: '是', no: '否',
      probabilityYes: probability => `回答“是”的概率 · ${probability}`, confidence: '置信度',
      answerConfidence: '答案置信度', actionProbability: '行动概率',
      breakdown: '概率明细', unexpectedAnswer: '接口返回了格式异常的答案。',
      unknownAnswer: '接口返回了未知类型的答案。', missingInstructions: '请填写问题说明。',
      missingOption: '请至少添加一个选项。', missingCriterion: '请至少添加一个标准。',
      emptyCriterion: '请填写所有标准，或移除空白行。',
      incompleteOption: '每个选项都需要名称和说明。',
      duplicateOption: name => `选项名称不能重复：${name}`, missingState: '请描述当前状态。',
      requestFailed: status => `请求失败（${status}）。`, genericError: '发生错误，请重试。',
      offline: '推理服务暂时不可用，请稍后重试。',
      connectionFailed: '无法连接到服务器，请重试。',
      deviceTitle: '设备端推理', deviceHint: '首次下载模型后，加载模型即可在本设备上运行决策。',
      downloadModel: '下载模型', loadModel: '加载模型', lanTitle: '在局域网共享',
      lanCaveat: '请仅在可信网络上启动。应用失去焦点或进入后台时服务会停止，无法在后台持续运行。HTTP 未加密：网络上的其他设备可能看到令牌和你的问题。',
      lanNeedsModel: '启动局域网服务前请先加载模型。',
      portLabel: '端口（1–65535）', startLan: '启动局域网服务', stopLan: '停止局域网服务',
      urlLabel: '局域网地址', tokenLabel: 'Bearer 令牌', copyUrl: '复制地址', copyToken: '复制令牌',
      tokenWarning: '仅与信任的人分享此令牌。持有令牌的人可在服务运行期间提交预测请求。',
      browserLanTitle: '连接到此设备',
      browserLanHint: '输入 Android 设备上显示的 Bearer 令牌。它仅保留在本标签页的内存中，并只随预测请求发送。',
      browserLanCaveat: '请仅在可信局域网上使用。HTTP 未加密，令牌和你的问题可能在网络上被看到。',
      modelReady: '模型已就绪', modelNotReady: '模型尚未加载', provider: name => `推理后端：${name}`,
      lanRunning: port => `局域网服务运行中，端口 ${port}`, lanStopped: '局域网服务已停止',
      deviceBusy: '处理中…', invalidPort: '请选择 1 到 65535 之间的整数端口。',
      missingToken: '请输入 Android 设备上显示的 Bearer 令牌。',
      invalidToken: 'Bearer 令牌被拒绝。请核对 Android 设备上的令牌。',
      copied: '已复制到剪贴板。', selectToCopy: '无法使用剪贴板。请选择并复制已高亮的文本。',
      modelActionFailed: '设备操作未完成。',
    },
  };

  const form = document.querySelector('#prediction-form');
  const type = document.querySelector('#question-type');
  const list = document.querySelector('#criteria-list');
  const section = document.querySelector('#criteria-section');
  const add = document.querySelector('#add-criterion');
  const title = document.querySelector('#criteria-title');
  const hint = document.querySelector('#criteria-hint');
  const result = document.querySelector('#result-content');
  const status = document.querySelector('#request-status');
  const error = document.querySelector('#form-error');
  const submit = document.querySelector('#submit-button');
  const languageSelect = document.querySelector('#language');
  const mobile = window.LayaMobile?.mount(t);
  const invoke = mobile?.invoke;
  const browserToken = document.querySelector('#browser-lan-token');
  if (!mobile) document.querySelector('#browser-lan-controls').hidden = false;
  let nextId = 0;
  let language = detectLanguage();
  let currentError = null;
  let currentResult = null;

  function detectLanguage() {
    try {
      const saved = localStorage.getItem('laya-language');
      if (saved === 'en' || saved === 'zh') return saved;
    } catch (_) { /* Storage may be disabled; browser preference still works. */ }
    const preferred = navigator.languages?.[0] || navigator.language || '';
    return /^zh(?:-|$)/i.test(preferred) ? 'zh' : 'en';
  }

  function t(key, ...args) {
    const value = messages[language][key];
    return typeof value === 'function' ? value(...args) : value;
  }

  function node(tag, className, text) {
    const element = document.createElement(tag);
    if (className) element.className = className;
    if (text !== undefined) element.textContent = text;
    return element;
  }

  function keyedError(key, ...args) {
    const cause = new Error();
    cause.translation = { key, args };
    return cause;
  }

  function errorText(descriptor) {
    return descriptor.key ? t(descriptor.key, ...(descriptor.args || [])) : descriptor.detail;
  }

  function setStatus(state) {
    status.dataset.state = state;
    status.textContent = t(state === 'error' ? 'errorStatus' : state);
  }

  function emptyResult() {
    const empty = node('div', 'empty-result');
    empty.append(node('p', '', t('emptyResult')));
    result.replaceChildren(empty);
  }

  function localizeCriteria() {
    const choice = type.value === 'choice';
    title.textContent = t(choice ? 'options' : 'criteria');
    hint.textContent = t(choice ? 'optionsHint' : 'criteriaHint');
    add.textContent = t(choice ? 'addOption' : 'addCriterion');
    for (const row of list.children) {
      const first = row.querySelector('.criterion-label');
      first.firstChild.textContent = t(choice ? 'optionName' : 'criterion');
      first.querySelector('input').placeholder = t(choice ? 'optionExample' : 'criterionExample');
      if (choice) {
        const description = row.querySelectorAll('.criterion-label')[1];
        description.firstChild.textContent = t('description');
        description.querySelector('input').placeholder = t('descriptionPlaceholder');
      }
      const remove = row.querySelector('.remove-button');
      remove.textContent = t('remove');
      remove.setAttribute('aria-label', t(choice ? 'removeOption' : 'removeCriterion', row.dataset.id));
    }
  }

  function localize() {
    document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
    document.title = t('pageTitle');
    languageSelect.value = language;
    document.querySelectorAll('[data-i18n]').forEach(element => { element.textContent = t(element.dataset.i18n); });
    document.querySelectorAll('[data-i18n-placeholder]').forEach(element => { element.placeholder = t(element.dataset.i18nPlaceholder); });
    document.querySelectorAll('[data-i18n-aria]').forEach(element => { element.setAttribute('aria-label', t(element.dataset.i18nAria)); });
    localizeCriteria();
    mobile?.localize();
    submit.querySelector('#submit-label').textContent = t(submit.disabled ? 'reading' : 'runDecision');
    setStatus(status.dataset.state);
    if (currentError) error.textContent = errorText(currentError);
    if (currentResult) renderResult(currentResult);
    else emptyResult();
  }

  function field(name, value, id, placeholder) {
    const label = node('label', 'criterion-label', t(name));
    label.htmlFor = id;
    const input = node('input', 'criterion-input');
    input.id = id;
    input.type = 'text';
    input.value = value;
    input.placeholder = t(placeholder);
    input.required = true;
    label.append(input);
    return label;
  }

  function addCriterion(values = [], focus = true) {
    const id = ++nextId;
    const row = node('div', 'criterion-row');
    row.dataset.id = id;
    row.append(field(type.value === 'choice' ? 'optionName' : 'criterion', values[0] || '', `criterion-${id}`, type.value === 'choice' ? 'optionExample' : 'criterionExample'));
    if (type.value === 'choice') row.append(field('description', values[1] || '', `description-${id}`, 'descriptionPlaceholder'));
    const remove = node('button', 'remove-button', t('remove'));
    remove.type = 'button';
    remove.setAttribute('aria-label', t(type.value === 'choice' ? 'removeOption' : 'removeCriterion', id));
    remove.addEventListener('click', () => row.remove());
    row.append(remove);
    list.append(row);
    if (focus) row.querySelector('input').focus();
  }

  function updateType() {
    const isNoul = type.value === 'noul';
    section.hidden = isNoul;
    list.replaceChildren();
    if (isNoul) return;
    localizeCriteria();
    addCriterion([], false);
    addCriterion([], false);
  }

  function formatProbability(value) {
    return typeof value === 'number' && Number.isFinite(value) ? `${(value * 100).toFixed(1)}%` : '—';
  }

  function metric(container, label, value, probability) {
    const item = node('div', 'metric');
    item.append(node('dt', '', label), node('dd', '', value));
    if (typeof probability === 'number' && Number.isFinite(probability) && probability >= 0 && probability <= 1) {
      const track = node('span', 'probability-track');
      track.setAttribute('aria-hidden', 'true');
      const fill = node('span', 'probability-fill');
      fill.style.width = `${probability * 100}%`;
      track.append(fill);
      item.append(track);
    }
    container.append(item);
  }

  function renderResult(data) {
    const answer = data?.result;
    if (!answer || typeof answer !== 'object') throw keyedError('unexpectedAnswer');
    if (!['choice', 'score', 'noul'].includes(answer.type)) throw keyedError('unknownAnswer');
    const card = node('article', 'answer-card');
    card.append(node('span', 'answer-kicker', `${t(answer.type === 'noul' ? 'typeNoul' : answer.type === 'score' ? 'typeScore' : 'typeChoice')} / ${t('decision')}`));
    if (answer.type === 'choice') {
      card.append(node('h3', 'answer-value', answer.choice ?? '—'), node('p', 'answer-caption', t('recommended')));
    } else if (answer.type === 'score') {
      card.append(node('h3', 'answer-value', typeof answer.score === 'number' ? answer.score.toFixed(2) : '—'), node('p', 'answer-caption', t('overallScore')));
    } else {
      card.append(node('h3', 'answer-value', answer.value === true ? t('yes') : answer.value === false ? t('no') : '—'), node('p', 'answer-caption', t('probabilityYes', formatProbability(answer.probability))));
    }
    const metrics = node('dl', 'metrics');
    metric(metrics, t('confidence'), formatProbability(answer.confidence), answer.confidence);
    metric(metrics, t('answerConfidence'), formatProbability(answer.answer_confidence), answer.answer_confidence);
    metric(metrics, t('actionProbability'), formatProbability(answer.action_probability), answer.action_probability);
    const children = [card, metrics];
    if (answer.probabilities && typeof answer.probabilities === 'object' && Object.keys(answer.probabilities).length) {
      const breakdown = node('section', 'breakdown');
      breakdown.append(node('h3', '', t('breakdown')));
      const entries = node('dl', 'breakdown-list');
      for (const [key, value] of Object.entries(answer.probabilities)) {
        const row = node('div', 'breakdown-row');
        const label = answer.legend?.[key] ? `${key} · ${answer.legend[key]}` : key;
        metric(row, label, formatProbability(value), value);
        entries.append(row);
      }
      breakdown.append(entries);
      children.push(breakdown);
    }
    result.replaceChildren(...children);
  }

  function buildQuestion() {
    const kind = type.value;
    const instructions = document.querySelector('#instructions').value.trim();
    if (!instructions) throw keyedError('missingInstructions');
    if (kind === 'noul') return { type: kind, instructions, criteria: {} };
    const rows = [...list.querySelectorAll('.criterion-row')];
    if (!rows.length) throw keyedError(kind === 'choice' ? 'missingOption' : 'missingCriterion');
    if (kind === 'score') {
      const criteria = rows.map(row => row.querySelector('input').value.trim());
      if (criteria.some(value => !value)) throw keyedError('emptyCriterion');
      return { type: kind, instructions, criteria };
    }
    const criteria = Object.create(null);
    for (const row of rows) {
      const [name, description] = [...row.querySelectorAll('input')].map(input => input.value.trim());
      if (!name || !description) throw keyedError('incompleteOption');
      if (Object.hasOwn(criteria, name)) throw keyedError('duplicateOption', name);
      criteria[name] = description;
    }
    return { type: kind, instructions, criteria };
  }

  function showError(descriptor) {
    currentError = descriptor;
    error.textContent = errorText(descriptor);
    error.hidden = false;
    setStatus('error');
    error.focus();
  }

  languageSelect.addEventListener('change', () => {
    language = languageSelect.value === 'zh' ? 'zh' : 'en';
    try { localStorage.setItem('laya-language', language); } catch (_) { /* Keep the in-memory choice. */ }
    localize();
  });
  type.addEventListener('change', () => { error.hidden = true; currentError = null; updateType(); });
  add.addEventListener('click', () => addCriterion());
  form.noValidate = true;
  form.addEventListener('submit', async event => {
    event.preventDefault();
    error.hidden = true;
    currentError = null;
    // Validate in the selected language rather than showing the browser's locale-dependent tooltip.
    let question;
    try {
      question = buildQuestion();
      const state = document.querySelector('#state').value.trim();
      if (!state) throw keyedError('missingState');
      submit.disabled = true;
      submit.querySelector('#submit-label').textContent = t('reading');
      setStatus('pending');
      if (!invoke) {
        browserToken.focus();
        if (!browserToken.value.trim()) throw keyedError('missingToken');
      }
      const request = { state, questions: { result: question } };
      let data;
      if (invoke) {
        data = await invoke('mobile_predict', { request });
      } else {
        const token = browserToken.value.trim();
        let response;
        try {
          response = await fetch('/api/v1/predictions', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: `Bearer ${token}`,
            },
            body: JSON.stringify(request),
          });
        } catch (cause) {
          if (cause instanceof TypeError) throw keyedError('connectionFailed');
          throw cause;
        }
        data = await response.json();
        if (!response.ok) {
          if (response.status === 401) throw keyedError('invalidToken');
          if (data?.error?.code === 'upstream_unavailable' || data?.error?.message === messages.en.offline) throw keyedError('offline');
          if (data?.error?.message) throw new Error(data.error.message);
          throw keyedError('requestFailed', response.status);
        }
      }
      renderResult(data);
      currentResult = data;
      setStatus('ready');
    } catch (cause) {
      showError(cause?.translation || (typeof cause === 'string' && cause ? { detail: cause } : cause instanceof Error && cause.message ? { detail: cause.message } : { key: 'genericError' }));
    } finally {
      submit.disabled = false;
      submit.querySelector('#submit-label').textContent = t('runDecision');
    }
  });
  updateType();
  localize();
  document.querySelector('#state').focus();
})();
