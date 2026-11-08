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

  // Ползунки
  const setupSlider = (sliderId, valId, formatFn) => {
    const slider = document.getElementById(sliderId);
    const label = document.getElementById(valId);
    if (slider && label) {
      slider.addEventListener('input', () => {
        label.textContent = formatFn(slider.value);
      });
    }
  };

  setupSlider('slider-brightness', 'val-brightness', v => v + '%');
  setupSlider('slider-smoothing', 'val-smoothing', v => (v / 100).toFixed(2));
  setupSlider('slider-gamma', 'val-gamma', v => (v / 10).toFixed(1));

  // Отрисовка зон подсветки
  const canvas = document.getElementById('monitor-canvas');
  let zones = [];

  // Генератор периметра зон (по умолчанию 60 LED: 20 верх, 20 низ, 10 лево, 10 право)
  function generateZones(topCount = 20, bottomCount = 20, leftCount = 10, rightCount = 10) {
    zones = [];
    let id = 0;
    const depth = 0.10;

    // Низ (слева направо)
    const bStep = 1.0 / bottomCount;
    for (let i = 0; i < bottomCount; i++) {
      zones.push({ id: id++, x: i * bStep, y: 1.0 - depth, w: bStep, h: depth });
    }

    // Право (снизу вверх)
    const rStep = 1.0 / rightCount;
    for (let i = rightCount - 1; i >= 0; i--) {
      zones.push({ id: id++, x: 1.0 - depth, y: i * rStep, w: depth, h: rStep });
    }

    // Верх (справа налево)
    const tStep = 1.0 / topCount;
    for (let i = topCount - 1; i >= 0; i--) {
      zones.push({ id: id++, x: i * tStep, y: 0.0, w: tStep, h: depth });
    }

    // Лево (сверху вниз)
    const lStep = 1.0 / leftCount;
    for (let i = 0; i < leftCount; i++) {
      zones.push({ id: id++, x: 0.0, y: i * lStep, w: depth, h: lStep });
    }

    renderZones();
  }

  function renderZones() {
    if (!canvas) return;
    canvas.innerHTML = '';
    const cw = canvas.clientWidth;
    const ch = canvas.clientHeight;

    zones.forEach(z => {
      const box = document.createElement('div');
      box.className = 'zone-box';
      box.style.left = (z.x * 100) + '%';
      box.style.top = (z.y * 100) + '%';
      box.style.width = (z.w * 100) + '%';
      box.style.height = (z.h * 100) + '%';
      box.textContent = z.id;
      canvas.appendChild(box);
    });
  }

  document.getElementById('btn-wizard')?.addEventListener('click', () => {
    generateZones(20, 20, 10, 10);
  });

  generateZones(20, 20, 10, 10);
});
