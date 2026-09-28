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
      let response;
      try {
        response = await fetch('/api/v1/predictions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ state, questions: { result: question } }),
        });
      } catch (cause) {
        if (cause instanceof TypeError) throw keyedError('connectionFailed');
        throw cause;
      }
      const data = await response.json();
      if (!response.ok) {
        if (data?.error?.code === 'upstream_unavailable' || data?.error?.message === messages.en.offline) throw keyedError('offline');
        if (data?.error?.message) throw new Error(data.error.message);
        throw keyedError('requestFailed', response.status);
      }
      renderResult(data);
      currentResult = data;
      setStatus('ready');
    } catch (cause) {
      showError(cause?.translation || (cause instanceof Error && cause.message ? { detail: cause.message } : { key: 'genericError' }));
    } finally {
      submit.disabled = false;
      submit.querySelector('#submit-label').textContent = t('runDecision');
    }
  });
  updateType();
  localize();
  document.querySelector('#state').focus();
})();
