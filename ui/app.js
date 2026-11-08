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
  const btnModeMusic = document.getElementById('btn-mode-music');
  const cardMusicSettings = document.getElementById('card-music-settings');
  const selectAudioEffect = document.getElementById('select-audio-effect');
  const sliderAudioSensitivity = document.getElementById('slider-audio-sensitivity');
  const valAudioSensitivity = document.getElementById('val-audio-sensitivity');

  const sliderBrightness = document.getElementById('slider-brightness');
  const valBrightness = document.getElementById('val-brightness');
  const sliderSmoothing = document.getElementById('slider-smoothing');
  const valSmoothing = document.getElementById('val-smoothing');
  const sliderGamma = document.getElementById('slider-gamma');
  const valGamma = document.getElementById('val-gamma');
  const toggleLetterbox = document.getElementById('toggle-letterbox');
  const toggleAutostart = document.getElementById('toggle-autostart');
  const sliderFps = document.getElementById('slider-fps');
  const valFps = document.getElementById('val-fps');
  const selectBaud = document.getElementById('select-baud');
  const selectPort = document.getElementById('select-port');
  const btnReconnectPort = document.getElementById('btn-reconnect-port');

  const selectOutputType = document.getElementById('select-output-type');
  const groupSerialSettings = document.getElementById('group-serial-settings');
  const groupWledSettings = document.getElementById('group-wled-settings');
  const inputWledHost = document.getElementById('input-wled-host');
  const inputWledPort = document.getElementById('input-wled-port');
  const selectWledProtocol = document.getElementById('select-wled-protocol');
  const btnTestWled = document.getElementById('btn-test-wled');
  const wledTestStatus = document.getElementById('wled-test-status');

  const btnToggleOverlay = document.getElementById('btn-toggle-overlay');
  const btnOpenWizard = document.getElementById('btn-open-wizard');
  const btnCloseWizard = document.getElementById('btn-close-wizard');
  const modalWizard = document.getElementById('modal-wizard');
  const wizDepth = document.getElementById('wiz-depth');
  const valWizDepth = document.getElementById('val-wiz-depth');
  const btnPreset98 = document.getElementById('btn-preset-98');
  const btnApplyWizard = document.getElementById('btn-apply-wizard');

  const selectMonitor = document.getElementById('select-monitor');
  const selectColorOrder = document.getElementById('select-color-order');
  const systemStatus = document.getElementById('system-status');
  const canvas = document.getElementById('monitor-canvas');

  const btnSaveZones = document.getElementById('btn-save-zones');
  const infoZoneId = document.getElementById('info-zone-id');
  const infoZoneCoords = document.getElementById('info-zone-coords');
  const toggleZoneSnapping = document.getElementById('toggle-zone-snapping');

  let currentConfig = null;
  let updateTimer = null;
  let pendingUpdates = {};

  let activeSelectedZoneId = null;
  let isInteractingWithZones = false;
  let interactionMode = null;
  let interactionZone = null;

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

  // Привязка автозапуска Windows
  toggleAutostart?.addEventListener('change', (e) => {
    queueConfigUpdate({ autostart: e.target.checked });
  });

  // Привязка целевого FPS
  sliderFps?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value);
    valFps.textContent = val + ' FPS';
    queueConfigUpdate({ target_fps: val });
  });

  // Выбор скорости порта
  selectBaud?.addEventListener('change', (e) => {
    const baud = parseInt(e.target.value);
    queueConfigUpdate({ baud_rate: baud });
  });

  // Выбор COM-порта
  selectPort?.addEventListener('change', (e) => {
    queueConfigUpdate({ serial_port: e.target.value, reconnect: true });
  });

  // Кнопка аппаратного сброса / переподключения контроллера
  btnReconnectPort?.addEventListener('click', () => {
    btnReconnectPort.textContent = 'Сброс...';
    btnReconnectPort.disabled = true;
    queueConfigUpdate({ reconnect: true });
    setTimeout(() => {
      btnReconnectPort.textContent = 'Сброс / Переподключение';
      btnReconnectPort.disabled = false;
    }, 1500);
  });

  // Переключение типа вывода (USB Serial vs WLED Wi-Fi)
  function updateOutputTypeVisibility(type) {
    const isWled = type === 'wled';
    if (groupSerialSettings) groupSerialSettings.style.display = isWled ? 'none' : 'block';
    if (groupWledSettings) groupWledSettings.style.display = isWled ? 'block' : 'none';
  }

  selectOutputType?.addEventListener('change', (e) => {
    const val = e.target.value;
    updateOutputTypeVisibility(val);
    queueConfigUpdate({ output_type: val, reconnect: true });
  });

  inputWledHost?.addEventListener('change', (e) => {
    const host = e.target.value.trim();
    if (host) {
      queueConfigUpdate({ wled_host: host, reconnect: true });
    }
  });

  inputWledPort?.addEventListener('change', (e) => {
    const port = parseInt(e.target.value);
    if (!isNaN(port) && port > 0) {
      queueConfigUpdate({ wled_port: port, reconnect: true });
    }
  });

  selectWledProtocol?.addEventListener('change', (e) => {
    queueConfigUpdate({ wled_protocol: e.target.value, reconnect: true });
  });

  btnTestWled?.addEventListener('click', async () => {
    if (!btnTestWled || btnTestWled.disabled) return;
    btnTestWled.disabled = true;
    btnTestWled.textContent = 'Отправка...';
    if (wledTestStatus) {
      wledTestStatus.textContent = '';
      wledTestStatus.style.color = 'var(--text-muted)';
    }

    try {
      const res = await fetch('/api/wled/test', { method: 'POST' });
      const data = await res.json();
      if (wledTestStatus) {
        wledTestStatus.textContent = data.message || 'Готово';
        wledTestStatus.style.color = data.status === 'ok' ? '#4ade80' : '#f87171';
      }
    } catch (err) {
      if (wledTestStatus) {
        wledTestStatus.textContent = 'Ошибка связи с бэкендом';
        wledTestStatus.style.color = '#f87171';
      }
    } finally {
      setTimeout(() => {
        if (btnTestWled) {
          btnTestWled.disabled = false;
          btnTestWled.textContent = 'Тестовый кадр (Blink)';
        }
      }, 1000);
    }
  });

  // Переключение режимов
  function setMode(mode) {
    btnModeScreen?.classList.remove('btn-primary');
    btnModeScreen?.classList.add('btn-secondary');
    btnModeStatic?.classList.remove('btn-primary');
    btnModeStatic?.classList.add('btn-secondary');
    btnModeMusic?.classList.remove('btn-primary');
    btnModeMusic?.classList.add('btn-secondary');

    if (cardMusicSettings) {
      cardMusicSettings.style.display = (mode === 'music') ? 'block' : 'none';
    }

    if (mode === 'static') {
      btnModeStatic?.classList.remove('btn-secondary');
      btnModeStatic?.classList.add('btn-primary');
      if (btnModeToggle) btnModeToggle.textContent = 'Вернуть: Экран';
    } else if (mode === 'music') {
      btnModeMusic?.classList.remove('btn-secondary');
      btnModeMusic?.classList.add('btn-primary');
      if (btnModeToggle) btnModeToggle.textContent = 'Вернуть: Экран';
    } else {
      btnModeScreen?.classList.remove('btn-secondary');
      btnModeScreen?.classList.add('btn-primary');
      if (btnModeToggle) btnModeToggle.textContent = 'Тест белым (100% LED)';
    }
    queueConfigUpdate({ mode });
  }

  btnModeScreen?.addEventListener('click', () => setMode('screen'));
  btnModeStatic?.addEventListener('click', () => setMode('static'));
  btnModeMusic?.addEventListener('click', () => setMode('music'));

  selectAudioEffect?.addEventListener('change', (e) => {
    queueConfigUpdate({ audio_effect: e.target.value });
  });

  sliderAudioSensitivity?.addEventListener('input', (e) => {
    const val = parseInt(e.target.value) / 100.0;
    if (valAudioSensitivity) valAudioSensitivity.textContent = val.toFixed(1) + 'x';
    queueConfigUpdate({ audio_sensitivity: val });
  });

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

  // Управление оверлеем
  btnToggleOverlay?.addEventListener('click', async () => {
    try {
      const res = await fetch('/api/overlay/toggle', { method: 'POST' });
      const data = await res.json();
      updateOverlayButton(data.is_visible);
    } catch (e) {
      console.error('Ошибка переключения оверлея:', e);
    }
  });

  function updateOverlayButton(isVisible) {
    if (btnToggleOverlay) {
      if (isVisible) {
        btnToggleOverlay.textContent = 'Скрыть зоны на экране (Оверлей)';
        btnToggleOverlay.className = 'btn btn-primary';
      } else {
        btnToggleOverlay.textContent = 'Показать зоны на экране (Оверлей)';
        btnToggleOverlay.className = 'btn btn-secondary';
      }
    }
  }

  // Модальное окно мастера
  btnOpenWizard?.addEventListener('click', () => {
    if (modalWizard) modalWizard.style.display = 'flex';
  });

  btnCloseWizard?.addEventListener('click', () => {
    if (modalWizard) modalWizard.style.display = 'none';
  });

  wizDepth?.addEventListener('input', (e) => {
    if (valWizDepth) valWizDepth.textContent = e.target.value + '%';
  });

  btnPreset98?.addEventListener('click', () => {
    const elTop = document.getElementById('wiz-top');
    const elBottom = document.getElementById('wiz-bottom');
    const elLeft = document.getElementById('wiz-left');
    const elRight = document.getElementById('wiz-right');
    const elCorner = document.getElementById('wiz-corner');
    const elDir = document.getElementById('wiz-direction');
    const elDepth = document.getElementById('wiz-depth');

    if (elTop) elTop.value = 26;
    if (elBottom) elBottom.value = 25;
    if (elLeft) elLeft.value = 24;
    if (elRight) elRight.value = 23;
    if (elCorner) elCorner.value = 'BottomLeft';
    if (elDir) elDir.value = 'Clockwise';
    if (elDepth) elDepth.value = 8;
    if (valWizDepth) valWizDepth.textContent = '8%';
  });

  btnApplyWizard?.addEventListener('click', async () => {
    const params = {
      top_count: parseInt(document.getElementById('wiz-top')?.value) || 0,
      bottom_count: parseInt(document.getElementById('wiz-bottom')?.value) || 0,
      left_count: parseInt(document.getElementById('wiz-left')?.value) || 0,
      right_count: parseInt(document.getElementById('wiz-right')?.value) || 0,
      start_corner: document.getElementById('wiz-corner')?.value || 'BottomLeft',
      direction: document.getElementById('wiz-direction')?.value || 'Clockwise',
      depth: (parseInt(document.getElementById('wiz-depth')?.value) || 8) / 100.0,
    };

    try {
      btnApplyWizard.textContent = 'Применение...';
      btnApplyWizard.disabled = true;
      await fetch('/api/zones/wizard', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(params),
      });
      if (modalWizard) modalWizard.style.display = 'none';
      fetchStatus();
    } catch (e) {
      console.error('Ошибка применения мастера:', e);
    } finally {
      btnApplyWizard.textContent = 'Применить к экрану';
      btnApplyWizard.disabled = false;
    }
  });

  function updateZoneElement(box, z) {
    box.style.left = (z.rect.x * 100) + '%';
    box.style.top = (z.rect.y * 100) + '%';
    box.style.width = (z.rect.width * 100) + '%';
    box.style.height = (z.rect.height * 100) + '%';
    if (activeSelectedZoneId === z.id) {
      box.classList.add('active-zone');
    } else {
      box.classList.remove('active-zone');
    }
  }

  function updateZoneInfo(z) {
    if (!z || !infoZoneId) return;
    infoZoneId.textContent = `№${z.id + 1}`;
    if (infoZoneCoords) {
      infoZoneCoords.textContent = `[X: ${(z.rect.x * 100).toFixed(1)}%, Y: ${(z.rect.y * 100).toFixed(1)}%, W: ${(z.rect.width * 100).toFixed(1)}%, H: ${(z.rect.height * 100).toFixed(1)}%]`;
    }
  }

  function selectZone(id) {
    activeSelectedZoneId = id;
    const all = canvas.querySelectorAll('.zone-box');
    all.forEach(b => {
      if (parseInt(b.dataset.id) === id) {
        b.classList.add('active-zone');
      } else {
        b.classList.remove('active-zone');
      }
    });
    if (currentConfig && currentConfig.zones) {
      const z = currentConfig.zones.find(x => x.id === id);
      if (z) updateZoneInfo(z);
    }
  }

  btnSaveZones?.addEventListener('click', () => {
    if (currentConfig && currentConfig.zones) {
      queueConfigUpdate({ zones: currentConfig.zones });
      btnSaveZones.textContent = 'Сохранено!';
      setTimeout(() => {
        btnSaveZones.textContent = 'Сохранить зоны';
      }, 1200);
    }
  });

  // Отрисовка зон и привязка Drag & Drop событий
  function renderZones(zones) {
    if (!canvas || !zones) return;
    if (isInteractingWithZones) return; // Не прерывать перетаскивание мыши

    canvas.innerHTML = '';

    zones.forEach((z) => {
      const box = document.createElement('div');
      box.className = 'zone-box';
      box.dataset.id = z.id;
      box.textContent = (z.id + 1);
      box.title = `Зона ${z.id + 1}: перетаскивайте мышкой или меняйте размер`;

      const handle = document.createElement('div');
      handle.className = 'zone-resize-handle';
      handle.title = 'Потяните для изменения размера';
      box.appendChild(handle);

      updateZoneElement(box, z);

      box.addEventListener('pointerdown', (e) => {
        e.preventDefault();
        e.stopPropagation();

        selectZone(z.id);

        const isResize = e.target === handle;
        interactionMode = isResize ? 'resize' : 'drag';
        interactionZone = z;
        isInteractingWithZones = true;

        box.classList.add('dragging');
        box.setPointerCapture(e.pointerId);

        const rect = canvas.getBoundingClientRect();
        const startPointerX = e.clientX;
        const startPointerY = e.clientY;
        const startZoneX = z.rect.x;
        const startZoneY = z.rect.y;
        const startZoneW = z.rect.width;
        const startZoneH = z.rect.height;

        const onPointerMove = (ev) => {
          if (!isInteractingWithZones || !interactionZone) return;
          const dx = (ev.clientX - startPointerX) / rect.width;
          const dy = (ev.clientY - startPointerY) / rect.height;
          const snapping = toggleZoneSnapping ? toggleZoneSnapping.checked : true;

          if (interactionMode === 'drag') {
            let newX = Math.max(0, Math.min(1 - startZoneW, startZoneX + dx));
            let newY = Math.max(0, Math.min(1 - startZoneH, startZoneY + dy));

            if (snapping) {
              if (newX < 0.015) newX = 0;
              if (newX + startZoneW > 0.985) newX = 1 - startZoneW;
              if (newY < 0.015) newY = 0;
              if (newY + startZoneH > 0.985) newY = 1 - startZoneH;
            }

            interactionZone.rect.x = Math.round(newX * 1000) / 1000;
            interactionZone.rect.y = Math.round(newY * 1000) / 1000;
          } else if (interactionMode === 'resize') {
            let newW = Math.max(0.02, Math.min(1 - startZoneX, startZoneW + dx));
            let newH = Math.max(0.02, Math.min(1 - startZoneY, startZoneH + dy));

            if (snapping) {
              if (startZoneX + newW > 0.985) newW = 1 - startZoneX;
              if (startZoneY + newH > 0.985) newH = 1 - startZoneY;
            }

            interactionZone.rect.width = Math.round(newW * 1000) / 1000;
            interactionZone.rect.height = Math.round(newH * 1000) / 1000;
          }

          updateZoneElement(box, interactionZone);
          updateZoneInfo(interactionZone);
        };

        const onPointerUp = (ev) => {
          box.classList.remove('dragging');
          try {
            box.releasePointerCapture(ev.pointerId);
          } catch (_) {}
          box.removeEventListener('pointermove', onPointerMove);
          box.removeEventListener('pointerup', onPointerUp);
          isInteractingWithZones = false;
          interactionMode = null;
          interactionZone = null;

          // Сохраняем обновленные координаты зон
          if (currentConfig && currentConfig.zones) {
            queueConfigUpdate({ zones: currentConfig.zones });
          }
        };

        box.addEventListener('pointermove', onPointerMove);
        box.addEventListener('pointerup', onPointerUp);
      });

      canvas.appendChild(box);
    });

    if (activeSelectedZoneId !== null) {
      selectZone(activeSelectedZoneId);
    }
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
        const outLabel = data.output_type === 'wled' ? 'WLED' : 'Порт';
        systemStatus.textContent = `${data.is_active ? 'Активен' : 'Выключен'} | Экран: ${data.capture_fps.toFixed(0)} FPS | ${outLabel}: ${data.serial_fps.toFixed(0)} FPS`;
      }

      updatePowerButton(data.is_active);
      updateOverlayButton(data.is_overlay_visible);
      renderZones(data.zones);

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
        if (toggleAutostart) {
          toggleAutostart.checked = data.autostart;
        }
        if (sliderFps && data.target_fps) {
          sliderFps.value = data.target_fps;
          valFps.textContent = data.target_fps + ' FPS';
        }
        if (selectBaud && data.baud_rate) {
          selectBaud.value = data.baud_rate.toString();
        }

        // Заполняем доступные COM-порты
        if (selectPort && data.available_ports) {
          selectPort.innerHTML = '';
          const ports = data.available_ports.length > 0 ? data.available_ports : [data.serial_port];
          if (!ports.includes(data.serial_port)) ports.unshift(data.serial_port);
          ports.forEach(p => {
            const opt = document.createElement('option');
            opt.value = p;
            opt.textContent = `${p} ${p === data.serial_port ? '(Текущий)' : ''}`;
            if (p === data.serial_port) opt.selected = true;
            selectPort.appendChild(opt);
          });
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

        // Заполняем настройки типа вывода и WLED
        if (selectOutputType && data.output_type) {
          selectOutputType.value = data.output_type;
          updateOutputTypeVisibility(data.output_type);
        }
        if (inputWledHost && data.wled_host) {
          inputWledHost.value = data.wled_host;
        }
        if (inputWledPort && data.wled_port) {
          inputWledPort.value = data.wled_port;
        }
        if (selectWledProtocol && data.wled_protocol) {
          selectWledProtocol.value = data.wled_protocol;
        }

        // Заполняем настройки светомузыки
        if (selectAudioEffect && data.audio_effect) {
          selectAudioEffect.value = data.audio_effect;
        }
        if (sliderAudioSensitivity && data.audio_sensitivity) {
          sliderAudioSensitivity.value = Math.round(data.audio_sensitivity * 100);
          if (valAudioSensitivity) valAudioSensitivity.textContent = (data.audio_sensitivity).toFixed(1) + 'x';
        }

        // Заполняем горячие клавиши
        if (data.hotkeys) {
          const tog = document.getElementById('toggle-hotkeys-enabled');
          const pwr = document.getElementById('hk-toggle-power');
          const up = document.getElementById('hk-brightness-up');
          const down = document.getElementById('hk-brightness-down');
          const ovl = document.getElementById('hk-toggle-overlay');

          if (tog) tog.checked = data.hotkeys.enabled;
          setHotkeyDisplay(pwr, data.hotkeys.toggle_power);
          setHotkeyDisplay(up, data.hotkeys.brightness_up);
          setHotkeyDisplay(down, data.hotkeys.brightness_down);
          setHotkeyDisplay(ovl, data.hotkeys.toggle_overlay);
        }

        setMode(data.mode || 'screen');
        renderZones(data.zones);
      }
    } catch (e) {
      console.warn('Сервер ReAmbiBox недоступен:', e);
    }
  }

  // Вспомогательная функция установки отображения хоткея
  function setHotkeyDisplay(input, val) {
    if (!input) return;
    const str = (val || '').trim();
    if (!str || str.toLowerCase() === 'none' || str.toLowerCase() === 'disabled' || str === 'Отключено' || str === 'отключено') {
      input.value = 'Отключено';
      input.classList.add('disabled');
    } else {
      input.value = str;
      input.classList.remove('disabled');
    }
  }

  // Интерактивная запись горячих клавиш
  document.querySelectorAll('.hotkey-input').forEach(input => {
    input.addEventListener('focus', () => {
      input.dataset.oldVal = input.value;
      input.value = 'Нажмите клавиши...';
      input.classList.remove('disabled');
    });

    input.addEventListener('blur', () => {
      if (input.value === 'Нажмите клавиши...') {
        setHotkeyDisplay(input, input.dataset.oldVal || 'Отключено');
      }
    });

    input.addEventListener('keydown', (e) => {
      e.preventDefault();
      e.stopPropagation();

      // Очистка / отключение клавишами Backspace или Delete
      if (e.key === 'Backspace' || e.key === 'Delete') {
        setHotkeyDisplay(input, '');
        input.blur();
        return;
      }

      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;

      const parts = [];
      if (e.ctrlKey) parts.push('Ctrl');
      if (e.shiftKey) parts.push('Shift');
      if (e.altKey) parts.push('Alt');
      if (e.metaKey) parts.push('Win');

      let key = e.key;
      if (key === 'ArrowUp') key = 'Up';
      else if (key === 'ArrowDown') key = 'Down';
      else if (key === 'ArrowLeft') key = 'Left';
      else if (key === 'ArrowRight') key = 'Right';
      else if (key === ' ') key = 'Space';
      else if (key.length === 1) key = key.toUpperCase();

      // Если нет модификаторов, разрешаем только клавиши F1-F12
      if (parts.length === 0 && !key.startsWith('F')) {
        return;
      }

      parts.push(key);
      input.value = parts.join('+');
      input.classList.remove('disabled');
      input.blur();
    });
  });

  // Кнопки очистки / отключения («✕»)
  document.querySelectorAll('.btn-clear-hotkey').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      const targetId = btn.dataset.target;
      const targetInput = document.getElementById(targetId);
      if (targetInput) {
        setHotkeyDisplay(targetInput, '');
      }
    });
  });

  const btnSaveHotkeys = document.getElementById('btn-save-hotkeys');
  const btnResetHotkeys = document.getElementById('btn-reset-hotkeys');

  function getHotkeyPayloadValue(id) {
    const val = document.getElementById(id)?.value?.trim() || '';
    if (!val || val === 'Отключено' || val === 'Нажмите клавиши...') {
      return '';
    }
    return val;
  }

  btnSaveHotkeys?.addEventListener('click', () => {
    const hotkeys = {
      enabled: document.getElementById('toggle-hotkeys-enabled')?.checked ?? true,
      toggle_power: getHotkeyPayloadValue('hk-toggle-power'),
      brightness_up: getHotkeyPayloadValue('hk-brightness-up'),
      brightness_down: getHotkeyPayloadValue('hk-brightness-down'),
      toggle_overlay: getHotkeyPayloadValue('hk-toggle-overlay'),
    };
    queueConfigUpdate({ hotkeys });
    btnSaveHotkeys.textContent = 'Сохранено!';
    setTimeout(() => {
      btnSaveHotkeys.textContent = 'Сохранить хоткеи';
    }, 1200);
  });

  btnResetHotkeys?.addEventListener('click', () => {
    const tog = document.getElementById('toggle-hotkeys-enabled');
    const pwr = document.getElementById('hk-toggle-power');
    const up = document.getElementById('hk-brightness-up');
    const down = document.getElementById('hk-brightness-down');
    const ovl = document.getElementById('hk-toggle-overlay');

    if (tog) tog.checked = true;
    setHotkeyDisplay(pwr, 'Ctrl+Shift+A');
    setHotkeyDisplay(up, 'Ctrl+Shift+Up');
    setHotkeyDisplay(down, 'Ctrl+Shift+Down');
    setHotkeyDisplay(ovl, 'Ctrl+Shift+O');

    const hotkeys = {
      enabled: true,
      toggle_power: 'Ctrl+Shift+A',
      brightness_up: 'Ctrl+Shift+Up',
      brightness_down: 'Ctrl+Shift+Down',
      toggle_overlay: 'Ctrl+Shift+O',
    };
    queueConfigUpdate({ hotkeys });
    btnResetHotkeys.textContent = 'Сброшено!';
    setTimeout(() => {
      btnResetHotkeys.textContent = 'Сброс по умолчанию';
    }, 1200);
  });

  fetchStatus();
  setInterval(fetchStatus, 1000);
});
