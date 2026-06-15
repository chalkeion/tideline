import { defineConfig } from 'vitest/config'

/** Base Vitest config shared across Tideline packages. */
export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
  },
})
