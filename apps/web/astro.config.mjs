// @ts-check
import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";

// Ganti lewat env saat deploy, mis. PUBLIC_SITE_URL=https://domain-final.example
const site = process.env.PUBLIC_SITE_URL ?? "https://polify.cianjur.space";

export default defineConfig({
  site,
  integrations: [sitemap()],
});
