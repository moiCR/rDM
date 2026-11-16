import { defineConfig } from 'astro/config';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://rdm.moicr.dev',
  base: '/',
  output: 'static',
  vite: { plugins: [tailwindcss()] },
});
