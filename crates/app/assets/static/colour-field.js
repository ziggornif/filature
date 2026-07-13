(function () {
  function normalize(value) {
    var raw = (value || '').trim();
    if (!raw) return '';
    if (raw.toLowerCase() === 'transparent') return 'transparent';
    var digits = raw.charAt(0) === '#' ? raw.slice(1) : raw;
    if (/^[0-9a-f]{3}$/i.test(digits)) {
      digits = digits.split('').map(function (c) { return c + c; }).join('');
    }
    return /^[0-9a-f]{6}$/i.test(digits) ? '#' + digits.toUpperCase() : null;
  }

  function initColour(root) {
    if (root.dataset.initialized) return;
    root.dataset.initialized = 'true';
    var hex = root.querySelector('input[name="colour_hex"]');
    var picker = root.querySelector('.colour-picker');
    var preview = root.querySelector('.colour-preview');
    var derived = root.querySelector('.colour-derived-name');
    var error = root.querySelector('.colour-error');
    var hint = root.querySelector('.colour-hint');
    var clear = root.querySelector('.colour-clear');
    var presets = Array.prototype.slice.call(root.querySelectorAll('.colour-preset'));

    function presetFor(value) {
      return presets.find(function (button) {
        return button.dataset.hex.toLowerCase() === value.toLowerCase();
      });
    }

    function render(value, invalid) {
      var hasColour = Boolean(value) && !invalid;
      var preset = hasColour ? presetFor(value) : null;
      preview.classList.toggle('is-unset', !hasColour);
      preview.classList.toggle('is-transparent', value === 'transparent');
      if (hasColour && value !== 'transparent') {
        preview.querySelector('.colour-preview__chip').style.setProperty('--preview', value);
        picker.value = value;
      }
      presets.forEach(function (button) {
        button.classList.toggle('is-selected', hasColour && button === preset);
      });
      derived.textContent = hasColour ? (preset ? preset.dataset.name : value.toUpperCase()) : '';
      error.textContent = invalid ? root.dataset.invalidLabel : '';
      hint.textContent = !hasColour && !invalid ? root.dataset.unsetLabel : '';
      hex.setAttribute('aria-invalid', invalid ? 'true' : 'false');
    }

    hex.addEventListener('input', function () {
      var value = normalize(hex.value);
      render(value || '', value === null);
    });
    hex.addEventListener('blur', function () {
      var value = normalize(hex.value);
      if (value !== null) hex.value = value;
      render(value || '', value === null);
    });
    picker.addEventListener('input', function () {
      hex.value = picker.value.toUpperCase();
      render(hex.value, false);
    });
    presets.forEach(function (button) {
      button.addEventListener('click', function () {
        hex.value = button.dataset.hex;
        render(hex.value, false);
      });
    });
    clear.addEventListener('click', function () {
      hex.value = '';
      render('', false);
      hex.focus();
    });
    var initial = normalize(hex.value);
    render(initial || '', initial === null);
  }

  function initDetails(root) {
    if (root.dataset.controlsInitialized) return;
    root.dataset.controlsInitialized = 'true';
    var manufacturer = root.querySelector('.manufacturer-select');
    var other = root.querySelector('.manufacturer-other');
    var manufacturerName = other && other.querySelector('input');
    function renderManufacturer() {
      var visible = manufacturer.value === '__other';
      other.classList.toggle('is-hidden', !visible);
      manufacturerName.required = visible;
    }
    manufacturer.addEventListener('change', renderManufacturer);
    renderManufacturer();

    var preset = root.querySelector('.net-weight-preset');
    var customLabel = root.querySelector('.net-weight-custom');
    var custom = customLabel.querySelector('input');
    var actual = root.querySelector('input[name="net_weight"]');
    var known = Array.prototype.some.call(preset.options, function (option) {
      return option.value === actual.value && option.value !== 'custom';
    });
    preset.value = known ? actual.value : 'custom';
    if (!known) custom.value = actual.value;
    function renderWeight() {
      var isCustom = preset.value === 'custom';
      customLabel.classList.toggle('is-hidden', !isCustom);
      custom.required = isCustom;
      actual.value = isCustom ? custom.value : preset.value;
    }
    preset.addEventListener('change', renderWeight);
    custom.addEventListener('input', renderWeight);
    renderWeight();
  }

  function init(scope) {
    (scope.matches && scope.matches('.colour-field') ? [scope] : scope.querySelectorAll('.colour-field'))
      .forEach(initColour);
    (scope.matches && scope.matches('.spool-wizard--details') ? [scope] : scope.querySelectorAll('.spool-wizard--details'))
      .forEach(initDetails);
  }

  document.addEventListener('DOMContentLoaded', function () { init(document); });
  document.addEventListener('htmx:load', function (event) { init(event.detail.elt); });
})();
