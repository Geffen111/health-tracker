// Global dark-mode state. The toggle lives once in the sidebar (see +layout);
// every page reads the same value, persisted across launches.

export const theme = $state({ dark: false });

function apply() {
  document.documentElement.classList.toggle('dark', theme.dark);
}

/** Read the saved preference and apply it. Call once on app start. */
export function initTheme() {
  theme.dark = localStorage.getItem('theme') === 'dark';
  apply();
}

export function setTheme(dark: boolean) {
  theme.dark = dark;
  localStorage.setItem('theme', dark ? 'dark' : 'light');
  apply();
}

export function toggleTheme() {
  setTheme(!theme.dark);
}
