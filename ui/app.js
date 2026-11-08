document.addEventListener('DOMContentLoaded', () => {
  // Навигация по вкладкам
  const navItems = document.querySelectorAll('.nav-item');
  const tabPanes = document.querySelectorAll('.tab-pane');

  navItems.forEach(item => {
    item.addEventListener('click', () => {
      navItems.forEach(n => n.classList.remove('active'));
      tabPanes.forEach(p => p.classList.remove('active'));

      item.classList.add('active');
      const tabId = 'tab-' + item.dataset.tab;
      document.getElementById(tabId)?.classList.add('active');
    });
  });

  const btnPower = document.getElementById('toggle-power');
  const btnModeToggle = document.getElementById('btn-mode-toggle');
  const btnModeScreen = document.getElementById('btn-mode-screen');
  const btnModeStatic = document.getElementById('btn-mode-static');

  const sliderBrightness = document.getElementById('slider-brightness');
  const valBrightness = document.getElementById('val-brightness');
  const sliderSmoothing = document.getElementById('slider-smoothing');
  const valSmoothing = document.getElementById('val-smoothing');
  const sliderGamma = document.getElementById('slider-gamma');
  const valGamma = document.getElementById('val-gamma');
  const toggleLetterbox = document.getElementById('toggle-letterbox');

  const selectMonitor = document.getElementById('select-monitor');
  const selectColorOrder = document.getElementById('select-color-order');
  const systemStatus = document.getElementById('system-status');
  const canvas = document.getElementById('monitor-canvas');

  let currentConfig = null;
  let updateTimer = null;
  let pendingUpdates = {};

  // Быстрая отправка с дебаунсом 30 мс
  function queueConfigUpdate(partial) {
    Object.assign(pendingUpdates, partial);
    if (updateTimer) clearTimeout(updateTimer);
    updateTimer = setTimeout(async () => {
      const payload = { ...pendingUpdates };
      pendingUpdates = {};
      try {
        await fetch('/api/config', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
      } catch (e) {
        console.error('Ошибка отправки настроек:', e);
      }
    }, 30);
  }

  // Привязка ползунка яркости
  sliderBrightness?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valBrightness.textContent = val + '%';
    queueConfigUpdate({ brightness: val / 100.0 });
  });

  // Привязка сглаживания
  sliderSmoothing?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valSmoothing.textContent = (val / 100).toFixed(2);
    queueConfigUpdate({ smoothing: val / 100.0 });
  });

  // Привязка гаммы
  sliderGamma?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valGamma.textContent = (val / 10).toFixed(1);
    queueConfigUpdate({ gamma: val / 10.0 });
  });

  // Привязка детектора 21:9
  toggleLetterbox?.addEventListener('change', (e) => {
    queueConfigUpdate({ letterbox: e.target.checked });
  });

  // Переключение режимов
  function setMode(mode) {
    if (mode === 'static') {
      btnModeStatic?.classList.remove('btn-secondary');
      btnModeStatic?.classList.add('btn-primary');
      btnModeScreen?.classList.remove('btn-primary');
      btnModeScreen?.classList.add('btn-secondary');
      if (btnModeToggle) btnModeToggle.textContent = 'Вернуть: Захват экрана';
    } else {
      btnModeScreen?.classList.remove('btn-secondary');
      btnModeScreen?.classList.add('btn-primary');
      btnModeStatic?.classList.remove('btn-primary');
      btnModeStatic?.classList.add('btn-secondary');
      if (btnModeToggle) btnModeToggle.textContent = 'Тест белым (100% LED)';
    }
    queueConfigUpdate({ mode });
  }

  btnModeScreen?.addEventListener('click', () => setMode('screen'));
  btnModeStatic?.addEventListener('click', () => setMode('static'));

  btnModeToggle?.addEventListener('click', () => {
    if (!currentConfig) return;
    const newMode = (currentConfig.mode === 'static') ? 'screen' : 'static';
    currentConfig.mode = newMode;
    setMode(newMode);
  });

  // Выбор монитора
  selectMonitor?.addEventListener('change', (e) => {
    const mon = parseInt(e.target.value);
    queueConfigUpdate({ selected_monitor: mon });
  });

  // Выбор порядка цветов (RGB / GRB)
  selectColorOrder?.addEventListener('change', (e) => {
    queueConfigUpdate({ color_order: e.target.value });
  });

  // Кнопка включения/выключения
  btnPower?.addEventListener('click', async () => {
    if (!currentConfig) return;
    const newState = !currentConfig.is_active;
    currentConfig.is_active = newState;
    queueConfigUpdate({ is_active: newState });
    updatePowerButton(newState);
  });

  function updatePowerButton(isActive) {
    if (btnPower) {
      if (isActive) {
        btnPower.textContent = 'Выключить подсветку';
        btnPower.className = 'btn btn-primary';
      } else {
        btnPower.textContent = 'Включить подсветку';
        btnPower.className = 'btn btn-secondary';
      }
    }
  }

  // Отрисовка зон
  function renderZones(zones) {
    if (!canvas || !zones) return;
    canvas.innerHTML = '';

    zones.forEach((z) => {
      const box = document.createElement('div');
      box.className = 'zone-box';
      box.style.left = (z.rect.x * 100) + '%';
      box.style.top = (z.rect.y * 100) + '%';
      box.style.width = (z.rect.width * 100) + '%';
      box.style.height = (z.rect.height * 100) + '%';
      box.textContent = (z.id + 1);
      box.title = `Зона ${z.id + 1}: [x=${(z.rect.x*100).toFixed(1)}%, y=${(z.rect.y*100).toFixed(1)}%]`;
      canvas.appendChild(box);
    });
  }

  // Загрузка состояния с бэкенда
  async function fetchStatus() {
    try {
      const res = await fetch('/api/config');
      if (!res.ok) return;
      const data = await res.json();
      currentConfig = data;

      // Обновляем статус в сайдбаре
      if (systemStatus) {
        systemStatus.textContent = `${data.is_active ? 'Активен' : 'Выключен'} | Экран: ${data.capture_fps.toFixed(0)} FPS | Порт: ${data.serial_fps.toFixed(0)} FPS`;
      }

      updatePowerButton(data.is_active);

      // Синхронизируем контролы только при первой загрузке
      if (!window.__initialized) {
        window.__initialized = true;

        if (sliderBrightness) {
          sliderBrightness.value = Math.round(data.brightness * 100);
          valBrightness.textContent = sliderBrightness.value + '%';
        }
        if (sliderSmoothing) {
          sliderSmoothing.value = Math.round(data.smoothing * 100);
          valSmoothing.textContent = (data.smoothing).toFixed(2);
        }
        if (sliderGamma) {
          sliderGamma.value = Math.round(data.gamma * 10);
          valGamma.textContent = (data.gamma).toFixed(1);
        }
        if (toggleLetterbox) {
          toggleLetterbox.checked = data.letterbox;
        }

        // Заполняем мониторы
        if (selectMonitor && data.monitors) {
          selectMonitor.innerHTML = '';
          data.monitors.forEach(m => {
            const opt = document.createElement('option');
            opt.value = m.index;
            opt.textContent = m.name + (m.is_primary ? ' [Основной]' : '');
            if (m.index === data.selected_monitor) opt.selected = true;
            selectMonitor.appendChild(opt);
          });
        }

        // Заполняем порядок цветов
        if (selectColorOrder && data.color_order) {
          selectColorOrder.value = data.color_order;
        }

        setMode(data.mode || 'screen');
        renderZones(data.zones);
      }
    } catch (e) {
      console.warn('Сервер ReAmbiBox недоступен:', e);
    }
  }

  fetchStatus();
  setInterval(fetchStatus, 1000);
});
