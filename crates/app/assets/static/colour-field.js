// Colour field: keeps the native <input type="color"> picker, the hex text
// input, and the preset swatches in sync. The text input stays the single
// source of truth submitted with the form; the picker and presets only write
// into it (and into the colour-name field, for presets). No dependencies.
(function () {
  function isHex(v) {
    return /^#[0-9A-Fa-f]{6}$/.test(v);
  }

  function init(root) {
    var hex = root.querySelector('input[name="colour_hex"]');
    var picker = root.querySelector('.colour-picker');
    if (!hex || !picker) return;

    var form = root.closest('form');
    var nameField = form && form.querySelector('input[name="colour_name"]');

    // hex text -> picker (only when it's a full valid #RRGGBB)
    hex.addEventListener('input', function () {
      if (isHex(hex.value)) picker.value = hex.value;
    });

    // picker -> hex text
    picker.addEventListener('input', function () {
      hex.value = picker.value.toUpperCase();
    });

    // preset swatch -> hex text + picker + colour name
    root.querySelectorAll('.colour-preset').forEach(function (btn) {
      btn.addEventListener('click', function () {
        var h = btn.getAttribute('data-hex') || '';
        hex.value = h.toUpperCase();
        if (isHex(h)) picker.value = h;
        var nm = btn.getAttribute('data-name');
        if (nameField && nm) nameField.value = nm;
      });
    });

    // seed the picker from a pre-filled hex (e.g. after a validation error)
    if (isHex(hex.value)) picker.value = hex.value;
  }

  document.addEventListener('DOMContentLoaded', function () {
    document.querySelectorAll('.colour-field').forEach(init);
  });
})();
