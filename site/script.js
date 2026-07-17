'use strict';

document.addEventListener('DOMContentLoaded', () => {
  const root = document.documentElement;
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  const themeButtons = [...document.querySelectorAll('[data-theme-choice]')];

  function preference() {
    try { return localStorage.getItem('filature-docs-theme') || 'auto'; }
    catch (_) { return 'auto'; }
  }

  function activeTheme() {
    const value = preference();
    return value === 'auto' ? (media.matches ? 'dark' : 'light') : value;
  }

  function applyTheme(value) {
    try {
      if (value === 'auto') localStorage.removeItem('filature-docs-theme');
      else localStorage.setItem('filature-docs-theme', value);
    } catch (_) {}
    if (value === 'auto') root.removeAttribute('data-theme');
    else root.dataset.theme = value;
    themeButtons.forEach(button => button.setAttribute('aria-pressed', String(button.dataset.themeChoice === value)));
    const theme = activeTheme();
    document.querySelectorAll('[data-theme-image]').forEach(image => {
      image.src = theme === 'dark' ? image.dataset.darkSrc : image.dataset.lightSrc;
    });
    document.querySelector('meta[name="theme-color"]').content = theme === 'dark' ? '#211f1c' : '#ece8e1';
  }

  themeButtons.forEach(button => button.addEventListener('click', () => applyTheme(button.dataset.themeChoice)));
  media.addEventListener('change', () => { if (preference() === 'auto') applyTheme('auto'); });
  applyTheme(preference());

  const menuButton = document.querySelector('.menu-toggle');
  const sidebar = document.getElementById('sidebar');
  function closeMenu() { sidebar.classList.remove('open'); menuButton.setAttribute('aria-expanded', 'false'); }
  menuButton.addEventListener('click', () => {
    const open = sidebar.classList.toggle('open');
    menuButton.setAttribute('aria-expanded', String(open));
  });
  sidebar.querySelectorAll('a').forEach(link => link.addEventListener('click', closeMenu));
  document.addEventListener('keydown', event => { if (event.key === 'Escape' && sidebar.classList.contains('open')) { closeMenu(); menuButton.focus(); } });

  const tabs = [...document.querySelectorAll('[role="tab"]')];
  function activateTab(tab) {
    tabs.forEach(item => {
      const selected = item === tab;
      item.setAttribute('aria-selected', String(selected));
      item.tabIndex = selected ? 0 : -1;
      document.getElementById(item.getAttribute('aria-controls')).hidden = !selected;
    });
  }
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => activateTab(tab));
    tab.addEventListener('keydown', event => {
      let next = index;
      if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
      else if (event.key === 'ArrowLeft') next = (index - 1 + tabs.length) % tabs.length;
      else if (event.key === 'Home') next = 0;
      else if (event.key === 'End') next = tabs.length - 1;
      else return;
      event.preventDefault(); activateTab(tabs[next]); tabs[next].focus();
    });
  });

  const links = [...document.querySelectorAll('.sidebar-link')];
  const sections = [...document.querySelectorAll('main section[id]')];
  const observer = new IntersectionObserver(entries => {
    entries.forEach(entry => entry.target.dataset.visible = String(entry.isIntersecting));
    const current = sections.find(section => section.dataset.visible === 'true');
    if (current) links.forEach(link => link.classList.toggle('active', link.hash === `#${current.id}`));
  }, { rootMargin: '-88px 0px -65% 0px' });
  sections.forEach(section => observer.observe(section));
});
