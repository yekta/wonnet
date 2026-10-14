// @ts-check

import react from "@astrojs/react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "astro/config"

export default defineConfig({
  site: "https://wonnet.app",
  trailingSlash: "always",
  vite: {
    plugins: [tailwindcss()],
  },
  integrations: [react()],
})
