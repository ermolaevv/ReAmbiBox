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
  const sliderBrightness = document.getElementById('slider-brightness');
  const valBrightness = document.getElementById('val-brightness');
  const sliderSmoothing = document.getElementById('slider-smoothing');
  const valSmoothing = document.getElementById('val-smoothing');
  const sliderGamma = document.getElementById('slider-gamma');
  const valGamma = document.getElementById('val-gamma');
  const toggleLetterbox = document.getElementById('toggle-letterbox');
  const systemStatus = document.getElementById('system-status');
  const canvas = document.getElementById('monitor-canvas');

  let currentConfig = null;

  // Отправка обновленных настроек на бэкенд
  async function updateConfig(partial) {
    try {
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(partial)
      });
    } catch (e) {
      console.error('Ошибка отправки настроек:', e);
    }
  }

  // Привязка ползунка яркости
  sliderBrightness?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valBrightness.textContent = val + '%';
    updateConfig({ brightness: val / 100.0 });
  });

  // Привязка сглаживания
  sliderSmoothing?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valSmoothing.textContent = (val / 100).toFixed(2);
    updateConfig({ smoothing: val / 100.0 });
  });

  // Привязка гаммы
  sliderGamma?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valGamma.textContent = (val / 10).toFixed(1);
    updateConfig({ gamma: val / 10.0 });
  });

  // Привязка детектора 21:9
  toggleLetterbox?.addEventListener('change', (e) => {
    updateConfig({ letterbox: e.target.checked });
  });

  // Кнопка включения/выключения
  btnPower?.addEventListener('click', async () => {
    if (!currentConfig) return;
    const newState = !currentConfig.is_active;
    await updateConfig({ is_active: newState });
    currentConfig.is_active = newState;
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

    zones.forEach((z, idx) => {
      const box = document.createElement('div');
      box.className = 'zone-box';
      box.style.left = (z.rect.x * 100) + '%';
      box.style.top = (z.rect.y * 100) + '%';
      box.style.width = (z.rect.width * 100) + '%';
      box.style.height = (z.rect.height * 100) + '%';
      box.textContent = (z.id + 1); // 1-based для соответствия AmbiBox
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
        systemStatus.textContent = `${data.is_active ? 'Активен' : 'Выключен'} (${data.serial_port}, ${data.fps.toFixed(1)} FPS)`;
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
        renderZones(data.zones);
      }
    } catch (e) {
      console.warn('Сервер ReAmbiBox недоступен:', e);
    }
  }

  fetchStatus();
  setInterval(fetchStatus, 1500);
});
