// Ferme les dropdowns .slot-picker au clic en dehors et sur Échap.
// Handler délégué au document : survit aux remplacements de contenu par htmx.
document.addEventListener('click', (e) => {
  document.querySelectorAll('details.slot-picker[open]').forEach((d) => {
    if (!d.contains(e.target)) d.removeAttribute('open');
  });
});
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  document.querySelectorAll('details.slot-picker[open]').forEach((d) => d.removeAttribute('open'));
});
