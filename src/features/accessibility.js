'use strict';
// Progressive accessibility: keeps existing button semantics so legacy browser tests remain valid.
// Arrow/Home/End navigation, roving focus, and safe layout responsiveness for 16 modules.
let wfsNavigationInitialized = false;
function wfsSyncNavigation() {
  const nav = document.querySelector('.tabs');
  if (!nav) return;
  for (const button of nav.querySelectorAll('.tab')) {
    const current = button.classList.contains('active');
    button.setAttribute('aria-current', current ? 'page' : 'false');
    button.setAttribute('aria-pressed', String(current));
    button.tabIndex = current ? 0 : -1;
    button.scrollMarginInline = '18px';
  }
}
function wfsInitNavigation() {
  if (wfsNavigationInitialized) return;
  const nav = document.querySelector('.tabs');
  if (!nav) return;
  wfsNavigationInitialized = true;
  nav.setAttribute('aria-label', 'WFS Notch modules — use Left/Right arrows to navigate');
  const buttons = Array.from(nav.querySelectorAll('.tab'));
  for (const button of buttons) {
    const name = button.dataset.tab;
    if (name) {
      button.id = button.id || 'nav-' + name;
      const panel = document.getElementById('panel-' + name);
      if (panel) {
        button.setAttribute('aria-controls', panel.id);
        panel.setAttribute('aria-labelledby', button.id);
      }
    }
  }
  const select = button => {
    button.focus({preventScroll:true});
    button.scrollIntoView({
      behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth',
      block:'nearest',
      inline:'nearest'
    });
    button.click(); // Uses the existing guarded switchTab() and autosave.
  };
  nav.addEventListener('keydown', event => {
    if (!['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
    const index = buttons.indexOf(document.activeElement);
    if (index < 0) return;
    event.preventDefault();
    let next = index;
    if (event.key === 'ArrowRight') next = (index + 1) % buttons.length;
    if (event.key === 'ArrowLeft') next = (index + buttons.length - 1) % buttons.length;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = buttons.length - 1;
    select(buttons[next]);
  });
  nav.addEventListener('wheel', event => {
    if (nav.scrollWidth <= nav.clientWidth || event.ctrlKey) return;
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
    event.preventDefault();
    nav.scrollLeft += event.deltaY;
  }, {passive:false});
  document.addEventListener('keydown', event => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || event.repeat) return;
    const target = event.target;
    if (target?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
    const shortcuts = ['home','todos','notes','calendar','timer','media','settings'];
    const number = Number(event.key);
    if (!Number.isInteger(number) || number < 1 || number > shortcuts.length) return;
    const button = nav.querySelector('.tab[data-tab="' + shortcuts[number-1] + '"]');
    if (!button) return;
    event.preventDefault();
    select(button);
  });
  const observer = new MutationObserver(mutations => {
    if (mutations.some(change => change.attributeName === 'class')) wfsSyncNavigation();
  });
  for (const button of buttons) observer.observe(button,{attributes:true,attributeFilter:['class']});
  wfsSyncNavigation();
}
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded',wfsInitNavigation,{once:true});
else wfsInitNavigation();
