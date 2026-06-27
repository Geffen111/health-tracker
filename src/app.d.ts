// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
  namespace App {
    // interface Error {}
    // interface Locals {}
    // interface PageData {}
    // interface PageState {}
    // interface Platform {}
  }

  // Git commit of this build, injected by Vite (see vite.config.js). 'dev' locally.
  const __APP_COMMIT__: string;
}

export {};
