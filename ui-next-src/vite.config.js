import { rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';

// The build goes to ../rust/resources/ui-next (served by lexo-server at /next/); its README stays. Old hashed files are removed before each build.
const OUT = fileURLToPath(new URL('../rust/resources/ui-next', import.meta.url));
const clean = () => ({
  name: 'clean-ui-next',
  buildStart() {
    rmSync(`${OUT}/assets`, { recursive: true, force: true });
    for (const f of ['index.html', 'tokens.css', 'components.css', 'styleguide.html', 'styleguide.js', 'placeholder.html']) rmSync(`${OUT}/${f}`, { force: true });
  },
});

export default defineConfig({
  base: '/next/',
  plugins: [vue(), clean()],
  build: { outDir: OUT, emptyOutDir: false, assetsDir: 'assets', sourcemap: false },
  // `npm run dev`: the page is at http://localhost:5173/next/ and talks to a running lexo-server (VITE_API, e.g. http://127.0.0.1:51777)
  server: { proxy: { '/api': { target: process.env.VITE_API || 'http://127.0.0.1:8765', changeOrigin: true } } },
  test: { environment: 'jsdom', include: ['src/**/*.test.js'] },
});
